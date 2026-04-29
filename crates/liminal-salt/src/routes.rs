use axum::{
    Router,
    extract::State,
    response::{Html, Redirect},
    routing::{get, post},
};

use crate::{AppState, handlers};

pub fn build_router(state: AppState) -> Router {
    Router::new()
        // Health + home
        .route("/", get(index))
        .route("/health", get(health))
        .route("/hello", get(hello))
        // Setup wizard — the app-ready gate exempts this path.
        .route("/setup/", get(handlers::setup::view).post(handlers::setup::submit))
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
        // Persona page + CRUD
        .route("/persona/", get(handlers::persona::view))
        .route("/persona/create/", post(handlers::persona::create_persona))
        .route("/persona/save/", post(handlers::persona::save_persona))
        .route("/persona/delete/", post(handlers::persona::delete_persona))
        .route("/persona/save-model/", post(handlers::persona::save_persona_model))
        .route(
            "/persona/save-default-mode/",
            post(handlers::persona::save_persona_default_mode),
        )
        .route(
            "/persona/reset-default-mode/",
            post(handlers::persona::reset_persona_default_mode),
        )
        .route("/settings/save/", post(handlers::settings::save))
        // Memory page + ops
        .route("/memory/", get(handlers::memory::view))
        .route("/memory/update/", post(handlers::memory::update))
        .route("/memory/wipe/", post(handlers::memory::wipe))
        .route("/memory/modify/", post(handlers::memory::modify))
        .route("/memory/seed/", post(handlers::memory::seed))
        .route("/memory/save-settings/", post(handlers::memory::save_settings))
        .route("/memory/update-status/", get(handlers::memory::update_status))
        .route(
            "/memory/thread-memory-defaults/save/",
            post(handlers::memory::save_thread_memory_defaults),
        )
        .route(
            "/memory/thread-memory-defaults/reset/",
            post(handlers::memory::reset_thread_memory_defaults),
        )
        // Context files — global scope
        .route("/settings/context/upload/", post(handlers::context::upload_global))
        .route("/settings/context/delete/", post(handlers::context::delete_file_global))
        .route("/settings/context/toggle/", post(handlers::context::toggle_file_global))
        .route("/settings/context/content/", get(handlers::context::get_file_content))
        .route("/settings/context/save/", post(handlers::context::save_file_content))
        // Context files — per-persona scope
        .route("/persona/context/upload/", post(handlers::context::upload_persona))
        .route("/persona/context/delete/", post(handlers::context::delete_file_persona))
        .route("/persona/context/toggle/", post(handlers::context::toggle_file_persona))
        .route("/persona/context/content/", get(handlers::context::get_file_content))
        .route("/persona/context/save/", post(handlers::context::save_file_content))
        // Local directories — shared global/persona via `persona` form field
        .route("/context/local/browse/", get(handlers::context::browse_directory))
        .route("/context/local/add/", post(handlers::context::add_directory))
        .route("/context/local/remove/", post(handlers::context::remove_directory))
        .route("/context/local/toggle/", post(handlers::context::toggle_local_file))
        .route("/context/local/content/", get(handlers::context::get_local_file_content))
        .route("/context/local/refresh/", post(handlers::context::refresh_local_dir))
        // Settings page + AJAX mutations.
        .route("/settings/", get(handlers::settings::view))
        .route(
            "/settings/save-context-history-limit/",
            post(handlers::settings::save_context_history_limit),
        )
        .route(
            "/settings/validate-api-key/",
            post(handlers::settings::validate_provider_api_key),
        )
        .route(
            "/settings/save-provider-model/",
            post(handlers::settings::save_provider_model),
        )
        // Thread-memory endpoints.
        .route("/session/thread-memory/update/", post(handlers::thread_memory::update))
        .route("/session/thread-memory/status/", get(handlers::thread_memory::status))
        .route(
            "/session/thread-memory/settings/save/",
            post(handlers::thread_memory::settings_save),
        )
        .route(
            "/session/thread-memory/settings/reset/",
            post(handlers::thread_memory::settings_reset),
        )
        // Prompts editor (M3).
        .route("/prompts/", get(handlers::prompts::view))
        .route("/prompts/save/", post(handlers::prompts::save))
        .route("/prompts/reset/", post(handlers::prompts::reset))
        .route("/prompts/default/", get(handlers::prompts::default))
        // API endpoints.
        .route("/api/themes/", get(handlers::api::themes))
        .route("/api/save-theme/", post(handlers::api::save_theme))
        .route("/settings/available-models/", get(handlers::api::available_models))
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
