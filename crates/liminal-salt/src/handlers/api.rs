//! `/api/*` + `/settings/available-models/` — simple JSON endpoints for the
//! theme picker, theme save, and the OpenRouter model catalog. Separate from
//! `/settings/*` mutation handlers (which live in `handlers/settings.rs`)
//! because these are essentially data-dictionary endpoints with no state
//! changes beyond the theme write.

use axum::{
    Form, Json,
    extract::State,
    http::StatusCode,
    response::{IntoResponse, Response},
};
use serde::Deserialize;

use crate::{
    AppState,
    services::{config, providers, themes},
};

// =============================================================================
// GET /api/themes/
// =============================================================================

pub async fn themes(State(_state): State<AppState>) -> Response {
    let list = themes::list();
    Json(serde_json::json!({"themes": list})).into_response()
}

// =============================================================================
// POST /api/save-theme/
// =============================================================================

#[derive(Deserialize)]
pub struct SaveThemeForm {
    #[serde(default, rename = "colorTheme")]
    pub color_theme: String,
    #[serde(default, rename = "themeMode")]
    pub theme_mode: String,
}

pub async fn save_theme(
    State(state): State<AppState>,
    Form(form): Form<SaveThemeForm>,
) -> Response {
    let mut cfg = config::load_config(&state.data_dir).await;
    let theme = if form.color_theme.is_empty() {
        "liminal-salt".to_string()
    } else {
        form.color_theme
    };
    let mode = if form.theme_mode.is_empty() {
        "dark".to_string()
    } else {
        form.theme_mode
    };
    cfg.theme = theme.clone();
    cfg.theme_mode = mode.clone();
    if let Err(err) = config::save_config(&state.data_dir, &cfg).await {
        tracing::error!(error = %err, "save_theme: config write failed");
        return (StatusCode::INTERNAL_SERVER_ERROR, "save failed").into_response();
    }
    Json(serde_json::json!({
        "success": true,
        "theme": theme,
        "mode": mode,
    }))
    .into_response()
}

// =============================================================================
// GET /settings/available-models/
// =============================================================================

pub async fn available_models(State(state): State<AppState>) -> Response {
    let cfg = config::load_config(&state.data_dir).await;
    if cfg.api_key.is_empty() {
        return (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({"error": "No API key configured"})),
        )
            .into_response();
    }
    let Some(provider) = providers::by_id(&cfg.provider) else {
        return (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({"error": "Unknown provider"})),
        )
            .into_response();
    };
    let models = provider.list_models(&state.http, &cfg.api_key).await;
    if models.is_empty() {
        return (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"error": "Failed to fetch models"})),
        )
            .into_response();
    }
    Json(serde_json::json!({"models": models})).into_response()
}
