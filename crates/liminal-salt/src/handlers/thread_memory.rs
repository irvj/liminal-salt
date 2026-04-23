//! `/session/thread-memory/*` handlers — manual update dispatch, status poll,
//! per-thread settings save + reset. All JSON AJAX endpoints (no HTMX
//! partials). LLM work goes through `MemoryWorker`.

use axum::{
    Form, Json,
    extract::{Query, State},
    http::StatusCode,
    response::{IntoResponse, Response},
};
use serde::Deserialize;
use tower_sessions::Session;

use crate::{
    AppState,
    middleware::session_state,
    services::{
        config,
        memory_worker::UpdateSource,
        persona,
        session::{self as session_svc, SessionError},
        thread_memory::{
            EffectiveThreadMemorySettings, resolve_persona_defaults, resolve_settings,
        },
    },
};

// =============================================================================
// POST /session/thread-memory/update/
// =============================================================================

#[derive(Deserialize, Default)]
pub struct UpdateForm {
    #[serde(default)]
    pub session_id: String,
}

pub async fn update(
    State(state): State<AppState>,
    session: Session,
    Form(form): Form<UpdateForm>,
) -> Response {
    let session_id = resolve_session_id(&session, form.session_id).await;
    if session_id.is_empty() {
        return json_error(StatusCode::BAD_REQUEST, "No active session.");
    }

    let cfg = config::load_config(&state.data_dir).await;
    if cfg.api_key.is_empty() {
        return json_error(StatusCode::BAD_REQUEST, "API key not configured.");
    }

    let started = state.memory.start_thread_memory_update(
        state.clone(),
        session_id,
        UpdateSource::Manual,
    );
    if !started {
        return (
            StatusCode::CONFLICT,
            Json(serde_json::json!({"state": "already_running"})),
        )
            .into_response();
    }
    (
        StatusCode::ACCEPTED,
        Json(serde_json::json!({"state": "started"})),
    )
        .into_response()
}

// =============================================================================
// GET /session/thread-memory/status/
// =============================================================================

#[derive(Deserialize, Default)]
pub struct StatusQuery {
    #[serde(default)]
    pub session_id: String,
}

pub async fn status(
    State(state): State<AppState>,
    session: Session,
    Query(q): Query<StatusQuery>,
) -> Response {
    let session_id = resolve_session_id(&session, q.session_id).await;
    if session_id.is_empty() {
        return json_error(StatusCode::BAD_REQUEST, "No active session.");
    }

    let status = state.memory.get_thread_update_status(&session_id);
    let (memory, updated_at) = session_svc::load_session(&state.sessions_dir, &session_id)
        .await
        .map(|s| (s.thread_memory, s.thread_memory_updated_at))
        .unwrap_or_default();

    // Extend the status struct's JSON with the memory/updated_at fields so
    // the frontend can refresh its view after a background update completes.
    let mut body = serde_json::to_value(&status).unwrap_or_else(|_| serde_json::json!({}));
    if let Some(obj) = body.as_object_mut() {
        obj.insert("memory".into(), serde_json::Value::String(memory));
        obj.insert(
            "updated_at".into(),
            serde_json::Value::String(updated_at),
        );
    }
    Json(body).into_response()
}

// =============================================================================
// POST /session/thread-memory/settings/save/
// =============================================================================

#[derive(Deserialize, Default)]
pub struct SettingsForm {
    #[serde(default)]
    pub session_id: String,
    // Optional so callers can send a partial patch. String to mirror Python's
    // "only look at fields that are present" pattern — empty == absent.
    #[serde(default)]
    pub interval_minutes: Option<String>,
    #[serde(default)]
    pub message_floor: Option<String>,
    #[serde(default)]
    pub size_limit: Option<String>,
}

