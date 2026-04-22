use axum::{
    extract::State,
    response::{Html, Redirect},
    routing::{get, post},
    Router,
};

use crate::{AppState, handlers};

pub fn build_router(state: AppState) -> Router {
    Router::new()
        // Health + home
        .route("/", get(index))
        .route("/health", get(health))
        .route("/hello", get(hello))
        // Chat lifecycle
        .route("/chat/", get(handlers::chat::view))
        .route("/chat/new/", get(handlers::chat::new_chat))
        .route("/chat/start/", post(handlers::chat::start_chat))
        .route("/chat/switch/", post(handlers::chat::switch))
        .route("/chat/send/", post(handlers::chat::send))
        .route("/chat/delete/", post(handlers::chat::delete))
        .route("/chat/pin/", post(handlers::chat::pin))
        .route("/chat/rename/", post(handlers::chat::rename))
        .route("/chat/save-draft/", post(handlers::chat::save_draft))
        .route("/chat/retry/", post(handlers::chat::retry))
        .route("/chat/edit-message/", post(handlers::chat::edit_message))
        // Session ops
        .route("/session/scenario/save/", post(handlers::session::save_scenario))
        .route("/session/fork-to-roleplay/", post(handlers::session::fork_to_roleplay))
        // Phase 4+ placeholders — returning something graceful so sidebar
        // footer clicks surface "Coming soon" rather than 404ing into broken state.
        .route("/memory/", get(handlers::stubs::page_not_yet))
        .route("/memory/wipe/", post(handlers::stubs::not_implemented))
        .route("/persona/", get(handlers::stubs::page_not_yet))
        .route("/persona/delete/", post(handlers::stubs::not_implemented))
        .route("/settings/", get(handlers::stubs::page_not_yet))
        // Phase 5 thread-memory endpoints (referenced by chat_main's data div).
        .route("/session/thread-memory/update/", post(handlers::stubs::not_implemented))
        .route("/session/thread-memory/status/", get(handlers::stubs::not_implemented))
        .route("/session/thread-memory/settings/save/", post(handlers::stubs::not_implemented))
        .route("/session/thread-memory/settings/reset/", post(handlers::stubs::not_implemented))
        // API endpoints — minimal JSON stubs so utils.js doesn't choke on page load.
        .route("/api/themes/", get(handlers::stubs::themes_empty))
        .route("/api/save-theme/", post(handlers::stubs::theme_save_ok))
        .with_state(state)
}

async fn index() -> Redirect {
    Redirect::to("/chat/")
}

async fn health() -> &'static str {
    "ok"
}

async fn hello(State(state): State<AppState>) -> Html<String> {
    let ctx = tera::Context::new();
    match state.tera.render("hello.html", &ctx) {
        Ok(html) => Html(html),
        Err(err) => {
            tracing::error!("tera render failed: {err}");
            Html(format!("<pre>tera error: {err}</pre>"))
        }
    }
}
