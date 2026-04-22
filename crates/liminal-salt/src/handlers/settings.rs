//! `/settings/*` — the app-wide settings page + its AJAX mutation endpoints.
//!
//! View is HTMX-only (non-HTMX GET redirects to `/chat/`, matching Python).
//! Mutations come in two flavors:
//! - Form POST (`/settings/save/`) that re-renders the partial
//! - AJAX JSON POST (validate API key, save provider+model, save history limit)

use axum::{
    Form, Json,
    extract::{Multipart, State},
    http::{HeaderMap, StatusCode},
    response::{Html, IntoResponse, Response},
};
use serde::Deserialize;
use tera::Context;
use tower_sessions::Session;

use crate::{
    AppState,
    services::{
        config, context_files::ContextScope, openrouter,
    },
};

// =============================================================================
// GET /settings/
// =============================================================================

pub async fn view(
    State(state): State<AppState>,
    session: Session,
    headers: HeaderMap,
) -> Response {
    // Full-page GET shows the chat shell with the settings panel; HTMX GET
    // returns just the settings_main.html partial. Matches the memory/persona
    // page pattern from Phase 4.
    render_settings(&state, &session, &headers, None, None).await
}

// =============================================================================
// POST /settings/save/  — save default persona
// =============================================================================

#[derive(Deserialize)]
pub struct SaveForm {
    #[serde(default)]
    pub persona: String,
    /// "persona" → render persona page after save; anything else (incl. empty)
    /// → render settings page. Matches Python's behavior.
    #[serde(default)]
    pub redirect_to: String,
}

pub async fn save(
    State(state): State<AppState>,
    session: Session,
    headers: HeaderMap,
    Form(form): Form<SaveForm>,
) -> Response {
    // "Personality is required — fall back to 'assistant' if empty."
    let selected_persona = if form.persona.trim().is_empty() {
        "assistant".to_string()
    } else {
        form.persona.trim().to_string()
    };

    let mut cfg = config::load_config(&state.data_dir).await;
    let mut success_msg: Option<&'static str> = None;
    if selected_persona != cfg.default_persona {
        cfg.default_persona = selected_persona.clone();
        if let Err(err) = config::save_config(&state.data_dir, &cfg).await {
            tracing::error!(error = %err, "save default persona failed");
            return (StatusCode::INTERNAL_SERVER_ERROR, "save failed").into_response();
        }
        success_msg = Some("Default persona updated");
    }

    if form.redirect_to == "persona" {
        // Persona page owns that render. Delegate.
        super::persona::view(
            State(state),
            session,
            headers,
            axum::extract::Query(super::persona::PersonaQuery {
                persona: Some(selected_persona),
                preview: None,
            }),
        )
        .await
    } else {
        render_settings(&state, &session, &headers, success_msg, None).await
    }
}

// =============================================================================
// POST /settings/save-context-history-limit/  — JSON AJAX (multipart)
//
// JS sends `new FormData()` which is always `multipart/form-data`, not
// urlencoded. `axum::Form<T>` only handles urlencoded, so we parse
// multipart manually here (matches the pattern in `memory::save_settings`).
// =============================================================================

pub async fn save_context_history_limit(
    State(state): State<AppState>,
    mut multipart: Multipart,
) -> Response {
    let mut raw = String::new();
    while let Ok(Some(field)) = multipart.next_field().await {
        if field.name() == Some("context_history_limit") {
            raw = field.text().await.unwrap_or_default();
            break;
        }
    }

    // 10..=500 with 50 fallback — matches Python's clamp.
    let value: u32 = raw
        .parse::<i64>()
        .map(|v| v.clamp(10, 500) as u32)
        .unwrap_or(50);

    let mut cfg = config::load_config(&state.data_dir).await;
    cfg.context_history_limit = value;
    if let Err(err) = config::save_config(&state.data_dir, &cfg).await {
        tracing::error!(error = %err, "save context history limit failed");
        return (StatusCode::INTERNAL_SERVER_ERROR, "save failed").into_response();
    }
    Json(serde_json::json!({
        "success": true,
        "context_history_limit": value,
    }))
    .into_response()
}

// =============================================================================
// POST /settings/validate-api-key/  — JSON AJAX (multipart)
// =============================================================================

pub async fn validate_provider_api_key(
    State(state): State<AppState>,
    mut multipart: Multipart,
) -> Response {
    let mut provider = String::new();
    let mut api_key = String::new();
    let mut use_existing_raw = String::new();
    while let Ok(Some(field)) = multipart.next_field().await {
        match field.name() {
            Some("provider") => provider = field.text().await.unwrap_or_default(),
            Some("api_key") => api_key = field.text().await.unwrap_or_default(),
            Some("use_existing") => use_existing_raw = field.text().await.unwrap_or_default(),
            _ => {}
        }
    }

    let provider = if provider.is_empty() {
        "openrouter".to_string()
    } else {
        provider
    };
    let use_existing = use_existing_raw == "true";

    // If using the existing key, pull it from config; otherwise use the one
    // the user just typed.
    let api_key = if use_existing {
        let cfg = config::load_config(&state.data_dir).await;
        match provider.as_str() {
            "openrouter" => cfg.openrouter_api_key,
            _ => String::new(),
        }
    } else {
        api_key.trim().to_string()
    };

    if api_key.is_empty() {
        return Json(serde_json::json!({
            "valid": false,
            "error": "API key required",
        }))
        .into_response();
    }

    if provider != "openrouter" {
        return Json(serde_json::json!({
            "valid": false,
            "error": "Unknown provider",
        }))
        .into_response();
    }

    // Validate unless re-using a previously-validated key.
    if !use_existing && !openrouter::validate_api_key(&state.http, &api_key).await {
        return Json(serde_json::json!({
            "valid": false,
            "error": "Invalid API key",
        }))
        .into_response();
    }

    let models = openrouter::get_formatted_model_list(&state.http, &api_key).await;
    if models.is_empty() {
        return Json(serde_json::json!({
            "valid": false,
            "error": "Could not fetch models",
        }))
        .into_response();
    }

    Json(serde_json::json!({
        "valid": true,
        "models": models,
    }))
    .into_response()
}

