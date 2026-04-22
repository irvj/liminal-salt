//! Memory worker — scheduler tasks + manual-dispatch shell for the two memory
//! LLM pipelines.
//!
//! Owns "already running" coordination for both memory operations:
//! - **Per-persona** mutex for cross-thread persona memory.
//! - **Per-session** mutex for thread memory. This is DISTINCT from
//!   `session::SESSION_LOCKS` — that one guards session JSON file I/O; this
//!   one only ensures we don't start a second thread-memory update for the
//!   same session while one is already running.
//!
//! Status state machine: `Idle → Running → (Completed | Failed)`. The status
//! maps live here so the `/memory/status/` and `/session/thread-memory/status/`
//! endpoints can poll them.
//!
//! # Lock invariant (CLAUDE.md + roadmap "Known Hard Spot #1")
//!
//! The thread-memory pipeline must not hold the session-JSON lock across the
//! LLM call. This is achieved structurally: we never acquire the session-JSON
//! lock ourselves. `session::load_session` acquires and drops it internally
//! before we call the LLM; `session::save_thread_memory` does the same on the
//! write back. Between those two calls we hold only the per-session
//! thread-memory mutex (the "already running" lock), which is ours alone.
//!
//! The persona-memory pipeline is simpler: no session-JSON lock is involved,
//! and its own `memory::save_memory_content` writer is the sole writer for
//! `data/memory/{persona}.md`. Holding the per-persona mutex across the LLM
//! call is deliberate — it IS the "already running" lock.

use std::{
    collections::{HashMap, HashSet},
    path::Path,
    sync::{Arc, Mutex as StdMutex, MutexGuard},
    time::{Duration, SystemTime},
};

use chrono::{DateTime, SecondsFormat, Utc};
use serde::Serialize;
use tokio::{
    sync::{Mutex as TokioMutex, watch},
    task::JoinHandle,
    time::Instant,
};

use crate::{
    AppState,
    services::{
        config,
        llm::{ChatLlm, LlmClient},
        memory,
        persona,
        session::{self, Mode},
        thread_memory,
    },
};

/// `StdMutex` gets poisoned when a thread panics while holding the lock. In
/// the memory worker all guarded state is either a lookup map or coordination
/// metadata — any partial update a panicked task made is a recoverable nuisance,
/// not a fatal invariant break. Extract the inner data and keep running so one
/// buggy update doesn't freeze every future status query / scheduler tick.
trait MutexRecover<T> {
    fn lock_recover(&self) -> MutexGuard<'_, T>;
}

impl<T> MutexRecover<T> for StdMutex<T> {
    fn lock_recover(&self) -> MutexGuard<'_, T> {
        self.lock().unwrap_or_else(|e| e.into_inner())
    }
}

/// Floor for "sleep until next scheduler tick" — never spin faster than this.
const SCHEDULER_TICK_FLOOR: Duration = Duration::from_secs(10);
/// Sleep used when the scheduler has nothing scheduled and nothing to do.
const DEFAULT_POLL_WHEN_NO_WORK: Duration = Duration::from_secs(60);
/// Back-off when a fire was attempted but the "already running" mutex was held
/// (e.g. because a manual update is already underway). Short so the next free
/// slot is picked up quickly.
const RETRY_DELAY_WHEN_LOCK_HELD: Duration = Duration::from_secs(60);

/// Max time `stop_schedulers` waits for a mid-tick (or mid-LLM) scheduler to
/// finish before abandoning the handle and letting the runtime clean up.
/// Matches Python's `threading.Thread.join(timeout=15)`.
const SCHEDULER_STOP_TIMEOUT: Duration = Duration::from_secs(15);

/// Default minimum number of new messages across a persona's non-roleplay
/// sessions before an auto-update fires. Paired with `auto_memory_interval`
/// to form the unified trigger shape (interval AND floor).
pub const DEFAULT_AUTO_MEMORY_MESSAGE_FLOOR: u32 = 10;

// =============================================================================
// Public types
// =============================================================================

/// Status state machine. `Idle` is the implicit baseline.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum State {
    #[default]
    Idle,
    Running,
    Completed,
    Failed,
}

#[derive(Clone, Debug, Default, Serialize)]
pub struct Status {
    pub state: State,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub started_at: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub finished_at: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

#[derive(Clone, Copy, Debug)]
pub enum UpdateSource {
    Manual,
    Auto,
    Modify,
    Seed,
}

impl UpdateSource {
    fn as_str(self) -> &'static str {
        match self {
            Self::Manual => "manual",
            Self::Auto => "auto",
            Self::Modify => "modify",
            Self::Seed => "seed",
        }
    }
}

