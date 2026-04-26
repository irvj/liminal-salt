//! `/memory/*` handlers — GET page + memory operations (update/wipe/modify/
//! seed/save-settings/status). All LLM work is dispatched to `MemoryWorker`;
//! this module only shapes the HTTP and template layer.

use axum::{
    Form, Json,
    extract::{Multipart, Query, State},
    http::{HeaderMap, StatusCode},
    response::{Html, IntoResponse, Redirect, Response},
};
use serde::{Deserialize, Serialize};
use tera::Context;
use tower_sessions::Session;

use crate::{
    AppState,
    handlers::status::persona_status,
    services::{
        config, memory,
        memory_worker::State as UpdateState,
        persona::{self, ThreadMemoryDefaults},
        thread_memory::{
            DEFAULT_THREAD_MEMORY_INTERVAL_MINUTES as DEFAULT_INTERVAL_MINUTES,
            DEFAULT_THREAD_MEMORY_MESSAGE_FLOOR as DEFAULT_MESSAGE_FLOOR,
            DEFAULT_THREAD_MEMORY_SIZE as DEFAULT_SIZE_LIMIT,
        },
    },
};

// =============================================================================
// GET /memory/
// =============================================================================

#[derive(Deserialize)]
pub struct MemoryQuery {
    #[serde(default)]
    pub persona: Option<String>,
}

pub async fn view(
    State(state): State<AppState>,
    session: Session,
    headers: HeaderMap,
    Query(q): Query<MemoryQuery>,
) -> Response {
    let personas = persona::list_personas(&state.data_dir).await;
    let cfg = config::load_config(&state.data_dir).await;

    let selected = q
        .persona
        .filter(|p| !p.is_empty() && personas.contains(p))
        .or_else(|| Some(cfg.default_persona.clone()).filter(|p| personas.contains(p)))
        .or_else(|| personas.first().cloned())
        .unwrap_or_default();

    let memory_updating =
        state.memory.get_update_status(&selected).state == UpdateState::Running;

    render_memory(
        &state,
        &session,
        &headers,
        &selected,
        ViewOpts {
            memory_updating,
            ..ViewOpts::default()
        },
    )
    .await
}

// =============================================================================
// POST /memory/update/
// =============================================================================

#[derive(Deserialize)]
pub struct PersonaForm {
    #[serde(default)]
    pub persona: String,
}

pub async fn update(
    State(state): State<AppState>,
    session: Session,
    headers: HeaderMap,
    Form(form): Form<PersonaForm>,
) -> Response {
    let selected = resolve_persona(&state, form.persona).await;

    let cfg = config::load_config(&state.data_dir).await;
    if cfg.api_key.is_empty() {
        return render_memory(
            &state,
            &session,
            &headers,
            &selected,
            ViewOpts {
                error: Some("API key not configured.".into()),
                ..ViewOpts::default()
            },
        )
        .await;
    }

    let started = state.memory.start_manual_update(state.clone(), selected.clone());
    render_after_start(&state, &session, &headers, &selected, started).await
}

// =============================================================================
// POST /memory/wipe/
// =============================================================================

pub async fn wipe(
    State(state): State<AppState>,
    session: Session,
    headers: HeaderMap,
    Form(form): Form<PersonaForm>,
) -> Response {
    let selected = resolve_persona(&state, form.persona).await;
    if let Err(err) = memory::delete_memory(&state.data_dir, &selected).await {
        tracing::warn!(persona = %selected, error = %err, "memory delete failed");
    }

    if is_htmx(&headers) {
        render_memory(
            &state,
            &session,
            &headers,
            &selected,
            ViewOpts {
                success: Some("Memory wiped successfully".into()),
                just_updated: true,
                ..ViewOpts::default()
            },
        )
        .await
    } else {
        // Non-HTMX path isn't exercised by the UI (`wipeMemoryWithConfirm`
        // always sends `HX-Request: true`). Redirect to the memory page
        // without a query flash — good enough.
        Redirect::to("/memory/").into_response()
    }
}

// =============================================================================
// POST /memory/modify/
// =============================================================================

#[derive(Deserialize)]
pub struct ModifyForm {
    #[serde(default)]
    pub persona: String,
    #[serde(default)]
    pub command: String,
}

