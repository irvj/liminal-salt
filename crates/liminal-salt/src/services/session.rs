//! SessionManager — all session file I/O in one place.
//!
//! Every read and every read-modify-write acquires a per-session lock so that
//! concurrent writers (ChatCore saving messages, memory worker saving a thread
//! summary) can't clobber each other. Locks are process-local; the app is
//! single-process.
//!
//! Invariants (see CLAUDE.md):
//! - Never hold the session lock across an LLM call. The memory worker acquires
//!   → loads → drops the guard → calls the LLM → re-acquires to write.
//! - Session IDs are validated against a strict regex before any filesystem
//!   access; invalid IDs short-circuit to `None` / no-op without panicking.

use std::{
    collections::HashMap,
    path::{Path, PathBuf},
    sync::{Arc, LazyLock, Mutex as StdMutex},
};

use chrono::{SecondsFormat, Utc};
use regex::Regex;
use serde::{Deserialize, Serialize};
use tokio::sync::Mutex as TokioMutex;

// =============================================================================
// Types
// =============================================================================

#[derive(Clone, Copy, Debug, Default, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum Mode {
    #[default]
    Chatbot,
    Roleplay,
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum Role {
    User,
    Assistant,
    System,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Message {
    pub role: Role,
    pub content: String,
    pub timestamp: String,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize, PartialEq, Eq)]
pub struct ThreadMemorySettings {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub interval_minutes: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub message_floor: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub size_limit: Option<u32>,
}

/// Session JSON schema. See CLAUDE.md "Session JSON schema" table.
///
/// Optional fields use `skip_serializing_if = "Option::is_none"` so newly-created
/// sessions don't write empty placeholders — matching the "absent until first
/// set" semantics from Python.
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Session {
    pub title: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub title_locked: Option<bool>,
    pub persona: String,
    #[serde(default)]
    pub mode: Mode,
    #[serde(default)]
    pub messages: Vec<Message>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub draft: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pinned: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub scenario: Option<String>,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub thread_memory: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub thread_memory_updated_at: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub thread_memory_settings: Option<ThreadMemorySettings>,
}

impl Session {
    fn blank() -> Self {
        Self {
            title: String::new(),
            title_locked: None,
            persona: String::new(),
            mode: Mode::Chatbot,
            messages: Vec::new(),
            draft: None,
            pinned: None,
            scenario: None,
            thread_memory: String::new(),
            thread_memory_updated_at: String::new(),
            thread_memory_settings: None,
        }
    }
}

/// Lightweight summary used by the sidebar listing; reads a session file without
/// acquiring the per-session lock (matches Python's `get_sessions_with_titles`).
#[derive(Clone, Debug, Serialize)]
pub struct SessionSummary {
    pub id: String,
    pub title: String,
    pub persona: String,
    pub pinned: bool,
    pub mode: Mode,
}

/// A session's thread reduced to the fields memory aggregation cares about.
/// Produced by `list_persona_threads`; consumed by `memory::update_memory`.
#[derive(Clone, Debug)]
pub struct ThreadSnapshot {
    pub title: String,
    pub persona: String,
    pub messages: Vec<Message>,
}

// =============================================================================
// Errors
// =============================================================================

/// Every distinct failure mode that can arise from a session operation. The
/// handler layer maps variants to HTTP status codes; internal callers (chat,
/// memory worker) propagate via their own error types.
#[derive(Debug, thiserror::Error)]
pub enum SessionError {
    #[error("invalid session id: {0}")]
    InvalidId(String),
    #[error("session not found: {0}")]
    NotFound(String),
    /// Operation violates a session-state invariant the caller should have
    /// upheld (e.g. "remove last assistant" when there isn't one).
    #[error("session state invalid: {0}")]
    InvalidState(&'static str),
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
    #[error("session json corrupt: {0}")]
    Corrupt(#[from] serde_json::Error),
}

// =============================================================================
// Timestamps & IDs
// =============================================================================

/// Canonical UTC timestamp used for message timestamps and thread-memory cutoffs.
/// `2026-04-21T12:34:56.123456Z` — fixed-width, lexicographically sortable,
/// `chrono::DateTime::parse_from_rfc3339` round-trips.
pub fn now_timestamp() -> String {
    Utc::now().to_rfc3339_opts(SecondsFormat::Micros, true)
}

/// Generate a new session filename. UTC-based (Python uses local time; using UTC
/// keeps filenames monotonic across timezones).
pub fn generate_session_id() -> String {
    format!("session_{}.json", Utc::now().format("%Y%m%d_%H%M%S"))
}

static SESSION_ID_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"^session_\d{8}_\d{6}(?:_\d+)?\.json$").expect("valid regex")
});