pub struct SchedulerHandles {
    stop_tx: watch::Sender<bool>,
    persona_handle: JoinHandle<()>,
    thread_handle: JoinHandle<()>,
}

// =============================================================================
// Worker
// =============================================================================

/// Cheaply cloneable handle to the shared worker state. Stored on `AppState`.
#[derive(Clone)]
pub struct MemoryWorker {
    inner: Arc<Inner>,
}

impl Default for MemoryWorker {
    fn default() -> Self {
        Self::new()
    }
}

struct Inner {
    // "Already running" mutexes — ours, separate from session-JSON locks.
    persona_locks: StdMutex<HashMap<String, Arc<TokioMutex<()>>>>,
    session_locks: StdMutex<HashMap<String, Arc<TokioMutex<()>>>>,

    // Status maps — polled by handlers.
    persona_status: StdMutex<HashMap<String, Status>>,
    thread_status: StdMutex<HashMap<String, Status>>,

    // Scheduler state. StdMutex for each; lock scope is always brief and
    // never holds across `.await`.
    persona_next_fire: StdMutex<HashMap<String, Instant>>,
    thread_next_fire: StdMutex<HashMap<String, Instant>>,
    persona_count_cache: StdMutex<HashMap<String, PersonaCountEntry>>,
    thread_scheduler_cache: StdMutex<HashMap<String, ThreadSchedEntry>>,
}

/// Per-session cache entry for the persona-memory scheduler's message-floor
/// counter. Reparse only when `mtime` changes.
struct PersonaCountEntry {
    mtime: SystemTime,
    persona: String,
    mode: Mode,
    timestamps: Vec<String>,
    missing: u32,
}

/// Per-session cache entry for the thread-memory scheduler. Keyed by mtime —
/// reparse only when the underlying session JSON changes.
struct ThreadSchedEntry {
    mtime: SystemTime,
    persona: String,
    thread_memory_settings_override: Option<session::ThreadMemorySettings>,
    new_message_count: u32,
}

#[derive(Clone)]
struct ThreadView {
    persona: String,
    thread_memory_settings_override: Option<session::ThreadMemorySettings>,
    new_message_count: u32,
}

impl MemoryWorker {
    pub fn new() -> Self {
        Self {
            inner: Arc::new(Inner {
                persona_locks: StdMutex::default(),
                session_locks: StdMutex::default(),
                persona_status: StdMutex::default(),
                thread_status: StdMutex::default(),
                persona_next_fire: StdMutex::default(),
                thread_next_fire: StdMutex::default(),
                persona_count_cache: StdMutex::default(),
                thread_scheduler_cache: StdMutex::default(),
            }),
        }
    }

    fn persona_lock(&self, persona: &str) -> Arc<TokioMutex<()>> {
        let mut map = self.inner.persona_locks.lock_recover();
        map.entry(persona.to_string())
            .or_insert_with(|| Arc::new(TokioMutex::new(())))
            .clone()
    }

    fn session_lock(&self, session_id: &str) -> Arc<TokioMutex<()>> {
        let mut map = self.inner.session_locks.lock_recover();
        map.entry(session_id.to_string())
            .or_insert_with(|| Arc::new(TokioMutex::new(())))
            .clone()
    }

    // =========================================================================
    // Status reads
    // =========================================================================

    pub fn get_update_status(&self, persona: &str) -> Status {
        self.inner
            .persona_status
            .lock_recover()
            .get(persona)
            .cloned()
            .unwrap_or_default()
    }

    pub fn get_thread_update_status(&self, session_id: &str) -> Status {
        self.inner
            .thread_status
            .lock_recover()
            .get(session_id)
            .cloned()
            .unwrap_or_default()
    }

    fn set_persona_status(&self, persona: &str, status: Status) {
        self.inner
            .persona_status
            .lock_recover()
            .insert(persona.to_string(), status);
    }

    fn set_thread_status(&self, session_id: &str, status: Status) {
        self.inner
            .thread_status
            .lock_recover()
            .insert(session_id.to_string(), status);
    }

    // =========================================================================
    // Manual dispatch — persona memory
    // =========================================================================

    /// Start a background persona-memory update. Returns false if an update
    /// is already running for this persona.
    pub fn start_manual_update(&self, state: AppState, persona: String) -> bool {
        if !self.try_reserve_persona_lock(&persona) {
            return false;
        }
        let this = self.clone();
        tokio::spawn(async move {
            let llm = build_memory_llm(&state, &persona).await;
            this.run_memory_update(&state, &llm, &persona, UpdateSource::Manual)
                .await;
        });
        true
    }