pub async fn modify(
    State(state): State<AppState>,
    session: Session,
    headers: HeaderMap,
    Form(form): Form<ModifyForm>,
) -> Response {
    let command = form.command.trim().to_string();
    if command.is_empty() {
        return (StatusCode::BAD_REQUEST, "empty command").into_response();
    }

    let selected = resolve_persona(&state, form.persona).await;

    let cfg = config::load_config(&state.data_dir).await;
    if cfg.api_key.is_empty() {
        return (StatusCode::INTERNAL_SERVER_ERROR, "Configuration not found").into_response();
    }

    let started = state
        .memory
        .start_modify_update(state.clone(), selected.clone(), command);
    render_after_start(&state, &session, &headers, &selected, started).await
}

// =============================================================================
// POST /memory/seed/
// =============================================================================

pub async fn seed(
    State(state): State<AppState>,
    session: Session,
    headers: HeaderMap,
    mut multipart: Multipart,
) -> Response {
    let mut persona_name = String::new();
    let mut filename: Option<String> = None;
    let mut body: Vec<u8> = Vec::new();

    while let Ok(Some(mut field)) = multipart.next_field().await {
        match field.name() {
            Some("persona") => persona_name = field.text().await.unwrap_or_default(),
            Some("file") => {
                filename = field.file_name().map(|s| s.to_string());
                while let Ok(Some(chunk)) = field.chunk().await {
                    body.extend_from_slice(&chunk);
                }
            }
            _ => {}
        }
    }

    let selected = resolve_persona(&state, persona_name).await;

    let Some(name) = filename.filter(|n| !n.is_empty()) else {
        return render_or_error(
            &state,
            &session,
            &headers,
            &selected,
            "No file provided.",
            StatusCode::BAD_REQUEST,
        )
        .await;
    };

    let lower = name.to_ascii_lowercase();
    if !(lower.ends_with(".md") || lower.ends_with(".txt")) {
        return render_or_error(
            &state,
            &session,
            &headers,
            &selected,
            "Only .md and .txt files are accepted.",
            StatusCode::BAD_REQUEST,
        )
        .await;
    }

    let cfg = config::load_config(&state.data_dir).await;
    if cfg.api_key.is_empty() {
        return (StatusCode::INTERNAL_SERVER_ERROR, "Configuration not found").into_response();
    }

    // Lossy decode — matches Python's `errors='replace'`.
    let seed_content = String::from_utf8_lossy(&body).into_owned();

    let started = state
        .memory
        .start_seed_update(state.clone(), selected.clone(), seed_content);
    render_after_start(&state, &session, &headers, &selected, started).await
}

// =============================================================================
// POST /memory/save-settings/ (AJAX JSON response)
// =============================================================================

#[derive(Deserialize)]
pub struct SettingsForm {
    #[serde(default)]
    pub persona: String,
    // All numeric fields come as strings because JS FormData coerces numbers
    // to strings; parsing + clamping happens server-side.
    #[serde(default)]
    pub user_history_max_threads: String,
    #[serde(default)]
    pub user_history_messages_per_thread: String,
    #[serde(default)]
    pub memory_size_limit: String,
    #[serde(default)]
    pub auto_memory_interval: String,
    #[serde(default)]
    pub auto_memory_message_floor: String,
}

pub async fn save_settings(State(state): State<AppState>, mut multipart: Multipart) -> Response {
    // Settings arrive via JS `FormData` which encodes multipart, not urlencoded.
    let mut form = SettingsForm::default_form();
    while let Ok(Some(field)) = multipart.next_field().await {
        let Some(name) = field.name().map(|s| s.to_string()) else { continue };
        let value = field.text().await.unwrap_or_default();
        match name.as_str() {
            "persona" => form.persona = value,
            "user_history_max_threads" => form.user_history_max_threads = value,
            "user_history_messages_per_thread" => form.user_history_messages_per_thread = value,
            "memory_size_limit" => form.memory_size_limit = value,
            "auto_memory_interval" => form.auto_memory_interval = value,
            "auto_memory_message_floor" => form.auto_memory_message_floor = value,
            _ => {}
        }
    }

    let selected = resolve_persona(&state, form.persona).await;
    if selected.is_empty() {
        return (StatusCode::BAD_REQUEST, "no persona").into_response();
    }

    let user_history_max_threads =
        parse_clamp_u32(&form.user_history_max_threads, 0, 100, 0);
    let user_history_messages_per_thread =
        parse_clamp_u32(&form.user_history_messages_per_thread, 0, 10_000, 0);
    let memory_size_limit = parse_clamp_u32(&form.memory_size_limit, 0, 100_000, 8_000);
    let auto_memory_interval = parse_auto_interval(&form.auto_memory_interval);
    let auto_memory_message_floor =
        parse_clamp_u32(&form.auto_memory_message_floor, 1, 1_000, 10);

    let mut persona_cfg = persona::load_persona_config(&state.data_dir, &selected).await;
    persona_cfg.user_history_max_threads = Some(user_history_max_threads);
    persona_cfg.user_history_messages_per_thread = Some(user_history_messages_per_thread);
    persona_cfg.memory_size_limit = Some(memory_size_limit);
    persona_cfg.auto_memory_interval = Some(auto_memory_interval);
    persona_cfg.auto_memory_message_floor = Some(auto_memory_message_floor);

    if let Err(err) = persona::save_persona_config(&state.data_dir, &selected, &persona_cfg).await
    {
        tracing::error!(error = %err, persona = %selected, "save memory settings failed");
        return (persona_status(&err), "save failed").into_response();
    }

    Json(serde_json::json!({"success": true})).into_response()
}

