//! `/memory/` page handler. Phase 4 lands the GET view only — the memory
//! operations (update/wipe/modify/seed/save-settings/status) stay stubbed
//! until Phase 5's `memory_manager` service ships.

use axum::{
    extract::{Query, State},
    http::{HeaderMap, StatusCode},
    response::{Html, IntoResponse, Response},
};
use serde::Deserialize;
use tower_sessions::Session;

use crate::{
    AppState,
    services::{
        config,
        persona,
    },
};

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

    let memory_content = if selected.is_empty() {
        String::new()
    } else {
        let path = state.data_dir.join("memory").join(format!("{selected}.md"));
        tokio::fs::read_to_string(&path).await.unwrap_or_default()
    };

    // Last-update timestamp = file mtime, formatted as ISO-8601 so the client
    // JS can render it.
    let last_update = if selected.is_empty() {
        String::new()
    } else {
        let path = state.data_dir.join("memory").join(format!("{selected}.md"));
        match tokio::fs::metadata(&path).await {
            Ok(meta) => meta
                .modified()
                .ok()
                .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                .map(|d| {
                    chrono::DateTime::<chrono::Utc>::from(
                        std::time::UNIX_EPOCH + std::time::Duration::from_secs(d.as_secs()),
                    )
                    .to_rfc3339()
                })
                .unwrap_or_default(),
            Err(_) => String::new(),
        }
    };

    let persona_cfg = if selected.is_empty() {
        persona::PersonaConfig::default()
    } else {
        persona::load_persona_config(&state.data_dir, &selected).await
    };

    let mut ctx = super::chat::base_chat_context(&state, &session).await;
    ctx.insert("page", "memory");
    ctx.insert("show_home", &false);
    ctx.insert("selected_persona", &selected);
    ctx.insert("available_personas", &personas);
    ctx.insert("model", &cfg.model);
    ctx.insert("memory_content", &memory_content);
    ctx.insert("last_update", &last_update);
    ctx.insert("just_updated", &false);
    ctx.insert("memory_updating", &false);
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

    let htmx = super::chat::is_htmx(&headers);
    ctx.insert("is_htmx", &htmx);

    let template = if htmx {
        "memory/memory_main.html"
    } else {
        "chat/chat.html"
    };
    match state.tera.render(template, &ctx) {
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