    pub fn start_modify_update(&self, state: AppState, persona: String, command: String) -> bool {
        if !self.try_reserve_persona_lock(&persona) {
            return false;
        }
        let this = self.clone();
        tokio::spawn(async move {
            let llm = build_memory_llm(&state, &persona).await;
            this.run_modify_memory(&state, &llm, &persona, &command).await;
        });
        true
    }

    pub fn start_seed_update(&self, state: AppState, persona: String, seed_content: String) -> bool {
        if !self.try_reserve_persona_lock(&persona) {
            return false;
        }
        let this = self.clone();
        tokio::spawn(async move {
            let llm = build_memory_llm(&state, &persona).await;
            this.run_seed_memory(&state, &llm, &persona, &seed_content).await;
        });
        true
    }

    /// Best-effort pre-check so the handler can return a fast 409 when an
    /// update is already running. The spawned task does its own `try_lock`
    /// on the same mutex to handle the narrow race between this check and
    /// the spawn actually executing.
    fn try_reserve_persona_lock(&self, persona: &str) -> bool {
        let lock = self.persona_lock(persona);
        lock.try_lock().is_ok()
    }

    // =========================================================================
    // Manual dispatch — thread memory
    // =========================================================================

    /// Start a background thread-memory update. Returns false if an update
    /// is already running for this session.
    pub fn start_thread_memory_update(
        &self,
        state: AppState,
        session_id: String,
        source: UpdateSource,
    ) -> bool {
        let lock = self.session_lock(&session_id);
        if lock.try_lock().is_err() {
            return false;
        }
        let this = self.clone();
        tokio::spawn(async move {
            let llm = build_thread_memory_llm(&state, &session_id).await;
            this.run_thread_memory_update(&state, &llm, &session_id, source)
                .await;
        });
        true
    }

    // =========================================================================
    // Settings-save hook
    // =========================================================================

    /// Re-anchor a session's next thread-memory fire time. Call after saving
    /// per-thread settings so the new interval takes effect cleanly — the
    /// next fire is `now + new_interval`, not either "immediately" or
    /// "wait out the previous interval."
    pub fn reschedule_thread_next_fire(&self, session_id: &str, interval_minutes: u32) {
        let mut map = self.inner.thread_next_fire.lock_recover();
        if interval_minutes > 0 {
            let secs = (interval_clamp(interval_minutes) as u64) * 60;
            map.insert(session_id.to_string(), Instant::now() + Duration::from_secs(secs));
        } else {
            map.remove(session_id);
        }
    }

    // =========================================================================
    // Core: persona memory update (generic over LLM so tests can inject a fake)
    // =========================================================================

    pub async fn run_memory_update<L: ChatLlm>(
        &self,
        state: &AppState,
        llm: &L,
        persona: &str,
        source: UpdateSource,
    ) -> bool {
        let lock = self.persona_lock(persona);
        let _guard = match lock.try_lock() {
            Ok(g) => g,
            Err(_) => return false,
        };

        self.set_persona_status(
            persona,
            Status {
                state: State::Running,
                source: Some(source.as_str().to_string()),
                started_at: Some(session::now_timestamp()),
                ..Default::default()
            },
        );
        tracing::info!(persona, source = source.as_str(), "memory update started");

        let persona_cfg = persona::load_persona_config(&state.data_dir, persona).await;
        let max_threads = persona_cfg.user_history_max_threads.unwrap_or(10);
        let messages_per_thread = persona_cfg.user_history_messages_per_thread.unwrap_or(100);

        let threads = session::list_persona_threads(
            &state.sessions_dir,
            persona,
            if max_threads > 0 { Some(max_threads as usize) } else { None },
            if messages_per_thread > 0 { Some(messages_per_thread as usize) } else { None },
        )
        .await;

        if threads.is_empty() {
            self.set_persona_status(
                persona,
                Status {
                    state: State::Completed,
                    finished_at: Some(session::now_timestamp()),
                    error: Some("No conversations found for this persona.".into()),
                    ..Default::default()
                },
            );
            return true;
        }

        let identity = persona::load_identity(&state.data_dir, persona).await;
        let size_limit = persona_cfg
            .memory_size_limit
            .unwrap_or(memory::DEFAULT_MEMORY_SIZE_LIMIT);

        let result = memory::update_memory(
            llm,
            &state.data_dir,
            persona,
            &identity,
            &threads,
            size_limit,
        )
        .await;

        self.complete_persona(persona, result, source.as_str());
        true
    }