/// Clamp with "0 = disabled, otherwise ≥5 minutes, ≤1440." Matches Python's
/// `save_memory_settings` handler.
fn parse_auto_interval(s: &str) -> u32 {
    let parsed = s.parse::<i64>().unwrap_or(0);
    if parsed <= 0 {
        0
    } else {
        parsed.clamp(5, 1440) as u32
    }
}

fn parse_clamp_u32(s: &str, min: u32, max: u32, fallback: u32) -> u32 {
    match s.parse::<i64>() {
        Ok(v) => v.clamp(min as i64, max as i64) as u32,
        Err(_) => fallback,
    }
}

impl SettingsForm {
    fn default_form() -> Self {
        Self {
            persona: String::new(),
            user_history_max_threads: String::new(),
            user_history_messages_per_thread: String::new(),
            memory_size_limit: String::new(),
            auto_memory_interval: String::new(),
            auto_memory_message_floor: String::new(),
        }
    }
}

// =============================================================================
// GET /memory/update-status/
// =============================================================================

#[derive(Deserialize)]
pub struct StatusQuery {
    #[serde(default)]
    pub persona: String,
}

pub async fn update_status(
    State(state): State<AppState>,
    Query(q): Query<StatusQuery>,
) -> Response {
    let status = state.memory.get_update_status(&q.persona);
    Json(status).into_response()
}

// =============================================================================
// POST /memory/thread-memory-defaults/save/  (and /reset/)
// =============================================================================
//
// Persona-level defaults for the per-thread memory summary. Writes to persona
// config (owned by `persona.rs`); URL is memory-namespaced because the user
// edits these from the Memory tab.

#[derive(Deserialize)]
pub struct ThreadMemoryDefaultsForm {
    persona: String,
    /// Values arrive as integers; blank fields become None.
    #[serde(default)]
    interval_minutes: Option<i64>,
    #[serde(default)]
    message_floor: Option<i64>,
    #[serde(default)]
    size_limit: Option<i64>,
}

#[derive(Serialize)]
pub struct ThreadMemoryDefaultsResponse {
    effective: EffectiveThreadMemoryDefaults,
    has_override: bool,
}

#[derive(Serialize)]
pub struct EffectiveThreadMemoryDefaults {
    interval_minutes: u32,
    message_floor: u32,
    size_limit: u32,
}

fn clamp_thread_interval(n: i64) -> u32 {
    if n <= 0 { 0 } else { n.clamp(5, 1440) as u32 }
}
fn clamp_thread_message_floor(n: i64) -> u32 {
    n.clamp(1, 1000) as u32
}
fn clamp_thread_size_limit(n: i64) -> u32 {
    n.clamp(0, 100_000) as u32
}

pub async fn save_thread_memory_defaults(
    State(state): State<AppState>,
    Form(form): Form<ThreadMemoryDefaultsForm>,
) -> Response {
    let mut cfg = persona::load_persona_config(&state.data_dir, &form.persona).await;

    let interval = form.interval_minutes.map(clamp_thread_interval);
    let floor = form.message_floor.map(clamp_thread_message_floor);
    let size = form.size_limit.map(clamp_thread_size_limit);

    // Only persist fields that differ from the global defaults — keeps the
    // "Custom" badge from lighting up when the user types in the default.
    let settings = ThreadMemoryDefaults {
        interval_minutes: interval.filter(|v| *v != DEFAULT_INTERVAL_MINUTES),
        message_floor: floor.filter(|v| *v != DEFAULT_MESSAGE_FLOOR),
        size_limit: size.filter(|v| *v != DEFAULT_SIZE_LIMIT),
    };
    cfg.default_thread_memory_settings = if settings.interval_minutes.is_none()
        && settings.message_floor.is_none()
        && settings.size_limit.is_none()
    {
        None
    } else {
        Some(settings)
    };

    if let Err(err) = persona::save_persona_config(&state.data_dir, &form.persona, &cfg).await {
        tracing::error!(error = %err, persona = %form.persona, "save thread memory defaults failed");
        return (persona_status(&err), "save failed").into_response();
    }

    let has_override = cfg.default_thread_memory_settings.is_some();
    let eff = cfg.default_thread_memory_settings.unwrap_or_default();
    Json(ThreadMemoryDefaultsResponse {
        effective: EffectiveThreadMemoryDefaults {
            interval_minutes: eff.interval_minutes.unwrap_or(DEFAULT_INTERVAL_MINUTES),
            message_floor: eff.message_floor.unwrap_or(DEFAULT_MESSAGE_FLOOR),
            size_limit: eff.size_limit.unwrap_or(DEFAULT_SIZE_LIMIT),
        },
        has_override,
    })
    .into_response()
}