// =============================================================================
// POST /settings/save-provider-model/  — JSON AJAX (multipart)
// =============================================================================

pub async fn save_provider_model(
    State(state): State<AppState>,
    mut multipart: Multipart,
) -> Response {
    let mut provider_raw = String::new();
    let mut api_key_raw = String::new();
    let mut model_raw = String::new();
    let mut keep_existing_key_raw = String::new();
    while let Ok(Some(field)) = multipart.next_field().await {
        match field.name() {
            Some("provider") => provider_raw = field.text().await.unwrap_or_default(),
            Some("api_key") => api_key_raw = field.text().await.unwrap_or_default(),
            Some("model") => model_raw = field.text().await.unwrap_or_default(),
            Some("keep_existing_key") => {
                keep_existing_key_raw = field.text().await.unwrap_or_default();
            }
            _ => {}
        }
    }

    let provider = provider_raw.trim().to_string();
    let api_key = api_key_raw.trim().to_string();
    let model = model_raw.trim().to_string();
    let keep_existing_key = keep_existing_key_raw == "true";

    if provider.is_empty() || model.is_empty() {
        return Json(serde_json::json!({
            "success": false,
            "error": "Provider and model required",
        }))
        .into_response();
    }

    let mut cfg = config::load_config(&state.data_dir).await;

    // Corruption safety check — matches Python. If the caller wants to keep
    // the existing key but the loaded config has no key AND the file does
    // exist on disk, that's almost certainly a broken load (e.g. JSON parse
    // returned default). Refuse to clobber rather than save an empty key.
    if keep_existing_key
        && cfg.openrouter_api_key.is_empty()
        && config::config_file_exists(&state.data_dir).await
    {
        tracing::error!("Config appears corrupted — load returned empty but file exists");
        return Json(serde_json::json!({
            "success": false,
            "error": "Configuration file may be corrupted. Please check config.json",
        }))
        .into_response();
    }

    cfg.provider = provider.clone();
    if !keep_existing_key && !api_key.is_empty() && provider == "openrouter" {
        cfg.openrouter_api_key = api_key;
    }
    cfg.model = model.clone();

    if let Err(err) = config::save_config(&state.data_dir, &cfg).await {
        tracing::error!(error = %err, "save provider/model failed");
        return Json(serde_json::json!({
            "success": false,
            "error": "Configuration save failed",
        }))
        .into_response();
    }

    Json(serde_json::json!({
        "success": true,
        "provider": provider,
        "model": model,
    }))
    .into_response()
}

// =============================================================================
// Helpers — render the settings view
// =============================================================================

async fn render_settings(
    state: &AppState,
    session: &Session,
    headers: &HeaderMap,
    success: Option<&str>,
    error: Option<&str>,
) -> Response {
    let cfg = config::load_config(state.data_dir.as_path()).await;
    let scope = ContextScope::global(&state.data_dir);
    let ctx_files = scope.list_files().await;
    let local_dirs = scope.list_local_directories().await;
    let context_badge = badge_count(&ctx_files, &local_dirs);

    let providers = config::get_providers();
    let has_api_key = !cfg.openrouter_api_key.is_empty();
    let providers_json = serde_json::to_string(providers).unwrap_or_else(|_| "[]".into());
    let local_dirs_json = serde_json::to_string(&local_dirs).unwrap_or_else(|_| "[]".into());

    let mut ctx = super::chat::base_chat_context(state, session).await;
    ctx.insert("page", "settings");
    ctx.insert("show_home", &false);
    ctx.insert("model", &cfg.model);
    ctx.insert("provider", &cfg.provider);
    ctx.insert("providers", providers);
    ctx.insert("providers_json", &providers_json);
    ctx.insert("has_api_key", &has_api_key);
    ctx.insert("context_history_limit", &cfg.context_history_limit);
    ctx.insert("context_files", &ctx_files);
    ctx.insert("context_local_dirs_json", &local_dirs_json);
    ctx.insert("context_badge_count", &context_badge);
    ctx.insert("success", &success);
    ctx.insert("error", &error);

    let htmx = super::chat::is_htmx(headers);
    ctx.insert("is_htmx", &htmx);

    let template = if htmx {
        "settings/settings_main.html"
    } else {
        "chat/chat.html"
    };
    render(&state.tera, template, &ctx)
}

fn badge_count(
    files: &[crate::services::context_files::ContextFileEntry],
    local_dirs: &[crate::services::context_files::LocalDirectoryEntry],
) -> usize {
    let enabled_files = files.iter().filter(|f| f.enabled).count();
    let enabled_local: usize = local_dirs
        .iter()
        .map(|d| d.files.iter().filter(|f| f.enabled).count())
        .sum();
    enabled_files + enabled_local
}

fn render(tera: &tera::Tera, template: &str, ctx: &Context) -> Response {
    match tera.render(template, ctx) {
        Ok(html) => Html(html).into_response(),
        Err(err) => {
            tracing::error!(template, error = ?err, "settings render failed");
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("render failed: {err:?}"),
            )
                .into_response()
        }
    }
}