    pub async fn run_modify_memory<L: ChatLlm>(
        &self,
        state: &AppState,
        llm: &L,
        persona: &str,
        command: &str,
    ) -> bool {
        let lock = self.persona_lock(persona);
        let _guard = match lock.try_lock() {
            Ok(g) => g,
            Err(_) => return false,
        };

        self.set_persona_status(
            persona,
            Status {
                state: State::Running,
                source: Some("modify".into()),
                started_at: Some(session::now_timestamp()),
                ..Default::default()
            },
        );

        let persona_cfg = persona::load_persona_config(&state.data_dir, persona).await;
        let identity = persona::load_identity(&state.data_dir, persona).await;
        let size_limit = persona_cfg
            .memory_size_limit
            .unwrap_or(memory::DEFAULT_MEMORY_SIZE_LIMIT);

        let result = memory::modify_memory(
            llm,
            &state.data_dir,
            persona,
            &identity,
            command,
            size_limit,
        )
        .await;

        self.complete_persona(persona, result, "modify");
        true
    }

    pub async fn run_seed_memory<L: ChatLlm>(
        &self,
        state: &AppState,
        llm: &L,
        persona: &str,
        seed_content: &str,
    ) -> bool {
        let lock = self.persona_lock(persona);
        let _guard = match lock.try_lock() {
            Ok(g) => g,
            Err(_) => return false,
        };

        self.set_persona_status(
            persona,
            Status {
                state: State::Running,
                source: Some("seed".into()),
                started_at: Some(session::now_timestamp()),
                ..Default::default()
            },
        );

        let persona_cfg = persona::load_persona_config(&state.data_dir, persona).await;
        let identity = persona::load_identity(&state.data_dir, persona).await;
        let size_limit = persona_cfg
            .memory_size_limit
            .unwrap_or(memory::DEFAULT_MEMORY_SIZE_LIMIT);

        let result = memory::seed_memory(
            llm,
            &state.data_dir,
            persona,
            &identity,
            seed_content,
            size_limit,
        )
        .await;

        self.complete_persona(persona, result, "seed");
        true
    }

    fn complete_persona(&self, persona: &str, result: Result<(), memory::MemoryError>, source: &str) {
        let finished = Some(session::now_timestamp());
        match result {
            Ok(()) => {
                self.set_persona_status(
                    persona,
                    Status {
                        state: State::Completed,
                        finished_at: finished,
                        ..Default::default()
                    },
                );
                tracing::info!(persona, source, "memory update completed");
            }
            Err(err) => {
                // Map each variant to a user-facing status message; the old
                // code collapsed everything to "unusable response".
                let user_msg = match &err {
                    memory::MemoryError::NoThreads => {
                        "No conversations found for this persona.".to_string()
                    }
                    memory::MemoryError::NoExistingMemory => {
                        "No existing memory to modify.".to_string()
                    }
                    memory::MemoryError::UnusableResponse => {
                        "The model returned an unusable response.".to_string()
                    }
                    memory::MemoryError::Llm(_) => {
                        "The model call failed. Try again.".to_string()
                    }
                    memory::MemoryError::Io(_) => {
                        "Could not write memory to disk.".to_string()
                    }
                    memory::MemoryError::InvalidPersonaName(_) => {
                        "Invalid persona name.".to_string()
                    }
                };
                self.set_persona_status(
                    persona,
                    Status {
                        state: State::Failed,
                        finished_at: finished,
                        error: Some(user_msg),
                        ..Default::default()
                    },
                );
                tracing::warn!(persona, source, error = %err, "memory update failed");
            }
        }
    }

    // =========================================================================
    // Core: thread memory update (THE lock-sensitive pipeline)
    // =========================================================================