#[derive(Deserialize)]
pub struct ResetThreadMemoryDefaultsForm {
    persona: String,
}

pub async fn reset_thread_memory_defaults(
    State(state): State<AppState>,
    Form(form): Form<ResetThreadMemoryDefaultsForm>,
) -> Response {
    let mut cfg = persona::load_persona_config(&state.data_dir, &form.persona).await;
    cfg.default_thread_memory_settings = None;

    if let Err(err) = persona::save_persona_config(&state.data_dir, &form.persona, &cfg).await {
        tracing::error!(error = %err, persona = %form.persona, "reset thread memory defaults failed");
        return (persona_status(&err), "reset failed").into_response();
    }

    Json(ThreadMemoryDefaultsResponse {
        effective: EffectiveThreadMemoryDefaults {
            interval_minutes: DEFAULT_INTERVAL_MINUTES,
            message_floor: DEFAULT_MESSAGE_FLOOR,
            size_limit: DEFAULT_SIZE_LIMIT,
        },
        has_override: false,
    })
    .into_response()
}

// =============================================================================
// Helpers — context building + rendering
// =============================================================================

#[derive(Default, Clone, Serialize)]
struct ViewOpts {
    success: Option<String>,
    error: Option<String>,
    just_updated: bool,
    memory_updating: bool,
}

async fn resolve_persona(state: &AppState, submitted: String) -> String {
    if !submitted.is_empty() {
        return submitted;
    }
    let cfg = config::load_config(&state.data_dir).await;
    cfg.default_persona
}

pub(crate) fn is_htmx(headers: &HeaderMap) -> bool {
    headers
        .get("HX-Request")
        .map(|v| v.as_bytes() == b"true")
        .unwrap_or(false)
}

/// Common post-dispatch response: if the mutex was already held, surface the
/// "already running" error; otherwise render with the spinner state on.
async fn render_after_start(
    state: &AppState,
    session: &Session,
    headers: &HeaderMap,
    selected: &str,
    started: bool,
) -> Response {
    if !started {
        return render_memory(
            state,
            session,
            headers,
            selected,
            ViewOpts {
                error: Some("Memory update already in progress.".into()),
                ..ViewOpts::default()
            },
        )
        .await;
    }
    render_memory(
        state,
        session,
        headers,
        selected,
        ViewOpts {
            memory_updating: true,
            ..ViewOpts::default()
        },
    )
    .await
}

async fn render_or_error(
    state: &AppState,
    session: &Session,
    headers: &HeaderMap,
    selected: &str,
    message: &str,
    fallback_status: StatusCode,
) -> Response {
    if is_htmx(headers) {
        render_memory(
            state,
            session,
            headers,
            selected,
            ViewOpts {
                error: Some(message.to_string()),
                ..ViewOpts::default()
            },
        )
        .await
    } else {
        (fallback_status, message.to_string()).into_response()
    }
}