pub async fn settings_save(
    State(state): State<AppState>,
    session: Session,
    Form(form): Form<SettingsForm>,
) -> Response {
    let session_id = resolve_session_id(&session, form.session_id).await;
    if session_id.is_empty() {
        return json_error(StatusCode::BAD_REQUEST, "No active session.");
    }

    let session_data = match session_svc::load_session(&state.sessions_dir, &session_id).await {
        Ok(s) => s,
        Err(SessionError::NotFound(_) | SessionError::InvalidId(_)) => {
            return json_error(StatusCode::NOT_FOUND, "Session not found.");
        }
        Err(err) => {
            tracing::error!(session_id, error = %err, "settings_save: load failed");
            return json_error(StatusCode::INTERNAL_SERVER_ERROR, "Session load failed.");
        }
    };

    // Parse + validate what the caller actually submitted.
    let interval_minutes = match parse_interval_override(form.interval_minutes.as_deref()) {
        Ok(v) => v,
        Err(err) => return json_error(StatusCode::BAD_REQUEST, err),
    };
    let message_floor = match parse_u32_override(form.message_floor.as_deref(), 1, 1_000) {
        Ok(v) => v,
        Err(err) => return json_error(StatusCode::BAD_REQUEST, err),
    };
    let size_limit = match parse_u32_override(form.size_limit.as_deref(), 0, 100_000) {
        Ok(v) => v,
        Err(err) => return json_error(StatusCode::BAD_REQUEST, err),
    };

    if interval_minutes.is_none() && message_floor.is_none() && size_limit.is_none() {
        return json_error(StatusCode::BAD_REQUEST, "No settings provided.");
    }

    let patch = session_svc::ThreadMemorySettings {
        interval_minutes,
        message_floor,
        size_limit,
    };

    // Merge existing override + patch, compare to persona/global defaults. If
    // the merged effective values all equal defaults, clear the override
    // instead of persisting a no-op that lights up the "Custom" badge.
    let persona_name = session_data.persona.clone();
    let persona_cfg = persona::load_persona_config(&state.data_dir, &persona_name).await;
    let defaults = resolve_persona_defaults(&persona_cfg);

    let existing = session_data
        .thread_memory_settings
        .clone()
        .unwrap_or_default();
    let merged = session_svc::ThreadMemorySettings {
        interval_minutes: patch.interval_minutes.or(existing.interval_minutes),
        message_floor: patch.message_floor.or(existing.message_floor),
        size_limit: patch.size_limit.or(existing.size_limit),
    };
    let effective_if_saved = EffectiveThreadMemorySettings {
        interval_minutes: merged.interval_minutes.unwrap_or(defaults.interval_minutes),
        message_floor: merged.message_floor.unwrap_or(defaults.message_floor),
        size_limit: merged.size_limit.unwrap_or(defaults.size_limit),
    };

    if effective_if_saved == defaults {
        if let Err(err) =
            session_svc::clear_thread_memory_settings_override(&state.sessions_dir, &session_id)
                .await
        {
            tracing::warn!(session_id, error = %err, "clear override failed");
        }
    } else if let Err(err) = session_svc::save_thread_memory_settings_override(
        &state.sessions_dir,
        &session_id,
        patch,
    )
    .await
    {
        let status = match err {
            SessionError::NotFound(_) | SessionError::InvalidId(_) => StatusCode::NOT_FOUND,
            _ => StatusCode::INTERNAL_SERVER_ERROR,
        };
        return json_error(status, "Session not found.");
    }

    finish_with_resolved(&state, &session_id).await
}

// =============================================================================
// POST /session/thread-memory/settings/reset/
// =============================================================================

pub async fn settings_reset(
    State(state): State<AppState>,
    session: Session,
    Form(form): Form<UpdateForm>,
) -> Response {
    let session_id = resolve_session_id(&session, form.session_id).await;
    if session_id.is_empty() {
        return json_error(StatusCode::BAD_REQUEST, "No active session.");
    }

    match session_svc::clear_thread_memory_settings_override(&state.sessions_dir, &session_id)
        .await
    {
        Ok(()) => {}
        Err(SessionError::NotFound(_) | SessionError::InvalidId(_)) => {
            return json_error(StatusCode::NOT_FOUND, "Session not found.");
        }
        Err(err) => {
            tracing::error!(session_id, error = %err, "settings_reset: clear failed");
            return json_error(StatusCode::INTERNAL_SERVER_ERROR, "Clear failed.");
        }
    }

    finish_with_resolved(&state, &session_id).await
}

// =============================================================================
// Helpers
// =============================================================================

async fn resolve_session_id(session: &Session, submitted: String) -> String {
    if !submitted.is_empty() {
        return submitted;
    }
    session_state::current_session_id(session)
        .await
        .unwrap_or_default()
}