    pub async fn run_thread_memory_update<L: ChatLlm>(
        &self,
        state: &AppState,
        llm: &L,
        session_id: &str,
        source: UpdateSource,
    ) -> bool {
        // Per-session thread-memory "already running" mutex — DISTINCT from
        // the session-JSON lock in session::SESSION_LOCKS.
        let lock = self.session_lock(session_id);
        let _guard = match lock.try_lock() {
            Ok(g) => g,
            Err(_) => return false,
        };

        // Wall-clock stamp captured BEFORE the session load. Used as both:
        //   (a) the display "Last updated: ..." value in the modal
        //   (b) the `thread_memory_updated_at` cutoff for `filter_new_messages`
        //       on the next run.
        // Stamping here (not at write time) keeps the concurrent-write
        // correctness property: any message that lands during the LLM call
        // has a timestamp strictly greater than `started_stamp`, so the next
        // run's `ts > updated_at` filter catches it. Python used the
        // last-message timestamp for this value; that was correct but showed
        // the user "Last updated: whenever I last spoke" — misleading.
        let started_stamp = session::now_timestamp();

        self.set_thread_status(
            session_id,
            Status {
                state: State::Running,
                source: Some(source.as_str().to_string()),
                started_at: Some(started_stamp.clone()),
                ..Default::default()
            },
        );
        tracing::info!(
            session_id,
            source = source.as_str(),
            "thread memory update started"
        );

        // ---- INVARIANT: session-JSON lock is NEVER held across the LLM call. ----
        // Only the thread-memory mutex (acquired above) is held. The session
        // JSON is loaded via session::load_session (which briefly acquires
        // and drops the session-JSON lock) and later saved via
        // session::save_thread_memory (ditto). Between those two calls we
        // hold nothing that would block session writes.

        let session_data = match session::load_session(&state.sessions_dir, session_id).await {
            Ok(s) => s,
            Err(err) => {
                let msg = match err {
                    session::SessionError::NotFound(_)
                    | session::SessionError::InvalidId(_) => "Session not found.".to_string(),
                    other => format!("Session load failed: {other}"),
                };
                self.set_thread_status(
                    session_id,
                    Status {
                        state: State::Failed,
                        finished_at: Some(session::now_timestamp()),
                        error: Some(msg),
                        ..Default::default()
                    },
                );
                return true;
            }
        };

        let existing_memory = session_data.thread_memory.clone();
        let updated_at = session_data.thread_memory_updated_at.clone();

        let mut new_messages =
            thread_memory::filter_new_messages(&session_data.messages, &updated_at);

        // Manual reruns reprocess the whole thread when there's nothing new
        // — lets the user refresh the summary after changing size_limit or
        // the prompt. Auto runs skip quietly; their interval/floor gating
        // already passed, but this last check short-circuits cleanly.
        if new_messages.is_empty()
            && matches!(source, UpdateSource::Manual)
            && !session_data.messages.is_empty()
        {
            new_messages = session_data.messages.clone();
        }

        if new_messages.is_empty() {
            self.set_thread_status(
                session_id,
                Status {
                    state: State::Completed,
                    finished_at: Some(session::now_timestamp()),
                    error: Some("No new messages since last update.".into()),
                    ..Default::default()
                },
            );
            tracing::info!(session_id, "thread memory update skipped: no new messages");
            return true;
        }

        let persona_name = session_data.persona.clone();
        let persona_display = display_persona_name(&persona_name);
        let mode = session_data.mode;

        let persona_cfg = persona::load_persona_config(&state.data_dir, &persona_name).await;
        let effective = thread_memory::resolve_settings(Some(&session_data), &persona_cfg);

        let persona_memory = if mode == Mode::Roleplay {
            String::new()
        } else {
            memory::get_memory_content(&state.data_dir, &persona_name).await
        };

        // ---- LLM call. No session-JSON lock held; thread-memory mutex held. ----
        let merged = thread_memory::merge(
            llm,
            &persona_display,
            &existing_memory,
            &new_messages,
            effective.size_limit,
            mode,
            &persona_memory,
        )
        .await;

        let Some(merged_text) = merged else {
            self.set_thread_status(
                session_id,
                Status {
                    state: State::Failed,
                    finished_at: Some(session::now_timestamp()),
                    error: Some("The model returned an unusable response.".into()),
                    ..Default::default()
                },
            );
            tracing::warn!(session_id, "thread memory update failed: unusable response");
            return true;
        };

        // save_thread_memory briefly re-acquires the session-JSON lock internally.
        if let Err(err) = session::save_thread_memory(
            &state.sessions_dir,
            session_id,
            &merged_text,
            &started_stamp,
        )
        .await
        {
            tracing::warn!(session_id, error = %err, "save_thread_memory failed");
        }

        self.set_thread_status(
            session_id,
            Status {
                state: State::Completed,
                finished_at: Some(session::now_timestamp()),
                ..Default::default()
            },
        );
        tracing::info!(session_id, "thread memory update completed");
        true
    }

    // =========================================================================
    // Scheduler lifecycle
    // =========================================================================

    /// Spawn both scheduler tasks. They run until `stop_schedulers` is called.
    pub fn start_schedulers(&self, state: AppState) -> SchedulerHandles {
        let (stop_tx, stop_rx) = watch::channel(false);

        let this = self.clone();
        let persona_state = state.clone();
        let persona_stop = stop_rx.clone();
        let persona_handle = tokio::spawn(async move {
            this.persona_scheduler_loop(persona_state, persona_stop).await;
        });

        let this = self.clone();
        let thread_handle = tokio::spawn(async move {
            this.thread_memory_scheduler_loop(state, stop_rx).await;
        });

        SchedulerHandles {
            stop_tx,
            persona_handle,
            thread_handle,
        }
    }