async fn render_memory(
    state: &AppState,
    session: &Session,
    headers: &HeaderMap,
    selected: &str,
    opts: ViewOpts,
) -> Response {
    let personas = persona::list_personas(&state.data_dir).await;
    let cfg = config::load_config(&state.data_dir).await;

    let memory_content = if selected.is_empty() {
        String::new()
    } else {
        memory::get_memory_content(&state.data_dir, selected).await
    };

    // The `memoryView` Alpine component does `new Date(parseInt(timestamp) * 1000)`
    // — it expects Unix seconds, not an ISO string.
    let last_update = if selected.is_empty() {
        String::new()
    } else {
        memory::get_mtime_secs(&state.data_dir, selected)
            .await
            .map(|s| s.to_string())
            .unwrap_or_default()
    };

    let persona_cfg = if selected.is_empty() {
        persona::PersonaConfig::default()
    } else {
        persona::load_persona_config(&state.data_dir, selected).await
    };

    let mut ctx = super::chat::base_chat_context(state, session).await;
    ctx.insert("page", "memory");
    ctx.insert("show_home", &false);
    ctx.insert("selected_persona", &selected);
    ctx.insert("personas", &personas);
    ctx.insert("default_persona", &cfg.default_persona);
    ctx.insert("model", &cfg.model);
    ctx.insert("memory_content", &memory_content);
    ctx.insert("last_update", &last_update);
    ctx.insert("just_updated", &opts.just_updated);
    ctx.insert("memory_updating", &opts.memory_updating);
    ctx.insert("success", &opts.success);
    ctx.insert("error", &opts.error);
    ctx.insert(
        "user_history_max_threads",
        &persona_cfg.user_history_max_threads.unwrap_or(0),
    );
    ctx.insert(
        "user_history_messages_per_thread",
        &persona_cfg.user_history_messages_per_thread.unwrap_or(0),
    );
    ctx.insert(
        "memory_size_limit",
        &persona_cfg.memory_size_limit.unwrap_or(8000),
    );
    ctx.insert(
        "auto_memory_interval",
        &persona_cfg.auto_memory_interval.unwrap_or(0),
    );
    ctx.insert(
        "auto_memory_message_floor",
        &persona_cfg.auto_memory_message_floor.unwrap_or(10),
    );

    // Per-chat (thread) memory defaults — controls live on the Memory tab.
    let thread_defaults = persona_cfg.default_thread_memory_settings.unwrap_or_default();
    ctx.insert(
        "persona_thread_interval_minutes",
        &thread_defaults.interval_minutes.unwrap_or(DEFAULT_INTERVAL_MINUTES),
    );
    ctx.insert(
        "persona_thread_message_floor",
        &thread_defaults.message_floor.unwrap_or(DEFAULT_MESSAGE_FLOOR),
    );
    ctx.insert(
        "persona_thread_size_limit",
        &thread_defaults.size_limit.unwrap_or(DEFAULT_SIZE_LIMIT),
    );
    let has_thread_memory_override = thread_defaults.interval_minutes.is_some()
        || thread_defaults.message_floor.is_some()
        || thread_defaults.size_limit.is_some();
    ctx.insert("persona_has_thread_memory_defaults", &has_thread_memory_override);

    let htmx = is_htmx(headers);
    ctx.insert("is_htmx", &htmx);

    let template = if htmx {
        "memory/memory_main.html"
    } else {
        "chat/chat.html"
    };
    render(&state.tera, template, &ctx).await
}

async fn render(
    tera: &tera::Tera,
    template: &str,
    ctx: &Context,
) -> Response {
    match tera.render(template, ctx) {
        Ok(html) => Html(html).into_response(),
        Err(err) => {
            tracing::error!(template, error = ?err, "memory render failed");
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("render failed: {err:?}"),
            )
                .into_response()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_clamp_u32_bounds_and_fallback() {
        // Normal in-range.
        assert_eq!(parse_clamp_u32("50", 0, 100, 0), 50);
        // Above max.
        assert_eq!(parse_clamp_u32("500", 0, 100, 0), 100);
        // Negative.
        assert_eq!(parse_clamp_u32("-5", 0, 100, 0), 0);
        // Below min.
        assert_eq!(parse_clamp_u32("5", 10, 100, 10), 10);
        // Parse error returns fallback, not clamped.
        assert_eq!(parse_clamp_u32("", 0, 100, 42), 42);
        assert_eq!(parse_clamp_u32("abc", 0, 100, 42), 42);
    }

    #[test]
    fn parse_auto_interval_zero_means_disabled() {
        // Explicit 0 → disabled.
        assert_eq!(parse_auto_interval("0"), 0);
        // Negative → disabled.
        assert_eq!(parse_auto_interval("-5"), 0);
        // 1..=4 → clamped to the 5-minute minimum.
        assert_eq!(parse_auto_interval("3"), 5);
        // In-range.
        assert_eq!(parse_auto_interval("60"), 60);
        // Above max clamps to 1440.
        assert_eq!(parse_auto_interval("9999"), 1440);
        // Empty/garbage → 0 (treated as absent/disabled).
        assert_eq!(parse_auto_interval(""), 0);
        assert_eq!(parse_auto_interval("abc"), 0);
    }
}