pub fn valid_session_id(id: &str) -> bool {
    SESSION_ID_RE.is_match(id)
}

// =============================================================================
// Per-session locks
// =============================================================================

static SESSION_LOCKS: LazyLock<StdMutex<HashMap<String, Arc<TokioMutex<()>>>>> =
    LazyLock::new(|| StdMutex::new(HashMap::new()));

fn get_session_lock(session_id: &str) -> Arc<TokioMutex<()>> {
    let mut registry = SESSION_LOCKS.lock().expect("session lock registry poisoned");
    registry
        .entry(session_id.to_string())
        .or_insert_with(|| Arc::new(TokioMutex::new(())))
        .clone()
}

fn drop_session_lock(session_id: &str) {
    let mut registry = SESSION_LOCKS.lock().expect("session lock registry poisoned");
    registry.remove(session_id);
}

// =============================================================================
// Low-level I/O (always called under a held per-session lock)
// =============================================================================

fn session_path(sessions_dir: &Path, session_id: &str) -> PathBuf {
    sessions_dir.join(session_id)
}

/// Low-level read. Errors:
/// - `NotFound(session_id)` — missing file (caller supplies the id for context)
/// - `Io(...)` — any other read failure
/// - `Corrupt(...)` — JSON didn't deserialize into `Session`
async fn read_session(path: &Path, session_id: &str) -> Result<Session, SessionError> {
    let bytes = match tokio::fs::read(path).await {
        Ok(b) => b,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => {
            return Err(SessionError::NotFound(session_id.to_string()));
        }
        Err(err) => return Err(SessionError::Io(err)),
    };
    Ok(serde_json::from_slice::<Session>(&bytes)?)
}

async fn write_session(path: &Path, session: &Session) -> std::io::Result<()> {
    let bytes = serde_json::to_vec_pretty(session)
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
    crate::services::fs::write_atomic(path, &bytes).await
}

// =============================================================================
// Public reads
// =============================================================================

pub async fn load_session(
    sessions_dir: &Path,
    session_id: &str,
) -> Result<Session, SessionError> {
    if !valid_session_id(session_id) {
        return Err(SessionError::InvalidId(session_id.to_string()));
    }
    let lock = get_session_lock(session_id);
    let _guard = lock.lock().await;
    read_session(&session_path(sessions_dir, session_id), session_id).await
}

/// List all sessions in the directory. Doesn't acquire per-session locks —
/// matches Python's `get_sessions_with_titles()`; a brief stale read during
/// concurrent writes is acceptable for sidebar display.
pub async fn list_sessions(sessions_dir: &Path) -> Vec<SessionSummary> {
    let _ = tokio::fs::create_dir_all(sessions_dir).await;
    let mut entries = match tokio::fs::read_dir(sessions_dir).await {
        Ok(e) => e,
        Err(err) => {
            tracing::error!(?sessions_dir, error = %err, "list_sessions: read_dir failed");
            return Vec::new();
        }
    };

    let mut summaries: Vec<SessionSummary> = Vec::new();
    while let Ok(Some(entry)) = entries.next_entry().await {
        let filename = entry.file_name().to_string_lossy().to_string();
        if !filename.ends_with(".json") {
            continue;
        }
        let path = entry.path();
        match read_session(&path, &filename).await {
            Ok(s) => summaries.push(SessionSummary {
                id: filename,
                title: s.title,
                persona: s.persona,
                pinned: s.pinned.unwrap_or(false),
                mode: s.mode,
            }),
            Err(err) => {
                tracing::error!(id = %filename, error = %err, "list_sessions: read failed");
                summaries.push(SessionSummary {
                    id: filename,
                    title: "Error Loading".to_string(),
                    persona: "assistant".to_string(),
                    pinned: false,
                    mode: Mode::Chatbot,
                });
            }
        }
    }

    summaries.sort_by(|a, b| b.id.cmp(&a.id));
    summaries
}