    /// Signal both schedulers to stop. Waits up to `SCHEDULER_STOP_TIMEOUT` for
    /// each to finish; an in-flight LLM call past that budget gets aborted so
    /// the process can exit in bounded time. Matches Python's
    /// `join(timeout=15)` semantics.
    pub async fn stop_schedulers(handles: SchedulerHandles) {
        let _ = handles.stop_tx.send(true);
        for handle in [handles.persona_handle, handles.thread_handle] {
            match tokio::time::timeout(SCHEDULER_STOP_TIMEOUT, handle).await {
                Ok(_) => {}
                Err(_) => {
                    tracing::warn!(
                        timeout_secs = SCHEDULER_STOP_TIMEOUT.as_secs(),
                        "scheduler did not stop within timeout; task will be dropped when runtime exits"
                    );
                }
            }
        }
    }

    async fn persona_scheduler_loop(
        self,
        state: AppState,
        mut stop_rx: watch::Receiver<bool>,
    ) {
        loop {
            if *stop_rx.borrow() {
                return;
            }
            let sleep_for = self.persona_scheduler_tick(&state).await;
            tokio::select! {
                _ = tokio::time::sleep(sleep_for) => {}
                _ = stop_rx.changed() => return,
            }
        }
    }

    async fn thread_memory_scheduler_loop(
        self,
        state: AppState,
        mut stop_rx: watch::Receiver<bool>,
    ) {
        loop {
            if *stop_rx.borrow() {
                return;
            }
            let sleep_for = self.thread_memory_scheduler_tick(&state).await;
            tokio::select! {
                _ = tokio::time::sleep(sleep_for) => {}
                _ = stop_rx.changed() => return,
            }
        }
    }

    async fn persona_scheduler_tick(&self, state: &AppState) -> Duration {
        let cfg = config::load_config(&state.data_dir).await;
        if cfg.openrouter_api_key.is_empty() {
            return DEFAULT_POLL_WHEN_NO_WORK;
        }

        let personas = persona::list_personas(&state.data_dir).await;
        let now = Instant::now();
        let mut next_due_times: Vec<Instant> = Vec::new();

        for persona_name in personas {
            let persona_cfg = persona::load_persona_config(&state.data_dir, &persona_name).await;
            let Some(interval_min) = persona_cfg.auto_memory_interval.filter(|v| *v > 0) else {
                continue;
            };
            let interval = Duration::from_secs((interval_clamp(interval_min) as u64) * 60);
            let floor = persona_cfg
                .auto_memory_message_floor
                .unwrap_or(DEFAULT_AUTO_MEMORY_MESSAGE_FLOOR);

            let due_at = self
                .inner
                .persona_next_fire
                .lock_recover()
                .get(&persona_name)
                .copied();
            if let Some(at) = due_at
                && now < at
            {
                next_due_times.push(at);
                continue;
            }

            // Interval elapsed — check the message floor.
            let memory_mtime = memory::get_mtime(&state.data_dir, &persona_name).await;
            let new_count = self
                .count_new_messages_for_persona(&state.sessions_dir, &persona_name, memory_mtime)
                .await;

            if new_count < floor {
                let next = now + interval;
                self.inner
                    .persona_next_fire
                    .lock_recover()
                    .insert(persona_name.clone(), next);
                next_due_times.push(next);
                continue;
            }

            // Fire synchronously — matches Python's persona scheduler. The
            // per-persona mutex already serializes concurrent manual starts,
            // so running one update at a time across all personas is fine.
            let llm = build_memory_llm(state, &persona_name).await;
            let fired = self
                .run_memory_update(state, &llm, &persona_name, UpdateSource::Auto)
                .await;
            let next = if fired {
                Instant::now() + interval
            } else {
                Instant::now() + RETRY_DELAY_WHEN_LOCK_HELD
            };
            self.inner
                .persona_next_fire
                .lock_recover()
                .insert(persona_name, next);
            next_due_times.push(next);
        }

        compute_next_sleep(&next_due_times)
    }

