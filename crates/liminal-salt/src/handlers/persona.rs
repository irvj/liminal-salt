//! `/persona/` page + persona CRUD endpoints.

use axum::{
    Form, Json,
    extract::{Query, State},
    http::{HeaderMap, StatusCode},
    response::{Html, IntoResponse, Response},
};
use serde::{Deserialize, Serialize};
use tera::Context;
use tower_sessions::Session;

use crate::{
    AppState,
    handlers::status::persona_status,
    services::{
        context_files::ContextScope,
        persona::{self, PersonaConfig, PersonaError},
        thread_memory::{
            DEFAULT_THREAD_MEMORY_INTERVAL_MINUTES as DEFAULT_INTERVAL_MINUTES,
            DEFAULT_THREAD_MEMORY_MESSAGE_FLOOR as DEFAULT_MESSAGE_FLOOR,
            DEFAULT_THREAD_MEMORY_SIZE as DEFAULT_SIZE_LIMIT,
        },
    },
};

// =============================================================================
// GET /persona/
// =============================================================================

#[derive(Deserialize)]
pub struct PersonaQuery {
    #[serde(default)]
    pub persona: Option<String>,
    /// `personaSettingsPicker` uses `?preview=...`; accept both.
    #[serde(default)]
    pub preview: Option<String>,
}

pub async fn view(
    State(state): State<AppState>,
    session: Session,
    headers: HeaderMap,
    Query(q): Query<PersonaQuery>,
) -> Response {
    let personas = persona::list_personas(&state.data_dir).await;
    let cfg = crate::services::config::load_config(&state.data_dir).await;

    // Pick the selected persona in priority order.
    let selected = q
        .preview
        .or(q.persona)
        .filter(|p| !p.is_empty() && personas.contains(p))
        .or_else(|| {
            Some(cfg.default_persona.clone()).filter(|p| personas.contains(p))
        })
        .or_else(|| personas.first().cloned())
        .unwrap_or_default();

    let persona_cfg = if selected.is_empty() {
        PersonaConfig::default()
    } else {
        persona::load_persona_config(&state.data_dir, &selected).await
    };

    let preview = if selected.is_empty() {
        String::new()
    } else {
        persona::get_preview(&state.data_dir, &selected).await
    };

    // Persona-scoped context files summary — for the "Context Files" badge.
    let (persona_files_json, persona_local_dirs_json, badge_count) = if selected.is_empty() {
        ("[]".to_string(), "[]".to_string(), 0usize)
    } else {
        let scope = ContextScope::persona(&state.data_dir, &selected);
        let files = scope.list_files().await;
        let local_dirs = scope.list_local_directories().await;
        let enabled_uploaded = files.iter().filter(|f| f.enabled).count();
        let enabled_local: usize = local_dirs
            .iter()
            .map(|d| d.files.iter().filter(|f| f.enabled).count())
            .sum();
        (
            serde_json::to_string(&files).unwrap_or_else(|_| "[]".to_string()),
            serde_json::to_string(&local_dirs).unwrap_or_else(|_| "[]".to_string()),
            enabled_uploaded + enabled_local,
        )
    };

    // Thread defaults for template data attributes.
    let thread_defaults = persona_cfg
        .default_thread_memory_settings
        .clone()
        .unwrap_or_default();
    let has_defaults = persona_cfg.default_mode.is_some()
        || persona_cfg.default_thread_memory_settings.is_some();

    let mut ctx = super::chat::base_chat_context(&state, &session).await;
    ctx.insert("page", "persona");
    ctx.insert("show_home", &false);
    ctx.insert("personas", &personas);
    ctx.insert("default_persona", &cfg.default_persona);
    ctx.insert("selected_persona", &selected);
    ctx.insert("persona_preview", &preview);
    ctx.insert("persona_model", &persona_cfg.model.unwrap_or_default());
    ctx.insert("model", &cfg.model);
    ctx.insert("persona_context_files_json", &persona_files_json);
    ctx.insert("persona_context_local_dirs_json", &persona_local_dirs_json);
    ctx.insert("persona_context_badge_count", &badge_count);
    ctx.insert(
        "persona_default_mode_raw",
        &persona_cfg.default_mode.clone().unwrap_or_default(),
    );
    ctx.insert(
        "persona_default_interval_minutes",
        &thread_defaults
            .interval_minutes
            .unwrap_or(DEFAULT_INTERVAL_MINUTES),
    );
    ctx.insert(
        "persona_default_message_floor",
        &thread_defaults.message_floor.unwrap_or(DEFAULT_MESSAGE_FLOOR),
    );
    ctx.insert(
        "persona_default_size_limit",
        &thread_defaults.size_limit.unwrap_or(DEFAULT_SIZE_LIMIT),
    );
    ctx.insert("persona_has_thread_defaults", &has_defaults);

    let htmx = crate::handlers::chat::is_htmx(&headers);
    ctx.insert("is_htmx", &htmx);

    let template = if htmx {
        "persona/persona_main.html"
    } else {
        "chat/chat.html"
    };
    render(&state, template, &ctx)
}