/// Aggregate messages from sessions that match a persona, newest session first.
/// Skips roleplay sessions — those don't feed cross-thread persona memory.
///
/// `max_threads` caps the number of threads returned (newest by file mtime).
/// `messages_per_thread` trims each thread's messages to its most recent N.
/// Both `None` means no cap.
///
/// Like `list_sessions`, this reads without acquiring the per-session lock: a
/// brief stale read during concurrent writes is acceptable since the scheduler
/// tolerates missing the most recent in-flight message.
pub async fn list_persona_threads(
    sessions_dir: &Path,
    persona: &str,
    max_threads: Option<usize>,
    messages_per_thread: Option<usize>,
) -> Vec<ThreadSnapshot> {
    let mut entries = match tokio::fs::read_dir(sessions_dir).await {
        Ok(e) => e,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => return Vec::new(),
        Err(err) => {
            tracing::error!(?sessions_dir, error = %err, "list_persona_threads: read_dir failed");
            return Vec::new();
        }
    };

    // Collect (path, mtime) so we can sort newest-first before reading bodies.
    let mut files: Vec<(PathBuf, std::time::SystemTime)> = Vec::new();
    while let Ok(Some(entry)) = entries.next_entry().await {
        let filename = entry.file_name().to_string_lossy().to_string();
        if !filename.ends_with(".json") {
            continue;
        }
        let Ok(meta) = entry.metadata().await else { continue };
        let Ok(mtime) = meta.modified() else { continue };
        files.push((entry.path(), mtime));
    }
    files.sort_by_key(|(_, mtime)| std::cmp::Reverse(*mtime));

    if let Some(cap) = max_threads {
        files.truncate(cap);
    }

    let mut threads = Vec::new();
    for (path, _) in files {
        let filename = path
            .file_name()
            .and_then(|s| s.to_str())
            .unwrap_or("")
            .to_string();
        let Ok(session) = read_session(&path, &filename).await else {
            continue;
        };
        if session.persona != persona {
            continue;
        }
        if session.mode == Mode::Roleplay {
            continue;
        }
        if session.messages.is_empty() {
            continue;
        }
        let mut messages = session.messages;
        if let Some(cap) = messages_per_thread
            && messages.len() > cap
        {
            let start = messages.len() - cap;
            messages = messages.split_off(start);
        }
        threads.push(ThreadSnapshot {
            title: session.title,
            persona: session.persona,
            messages,
        });
    }
    threads
}

// =============================================================================
// Public writes (each acquires the per-session lock)
// =============================================================================

pub async fn create_session(
    sessions_dir: &Path,
    session_id: &str,
    persona: &str,
    title: &str,
    mode: Mode,
    messages: Vec<Message>,
) -> Result<Session, SessionError> {
    if !valid_session_id(session_id) {
        return Err(SessionError::InvalidId(session_id.to_string()));
    }
    let session = Session {
        title: title.to_string(),
        title_locked: None,
        persona: persona.to_string(),
        mode,
        messages,
        draft: None,
        pinned: None,
        scenario: None,
        thread_memory: String::new(),
        thread_memory_updated_at: String::new(),
        thread_memory_settings: None,
    };
    let lock = get_session_lock(session_id);
    let _guard = lock.lock().await;
    write_session(&session_path(sessions_dir, session_id), &session).await?;
    Ok(session)
}

pub async fn delete_session(sessions_dir: &Path, session_id: &str) -> Result<(), SessionError> {
    if !valid_session_id(session_id) {
        return Err(SessionError::InvalidId(session_id.to_string()));
    }
    let outcome = {
        let lock = get_session_lock(session_id);
        let _guard = lock.lock().await;
        let path = session_path(sessions_dir, session_id);
        match tokio::fs::remove_file(&path).await {
            Ok(()) => Ok(()),
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => {
                Err(SessionError::NotFound(session_id.to_string()))
            }
            Err(err) => Err(SessionError::Io(err)),
        }
    };
    if outcome.is_ok() {
        drop_session_lock(session_id);
    }
    outcome
}