fn json_error(status: StatusCode, message: &str) -> Response {
    (status, Json(serde_json::json!({"error": message}))).into_response()
}

/// Parse a per-thread interval override. `None` / empty means "not submitted."
/// `0` persists as an explicit "auto disabled for this thread" override.
/// `1..=4` gets clamped to 5 (matches Python's "min 5 minutes when enabled").
fn parse_interval_override(input: Option<&str>) -> Result<Option<u32>, &'static str> {
    let Some(s) = input else { return Ok(None) };
    if s.is_empty() {
        return Ok(None);
    }
    let v = s
        .parse::<i64>()
        .map_err(|_| "Invalid setting value.")?;
    if v <= 0 {
        Ok(Some(0))
    } else {
        Ok(Some(v.clamp(5, 1440) as u32))
    }
}

fn parse_u32_override(
    input: Option<&str>,
    min: u32,
    max: u32,
) -> Result<Option<u32>, &'static str> {
    let Some(s) = input else { return Ok(None) };
    if s.is_empty() {
        return Ok(None);
    }
    let v = s
        .parse::<i64>()
        .map_err(|_| "Invalid setting value.")?;
    Ok(Some(v.clamp(min as i64, max as i64) as u32))
}

/// After a save or reset, load the session fresh + build the resolved-settings
/// JSON response + tell the scheduler about the new effective interval.
async fn finish_with_resolved(state: &AppState, session_id: &str) -> Response {
    let session_data = match session_svc::load_session(&state.sessions_dir, session_id).await {
        Ok(s) => s,
        Err(SessionError::NotFound(_) | SessionError::InvalidId(_)) => {
            return json_error(StatusCode::NOT_FOUND, "Session not found.");
        }
        Err(err) => {
            tracing::error!(session_id, error = %err, "finish_with_resolved: load failed");
            return json_error(StatusCode::INTERNAL_SERVER_ERROR, "Session load failed.");
        }
    };
    let persona_cfg = persona::load_persona_config(&state.data_dir, &session_data.persona).await;
    let effective = resolve_settings(Some(&session_data), &persona_cfg);
    let has_override = session_data.thread_memory_settings.is_some();

    state
        .memory
        .reschedule_thread_next_fire(session_id, effective.interval_minutes);

    Json(serde_json::json!({
        "effective": effective,
        "has_override": has_override,
    }))
    .into_response()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_interval_override_cases() {
        // Absent / empty → None (nothing submitted).
        assert_eq!(parse_interval_override(None), Ok(None));
        assert_eq!(parse_interval_override(Some("")), Ok(None));
        // Explicit 0 → Some(0) — "disable auto for this thread" override.
        assert_eq!(parse_interval_override(Some("0")), Ok(Some(0)));
        // Negative also → Some(0) so the UI can signal disable with any <=0.
        assert_eq!(parse_interval_override(Some("-10")), Ok(Some(0)));
        // Under-minimum → clamped up to 5.
        assert_eq!(parse_interval_override(Some("2")), Ok(Some(5)));
        // Normal value.
        assert_eq!(parse_interval_override(Some("60")), Ok(Some(60)));
        // Above max → 1440.
        assert_eq!(parse_interval_override(Some("9999")), Ok(Some(1440)));
        // Parse error.
        assert!(parse_interval_override(Some("abc")).is_err());
    }

    #[test]
    fn parse_u32_override_cases() {
        // Absent / empty.
        assert_eq!(parse_u32_override(None, 1, 1000), Ok(None));
        assert_eq!(parse_u32_override(Some(""), 1, 1000), Ok(None));
        // Within range.
        assert_eq!(parse_u32_override(Some("4"), 1, 1000), Ok(Some(4)));
        // Negative clamped to min.
        assert_eq!(parse_u32_override(Some("-5"), 1, 1000), Ok(Some(1)));
        // Above max clamped down.
        assert_eq!(parse_u32_override(Some("5000"), 1, 1000), Ok(Some(1000)));
        // Zero — size_limit accepts 0 when min=0; message_floor (min=1) clamps up.
        assert_eq!(parse_u32_override(Some("0"), 0, 100_000), Ok(Some(0)));
        assert_eq!(parse_u32_override(Some("0"), 1, 1000), Ok(Some(1)));
    }
}