fn render(state: &AppState, template: &str, ctx: &Context) -> Response {
    match state.tera.render(template, ctx) {
        Ok(html) => Html(html).into_response(),
        Err(err) => {
            tracing::error!(template, error = ?err, "persona render failed");
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("render failed: {err:?}"),
            )
                .into_response()
        }
    }
}

// =============================================================================
// CRUD — create / save-identity / delete
// =============================================================================

#[derive(Deserialize)]
pub struct CreatePersonaForm {
    name: String,
    #[serde(default)]
    content: String,
}

pub async fn create_persona(
    State(state): State<AppState>,
    session: Session,
    headers: HeaderMap,
    Form(form): Form<CreatePersonaForm>,
) -> Response {
    match persona::create_persona(&state.data_dir, &form.name, &form.content).await {
        Ok(()) => {
            // Re-render persona page with the new persona selected.
            let q = PersonaQuery {
                persona: Some(form.name),
                preview: None,
            };
            view(State(state), session, headers, Query(q)).await
        }
        Err(PersonaError::InvalidName) => {
            (StatusCode::BAD_REQUEST, "invalid persona name").into_response()
        }
        Err(PersonaError::AlreadyExists) => {
            (StatusCode::CONFLICT, "persona already exists").into_response()
        }
        Err(err) => {
            tracing::error!(error = %err, "create_persona failed");
            (StatusCode::INTERNAL_SERVER_ERROR, "create failed").into_response()
        }
    }
}

#[derive(Deserialize)]
pub struct SavePersonaForm {
    persona: String,
    #[serde(default)]
    new_name: Option<String>,
    content: String,
}

pub async fn save_persona(
    State(state): State<AppState>,
    session: Session,
    headers: HeaderMap,
    Form(form): Form<SavePersonaForm>,
) -> Response {
    let mut target = form.persona.clone();

    // Handle rename if a new_name was submitted and it differs.
    if let Some(new_name) = form.new_name.as_deref()
        && !new_name.is_empty()
        && new_name != form.persona
    {
        if let Err(err) = persona::rename_persona(&state.data_dir, &form.persona, new_name).await {
            tracing::warn!(error = %err, old_name = %form.persona, new_name, "rename failed; proceeding without rename");
        } else {
            target = new_name.to_string();
        }
    }

    if !persona::save_identity(&state.data_dir, &target, &form.content).await {
        return (StatusCode::INTERNAL_SERVER_ERROR, "save identity failed").into_response();
    }

    let q = PersonaQuery {
        persona: Some(target),
        preview: None,
    };
    view(State(state), session, headers, Query(q)).await
}

#[derive(Deserialize)]
pub struct DeletePersonaForm {
    persona: String,
}