/// Write chat-owned fields (title, persona, messages) while preserving every
/// other field. The RMW keeps mode, scenario, thread_memory,
/// thread_memory_updated_at, thread_memory_settings, pinned, draft, and
/// title_locked intact unless explicitly overwritten. If the session file is
/// missing, a fresh one is written.
pub async fn save_chat_history(
    sessions_dir: &Path,
    session_id: &str,
    title: &str,
    persona: &str,
    messages: Vec<Message>,
    title_locked: Option<bool>,
) -> Result<(), SessionError> {
    if !valid_session_id(session_id) {
        return Err(SessionError::InvalidId(session_id.to_string()));
    }
    let lock = get_session_lock(session_id);
    let _guard = lock.lock().await;
    let path = session_path(sessions_dir, session_id);
    let mut session = match read_session(&path, session_id).await {
        Ok(s) => s,
        Err(SessionError::NotFound(_)) => Session::blank(),
        Err(err) => return Err(err),
    };
    session.title = title.to_string();
    session.persona = persona.to_string();
    session.messages = messages;
    if let Some(locked) = title_locked {
        session.title_locked = Some(locked);
    }
    write_session(&path, &session).await?;
    Ok(())
}

/// Toggle pinned status. Returns the new state on success.
pub async fn toggle_pin(sessions_dir: &Path, session_id: &str) -> Result<bool, SessionError> {
    if !valid_session_id(session_id) {
        return Err(SessionError::InvalidId(session_id.to_string()));
    }
    let lock = get_session_lock(session_id);
    let _guard = lock.lock().await;
    let path = session_path(sessions_dir, session_id);
    let mut session = read_session(&path, session_id).await?;
    let new_state = !session.pinned.unwrap_or(false);
    session.pinned = Some(new_state);
    write_session(&path, &session).await?;
    Ok(new_state)
}

/// Update the title. Flags the title as user-set so auto-generation won't
/// overwrite it on subsequent sends — even when renamed to the literal "New Chat".
pub async fn rename_session(
    sessions_dir: &Path,
    session_id: &str,
    new_title: &str,
) -> Result<(), SessionError> {
    if !valid_session_id(session_id) {
        return Err(SessionError::InvalidId(session_id.to_string()));
    }
    let lock = get_session_lock(session_id);
    let _guard = lock.lock().await;
    let path = session_path(sessions_dir, session_id);
    let mut session = read_session(&path, session_id).await?;
    session.title = new_title.to_string();
    session.title_locked = Some(true);
    write_session(&path, &session).await?;
    Ok(())
}

pub async fn save_draft(
    sessions_dir: &Path,
    session_id: &str,
    draft: &str,
) -> Result<(), SessionError> {
    if !valid_session_id(session_id) {
        return Err(SessionError::InvalidId(session_id.to_string()));
    }
    let lock = get_session_lock(session_id);
    let _guard = lock.lock().await;
    let path = session_path(sessions_dir, session_id);
    let mut session = read_session(&path, session_id).await?;
    session.draft = Some(draft.to_string());
    write_session(&path, &session).await?;
    Ok(())
}

pub async fn clear_draft(sessions_dir: &Path, session_id: &str) -> Result<(), SessionError> {
    save_draft(sessions_dir, session_id, "").await
}

pub async fn save_scenario(
    sessions_dir: &Path,
    session_id: &str,
    content: &str,
) -> Result<(), SessionError> {
    if !valid_session_id(session_id) {
        return Err(SessionError::InvalidId(session_id.to_string()));
    }
    let lock = get_session_lock(session_id);
    let _guard = lock.lock().await;
    let path = session_path(sessions_dir, session_id);
    let mut session = read_session(&path, session_id).await?;
    session.scenario = Some(content.to_string());
    write_session(&path, &session).await?;
    Ok(())
}