    async fn thread_memory_scheduler_tick(&self, state: &AppState) -> Duration {
        let cfg = config::load_config(&state.data_dir).await;
        if cfg.openrouter_api_key.is_empty() {
            return DEFAULT_POLL_WHEN_NO_WORK;
        }

        let sessions_dir = &state.sessions_dir;
        let mut entries = match tokio::fs::read_dir(sessions_dir).await {
            Ok(e) => e,
            Err(_) => return DEFAULT_POLL_WHEN_NO_WORK,
        };

        let now = Instant::now();
        let mut next_due_times: Vec<Instant> = Vec::new();
        let mut live: HashSet<String> = HashSet::new();

        while let Ok(Some(entry)) = entries.next_entry().await {
            let filename = entry.file_name().to_string_lossy().to_string();
            if !filename.ends_with(".json") {
                continue;
            }
            let filepath = sessions_dir.join(&filename);
            let Some(view) = self.cached_thread_view(&filepath, &filename).await else {
                continue;
            };
            live.insert(filename.clone());

            let persona_cfg = persona::load_persona_config(&state.data_dir, &view.persona).await;
            // Build a stand-in Session with only the override field populated
            // so resolve_settings walks all three tiers without us duplicating
            // its logic.
            let stand_in = session::Session {
                title: String::new(),
                title_locked: None,
                persona: view.persona.clone(),
                mode: Mode::default(),
                messages: Vec::new(),
                draft: None,
                pinned: None,
                scenario: None,
                thread_memory: String::new(),
                thread_memory_updated_at: String::new(),
                thread_memory_settings: view.thread_memory_settings_override.clone(),
            };
            let effective = thread_memory::resolve_settings(Some(&stand_in), &persona_cfg);
            if effective.interval_minutes == 0 {
                continue;
            }
            let interval =
                Duration::from_secs((interval_clamp(effective.interval_minutes) as u64) * 60);

            let due_at = self
                .inner
                .thread_next_fire
                .lock_recover()
                .get(&filename)
                .copied();
            if let Some(at) = due_at
                && now < at
            {
                next_due_times.push(at);
                continue;
            }

            if view.new_message_count < effective.message_floor {
                let next = now + interval;
                self.inner
                    .thread_next_fire
                    .lock_recover()
                    .insert(filename.clone(), next);
                next_due_times.push(next);
                continue;
            }

            // Dispatch async — matches Python. A slow LLM for one session
            // does not block the scheduler from checking others.
            let fired = self.start_thread_memory_update(
                state.clone(),
                filename.clone(),
                UpdateSource::Auto,
            );
            let next = if fired {
                Instant::now() + interval
            } else {
                Instant::now() + RETRY_DELAY_WHEN_LOCK_HELD
            };
            self.inner
                .thread_next_fire
                .lock_recover()
                .insert(filename.clone(), next);
            next_due_times.push(next);
        }

        // Prune deleted-session entries from both scheduler maps.
        {
            let mut fire = self.inner.thread_next_fire.lock_recover();
            fire.retain(|k, _| live.contains(k));
        }
        {
            let mut cache = self.inner.thread_scheduler_cache.lock_recover();
            cache.retain(|k, _| live.contains(k));
        }

        compute_next_sleep(&next_due_times)
    }

    async fn cached_thread_view(&self, filepath: &Path, filename: &str) -> Option<ThreadView> {
        let mtime = tokio::fs::metadata(filepath).await.ok()?.modified().ok()?;

        {
            let cache = self.inner.thread_scheduler_cache.lock_recover();
            if let Some(entry) = cache.get(filename)
                && entry.mtime == mtime
            {
                return Some(ThreadView {
                    persona: entry.persona.clone(),
                    thread_memory_settings_override: entry.thread_memory_settings_override.clone(),
                    new_message_count: entry.new_message_count,
                });
            }
        }

        let bytes = tokio::fs::read(filepath).await.ok()?;
        let session_data: session::Session = serde_json::from_slice(&bytes).ok()?;
        let new_messages = thread_memory::filter_new_messages(
            &session_data.messages,
            &session_data.thread_memory_updated_at,
        );
        let entry = ThreadSchedEntry {
            mtime,
            persona: session_data.persona.clone(),
            thread_memory_settings_override: session_data.thread_memory_settings.clone(),
            new_message_count: new_messages.len() as u32,
        };
        let view = ThreadView {
            persona: entry.persona.clone(),
            thread_memory_settings_override: entry.thread_memory_settings_override.clone(),
            new_message_count: entry.new_message_count,
        };
        self.inner
            .thread_scheduler_cache
            .lock_recover()
            .insert(filename.to_string(), entry);
        Some(view)
    }