pub async fn delete_persona(
    State(state): State<AppState>,
    session: Session,
    headers: HeaderMap,
    Form(form): Form<DeletePersonaForm>,
) -> Response {
    if let Err(err) = persona::delete_persona(&state.data_dir, &form.persona).await {
        tracing::error!(error = %err, persona = %form.persona, "delete failed");
        return (persona_status(&err), "delete failed").into_response();
    }
    // Also: if any session on disk pointed at this persona, it now references
    // a missing persona. Leave that state; switching to that session surfaces
    // the "Persona not found" warning sentinel in the prompt.
    view(
        State(state),
        session,
        headers,
        Query(PersonaQuery {
            persona: None,
            preview: None,
        }),
    )
    .await
}

// =============================================================================
// Persona model override
// =============================================================================

#[derive(Serialize)]
pub struct SaveModelResponse {
    success: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    model: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    error: Option<String>,
}

/// Multipart-friendly — the `editPersonaModelModal` posts FormData.
pub async fn save_persona_model(
    State(state): State<AppState>,
    mut multipart: axum::extract::Multipart,
) -> Response {
    let mut persona_name: Option<String> = None;
    let mut model: Option<String> = None;
    while let Ok(Some(field)) = multipart.next_field().await {
        match field.name() {
            Some("persona") => persona_name = field.text().await.ok(),
            Some("model") => model = field.text().await.ok(),
            _ => {}
        }
    }
    let Some(name) = persona_name else {
        return Json(SaveModelResponse {
            success: false,
            model: None,
            error: Some("missing persona".to_string()),
        })
        .into_response();
    };

    let mut cfg = persona::load_persona_config(&state.data_dir, &name).await;
    let model = model.unwrap_or_default();
    cfg.model = if model.is_empty() {
        None
    } else {
        Some(model.clone())
    };

    match persona::save_persona_config(&state.data_dir, &name, &cfg).await {
        Ok(()) => Json(SaveModelResponse {
            success: true,
            model: cfg.model,
            error: None,
        })
        .into_response(),
        Err(err) => {
            tracing::error!(error = %err, persona = %name, "save model failed");
            Json(SaveModelResponse {
                success: false,
                model: None,
                error: Some("save failed".to_string()),
            })
            .into_response()
        }
    }
}

// =============================================================================
// Per-persona default thread mode
// =============================================================================

#[derive(Deserialize)]
pub struct DefaultModeForm {
    persona: String,
    #[serde(default)]
    default_mode: String,
}

#[derive(Serialize)]
pub struct DefaultModeResponse {
    default_mode_raw: String,
    has_override: bool,
}

pub async fn save_persona_default_mode(
    State(state): State<AppState>,
    Form(form): Form<DefaultModeForm>,
) -> Response {
    let mut cfg = persona::load_persona_config(&state.data_dir, &form.persona).await;
    // Only persist "roleplay" as a mode override; "chatbot" is the baseline.
    cfg.default_mode = if form.default_mode == "roleplay" {
        Some("roleplay".to_string())
    } else {
        None
    };

    if let Err(err) = persona::save_persona_config(&state.data_dir, &form.persona, &cfg).await {
        tracing::error!(error = %err, persona = %form.persona, "save default mode failed");
        return (persona_status(&err), "save failed").into_response();
    }

    let raw = cfg.default_mode.unwrap_or_default();
    Json(DefaultModeResponse {
        has_override: !raw.is_empty(),
        default_mode_raw: raw,
    })
    .into_response()
}

#[derive(Deserialize)]
pub struct ResetDefaultModeForm {
    persona: String,
}

pub async fn reset_persona_default_mode(
    State(state): State<AppState>,
    Form(form): Form<ResetDefaultModeForm>,
) -> Response {
    let mut cfg = persona::load_persona_config(&state.data_dir, &form.persona).await;
    cfg.default_mode = None;

    if let Err(err) = persona::save_persona_config(&state.data_dir, &form.persona, &cfg).await {
        tracing::error!(error = %err, persona = %form.persona, "reset default mode failed");
        return (persona_status(&err), "reset failed").into_response();
    }

    Json(DefaultModeResponse {
        default_mode_raw: String::new(),
        has_override: false,
    })
    .into_response()
}