/// Save thread memory + stamp it with the timestamp of the last message
/// included in the summary. `summarized_through` MUST be the last-message
/// timestamp, not "now" — using "now" would silently skip messages written
/// during the LLM call.
pub async fn save_thread_memory(
    sessions_dir: &Path,
    session_id: &str,
    content: &str,
    summarized_through: &str,
) -> Result<(), SessionError> {
    if !valid_session_id(session_id) {
        return Err(SessionError::InvalidId(session_id.to_string()));
    }
    let lock = get_session_lock(session_id);
    let _guard = lock.lock().await;
    let path = session_path(sessions_dir, session_id);
    let mut session = read_session(&path, session_id).await?;
    session.thread_memory = content.to_string();
    session.thread_memory_updated_at = summarized_through.to_string();
    write_session(&path, &session).await?;
    Ok(())
}

/// Merge-save a per-thread override for thread-memory settings. Only fields set
/// to `Some` in `patch` are written; other fields in the existing override are
/// preserved.
pub async fn save_thread_memory_settings_override(
    sessions_dir: &Path,
    session_id: &str,
    patch: ThreadMemorySettings,
) -> Result<(), SessionError> {
    if !valid_session_id(session_id) {
        return Err(SessionError::InvalidId(session_id.to_string()));
    }
    let lock = get_session_lock(session_id);
    let _guard = lock.lock().await;
    let path = session_path(sessions_dir, session_id);
    let mut session = read_session(&path, session_id).await?;
    let mut merged = session.thread_memory_settings.unwrap_or_default();
    if patch.interval_minutes.is_some() {
        merged.interval_minutes = patch.interval_minutes;
    }
    if patch.message_floor.is_some() {
        merged.message_floor = patch.message_floor;
    }
    if patch.size_limit.is_some() {
        merged.size_limit = patch.size_limit;
    }
    session.thread_memory_settings = Some(merged);
    write_session(&path, &session).await?;
    Ok(())
}

/// Remove the per-thread override, reverting to persona/global defaults.
pub async fn clear_thread_memory_settings_override(
    sessions_dir: &Path,
    session_id: &str,
) -> Result<(), SessionError> {
    if !valid_session_id(session_id) {
        return Err(SessionError::InvalidId(session_id.to_string()));
    }
    let lock = get_session_lock(session_id);
    let _guard = lock.lock().await;
    let path = session_path(sessions_dir, session_id);
    let mut session = read_session(&path, session_id).await?;
    if session.thread_memory_settings.is_none() {
        return Ok(());
    }
    session.thread_memory_settings = None;
    write_session(&path, &session).await?;
    Ok(())
}

/// Remove the last assistant message in preparation for a retry. Returns the
/// last user message content + the session data after modification.
///
/// `InvalidState` covers the cases where there's nothing meaningful to remove:
/// fewer than 2 messages, last message isn't an assistant, or no preceding user.
pub async fn remove_last_assistant_message(
    sessions_dir: &Path,
    session_id: &str,
) -> Result<(String, Session), SessionError> {
    if !valid_session_id(session_id) {
        return Err(SessionError::InvalidId(session_id.to_string()));
    }
    let lock = get_session_lock(session_id);
    let _guard = lock.lock().await;
    let path = session_path(sessions_dir, session_id);
    let mut session = read_session(&path, session_id).await?;

    if session.messages.len() < 2 {
        return Err(SessionError::InvalidState("fewer than two messages"));
    }
    if session.messages.last().map(|m| m.role) != Some(Role::Assistant) {
        return Err(SessionError::InvalidState("last message is not assistant"));
    }
    session.messages.pop();
    let last_role = session.messages.last().map(|m| m.role);
    if last_role != Some(Role::User) {
        return Err(SessionError::InvalidState(
            "no preceding user message after pop",
        ));
    }
    let user_content = session
        .messages
        .last()
        .map(|m| m.content.clone())
        .unwrap_or_default();

    write_session(&path, &session).await?;
    Ok((user_content, session))
}