    async fn count_new_messages_for_persona(
        &self,
        sessions_dir: &Path,
        persona: &str,
        since_mtime: Option<SystemTime>,
    ) -> u32 {
        let mut entries = match tokio::fs::read_dir(sessions_dir).await {
            Ok(e) => e,
            Err(_) => return 0,
        };

        let since_iso = since_mtime.map(|t| {
            let dt: DateTime<Utc> = t.into();
            dt.to_rfc3339_opts(SecondsFormat::Micros, true)
        });

        let mut count: u32 = 0;
        let mut live: HashSet<String> = HashSet::new();

        while let Ok(Some(entry)) = entries.next_entry().await {
            let filename = entry.file_name().to_string_lossy().to_string();
            if !filename.ends_with(".json") {
                continue;
            }
            let Ok(meta) = entry.metadata().await else { continue };
            let Ok(mtime) = meta.modified() else { continue };
            live.insert(filename.clone());

            // Cache peek — clone out because we can't hold the StdMutex guard
            // across the read-and-maybe-reparse branch.
            let cached = {
                let cache = self.inner.persona_count_cache.lock_recover();
                cache
                    .get(&filename)
                    .filter(|e| e.mtime == mtime)
                    .map(|e| {
                        (
                            e.persona.clone(),
                            e.mode,
                            e.timestamps.clone(),
                            e.missing,
                        )
                    })
            };

            let (entry_persona, entry_mode, timestamps, missing) = match cached {
                Some(c) => c,
                None => {
                    let bytes = match tokio::fs::read(entry.path()).await {
                        Ok(b) => b,
                        Err(_) => continue,
                    };
                    let data: session::Session = match serde_json::from_slice(&bytes) {
                        Ok(d) => d,
                        Err(_) => continue,
                    };
                    let mut ts: Vec<String> = Vec::new();
                    let mut missing: u32 = 0;
                    for m in &data.messages {
                        if m.timestamp.is_empty() {
                            missing = missing.saturating_add(1);
                        } else {
                            ts.push(m.timestamp.clone());
                        }
                    }
                    let p = data.persona.clone();
                    let mo = data.mode;
                    self.inner.persona_count_cache.lock_recover().insert(
                        filename.clone(),
                        PersonaCountEntry {
                            mtime,
                            persona: p.clone(),
                            mode: mo,
                            timestamps: ts.clone(),
                            missing,
                        },
                    );
                    (p, mo, ts, missing)
                }
            };

            if entry_persona != persona || entry_mode == Mode::Roleplay {
                continue;
            }

            match &since_iso {
                None => {
                    count = count.saturating_add(timestamps.len() as u32).saturating_add(missing);
                }
                Some(since) => {
                    for ts in &timestamps {
                        if ts.as_str() > since.as_str() {
                            count = count.saturating_add(1);
                        }
                    }
                    if missing > 0 {
                        tracing::warn!(
                            count_missing = missing,
                            file = %filename,
                            "count_new_messages: messages without timestamp, counting as new"
                        );
                        count = count.saturating_add(missing);
                    }
                }
            }
        }

        let mut cache = self.inner.persona_count_cache.lock_recover();
        cache.retain(|k, _| live.contains(k));

        count
    }

    // =========================================================================
    // Test-only introspection
    // =========================================================================

    #[cfg(test)]
    pub fn persona_count_cache_size(&self) -> usize {
        self.inner.persona_count_cache.lock_recover().len()
    }

    #[cfg(test)]
    pub fn thread_scheduler_cache_size(&self) -> usize {
        self.inner.thread_scheduler_cache.lock_recover().len()
    }
}

// =============================================================================
// Free helpers
// =============================================================================

fn interval_clamp(minutes: u32) -> u32 {
    minutes.clamp(5, 1440)
}

fn compute_next_sleep(next_due_times: &[Instant]) -> Duration {
    if next_due_times.is_empty() {
        return DEFAULT_POLL_WHEN_NO_WORK;
    }
    let now = Instant::now();
    let min = next_due_times.iter().min().copied().unwrap_or(now);
    min.saturating_duration_since(now).max(SCHEDULER_TICK_FLOOR)
}

fn display_persona_name(persona: &str) -> String {
    persona
        .split('_')
        .map(|w| {
            let mut c = w.chars();
            match c.next() {
                Some(f) => f.to_uppercase().collect::<String>() + c.as_str(),
                None => String::new(),
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

async fn build_memory_llm(state: &AppState, persona: &str) -> LlmClient {
    let cfg = config::load_config(&state.data_dir).await;
    let persona_cfg = persona::load_persona_config(&state.data_dir, persona).await;
    let override_model = cfg.extras.get("MEMORY_MODEL").and_then(|v| v.as_str());
    let model = memory::get_memory_model(override_model, &persona_cfg, &cfg.model);
    LlmClient::new(cfg.openrouter_api_key, model)
        .with_http_client(state.http.clone())
        .with_timeout(Duration::from_secs(600))
}

async fn build_thread_memory_llm(state: &AppState, session_id: &str) -> LlmClient {
    let cfg = config::load_config(&state.data_dir).await;
    let persona_name = session::load_session(&state.sessions_dir, session_id)
        .await
        .map(|s| s.persona)
        .unwrap_or_else(|_| cfg.default_persona.clone());
    let persona_cfg = persona::load_persona_config(&state.data_dir, &persona_name).await;
    let override_model = cfg.extras.get("MEMORY_MODEL").and_then(|v| v.as_str());
    let model = memory::get_memory_model(override_model, &persona_cfg, &cfg.model);
    LlmClient::new(cfg.openrouter_api_key, model)
        .with_http_client(state.http.clone())
        .with_timeout(Duration::from_secs(600))
}
