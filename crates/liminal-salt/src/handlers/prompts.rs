//! `/prompts/` page + per-prompt save / reset / view-default endpoints.
//!
//! Save and reset are AJAX-driven (no HTMX swap) so unsaved edits in *other*
//! prompts on the page aren't blown away when one is saved.

use axum::{
    Form,
    extract::{Query, State},
    http::{HeaderMap, StatusCode},
    response::{Html, IntoResponse, Response},
};
use serde::{Deserialize, Serialize};
use tower_sessions::Session;

use crate::{
    AppState,
    handlers::status::prompt_status,
    services::prompts::{self, PROMPTS},
};

// =============================================================================
// GET /prompts/  — render the editor page
// =============================================================================

/// Per-prompt payload for the Alpine editor component. Loaded server-side and
/// passed as JSON via a `data-*` attribute.
#[derive(Serialize)]
struct PromptView {
    id: &'static str,
    display_name: &'static str,
    description: &'static str,
    content: String,
}

pub async fn view(
    State(state): State<AppState>,
    session: Session,
    headers: HeaderMap,
) -> Response {
    let mut entries = Vec::with_capacity(PROMPTS.len());
    for meta in PROMPTS {
        let content = prompts::load(&state.data_dir, &state.bundled_prompts_dir, meta.id)
            .await
            .unwrap_or_else(|err| {
                tracing::warn!(prompt = meta.id, error = %err, "prompt load failed; rendering empty");
                String::new()
            });
        entries.push(PromptView {
            id: meta.id,
            display_name: meta.display_name,
            description: meta.description,
            content,
        });
    }

    let prompts_json =
        serde_json::to_string(&entries).unwrap_or_else(|_| "[]".to_string());

    let mut ctx = super::chat::base_chat_context(&state, &session).await;
    ctx.insert("page", "prompts");
    ctx.insert("show_home", &false);
    ctx.insert("prompts", &entries);
    ctx.insert("prompts_json", &prompts_json);

    let htmx = super::chat::is_htmx(&headers);
    ctx.insert("is_htmx", &htmx);

    let template = if htmx {
        "prompts/prompts_main.html"
    } else {
        "chat/chat.html"
    };

    match state.tera.render(template, &ctx) {
        Ok(html) => Html(html).into_response(),
        Err(err) => {
            tracing::error!(template, error = ?err, "prompts render failed");
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("render failed: {err:?}"),
            )
                .into_response()
        }
    }
}

// =============================================================================
// POST /prompts/save/
// =============================================================================

#[derive(Deserialize)]
pub struct SaveForm {
    pub id: String,
    #[serde(default)]
    pub content: String,
}

pub async fn save(State(state): State<AppState>, Form(form): Form<SaveForm>) -> Response {
    match prompts::save(&state.data_dir, &form.id, &form.content).await {
        Ok(()) => StatusCode::NO_CONTENT.into_response(),
        Err(err) => {
            let status = prompt_status(&err);
            tracing::warn!(id = %form.id, error = %err, "prompt save failed");
            (status, err.to_string()).into_response()
        }
    }
}

// =============================================================================
// POST /prompts/reset/  — overwrite user file with bundled default; return new content
// =============================================================================

#[derive(Deserialize)]
pub struct ResetForm {
    pub id: String,
}

pub async fn reset(State(state): State<AppState>, Form(form): Form<ResetForm>) -> Response {
    if let Err(err) = prompts::reset(&state.data_dir, &state.bundled_prompts_dir, &form.id).await {
        let status = prompt_status(&err);
        tracing::warn!(id = %form.id, error = %err, "prompt reset failed");
        return (status, err.to_string()).into_response();
    }
    // Return the new content so the editor can populate the textarea without a
    // separate fetch. After `reset`, `load` will return the default — the user
    // file now mirrors the bundled default byte-for-byte.
    match prompts::load(&state.data_dir, &state.bundled_prompts_dir, &form.id).await {
        Ok(content) => (StatusCode::OK, content).into_response(),
        Err(err) => {
            let status = prompt_status(&err);
            tracing::warn!(id = %form.id, error = %err, "prompt reload after reset failed");
            (status, err.to_string()).into_response()
        }
    }
}

// =============================================================================
// GET /prompts/default/?id=<id>  — bundled default text for the "View default" modal
// =============================================================================

#[derive(Deserialize)]
pub struct DefaultQuery {
    pub id: String,
}

pub async fn default(
    State(state): State<AppState>,
    Query(q): Query<DefaultQuery>,
) -> Response {
    match prompts::load_default(&state.bundled_prompts_dir, &q.id).await {
        Ok(content) => (StatusCode::OK, content).into_response(),
        Err(err) => {
            let status = prompt_status(&err);
            tracing::warn!(id = %q.id, error = %err, "prompt default load failed");
            (status, err.to_string()).into_response()
        }
    }
}