/// Replace the content of the last user message.
pub async fn update_last_user_message(
    sessions_dir: &Path,
    session_id: &str,
    new_content: &str,
) -> Result<(), SessionError> {
    if !valid_session_id(session_id) {
        return Err(SessionError::InvalidId(session_id.to_string()));
    }
    let lock = get_session_lock(session_id);
    let _guard = lock.lock().await;
    let path = session_path(sessions_dir, session_id);
    let mut session = read_session(&path, session_id).await?;
    let Some(idx) = session
        .messages
        .iter()
        .rposition(|m| m.role == Role::User)
    else {
        return Err(SessionError::InvalidState("no user message to update"));
    };
    session.messages[idx].content = new_content.to_string();
    write_session(&path, &session).await?;
    Ok(())
}

/// Rewrite the `persona` field of every session that references `old_name` to
/// use `new_name`. Called from `PersonaManager::rename_persona`. Locks each
/// session individually so concurrent writes on other sessions aren't blocked.
pub async fn update_persona_across_sessions(
    sessions_dir: &Path,
    old_name: &str,
    new_name: &str,
) {
    let mut entries = match tokio::fs::read_dir(sessions_dir).await {
        Ok(e) => e,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => return,
        Err(err) => {
            tracing::error!(?sessions_dir, error = %err, "update_persona_across_sessions: read_dir failed");
            return;
        }
    };

    while let Ok(Some(entry)) = entries.next_entry().await {
        let filename = entry.file_name().to_string_lossy().to_string();
        if !filename.ends_with(".json") {
            continue;
        }
        let path = entry.path();
        let lock = get_session_lock(&filename);
        let _guard = lock.lock().await;
        let mut session = match read_session(&path, &filename).await {
            Ok(s) => s,
            Err(err) => {
                tracing::warn!(session_id = %filename, error = %err, "update_persona_across_sessions: read failed, skipping");
                continue;
            }
        };
        if session.persona != old_name {
            continue;
        }
        session.persona = new_name.to_string();
        if let Err(err) = write_session(&path, &session).await {
            tracing::error!(session_id = %filename, error = %err, "update_persona_across_sessions write failed");
        }
    }
}

/// Fork a thread into a new roleplay session. Copies persona, messages,
/// thread_memory, and thread_memory_updated_at. Resets title; does not copy
/// pinned, draft, scenario, or thread_memory_settings. Source is untouched.
pub async fn fork_to_roleplay(
    sessions_dir: &Path,
    source_session_id: &str,
) -> Result<String, SessionError> {
    if !valid_session_id(source_session_id) {
        return Err(SessionError::InvalidId(source_session_id.to_string()));
    }

    let source = {
        let lock = get_session_lock(source_session_id);
        let _guard = lock.lock().await;
        read_session(
            &session_path(sessions_dir, source_session_id),
            source_session_id,
        )
        .await?
    };

    // Generate a collision-free id. Second-precision timestamps can collide if
    // the user forked twice in the same second.
    let mut new_id = generate_session_id();
    if new_id == source_session_id
        || tokio::fs::try_exists(&session_path(sessions_dir, &new_id))
            .await
            .unwrap_or(false)
    {
        let base = new_id.trim_end_matches(".json").to_string();
        let mut found = None;
        for i in 1..100 {
            let candidate = format!("{base}_{i}.json");
            if candidate == source_session_id {
                continue;
            }
            if !tokio::fs::try_exists(&session_path(sessions_dir, &candidate))
                .await
                .unwrap_or(false)
            {
                found = Some(candidate);
                break;
            }
        }
        new_id = found.ok_or(SessionError::InvalidState(
            "no collision-free id in 100 attempts",
        ))?;
    }

    let new_session = Session {
        title: "New Chat".to_string(),
        title_locked: None,
        persona: source.persona,
        mode: Mode::Roleplay,
        messages: source.messages,
        draft: None,
        pinned: None,
        scenario: None,
        thread_memory: source.thread_memory,
        thread_memory_updated_at: source.thread_memory_updated_at,
        thread_memory_settings: None,
    };

    let lock = get_session_lock(&new_id);
    let _guard = lock.lock().await;
    write_session(&session_path(sessions_dir, &new_id), &new_session).await?;
    Ok(new_id)
}
