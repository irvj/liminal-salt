//! Setup wizard — three-step onboarding flow. State (current step) lives in
//! `tower-sessions` so a page refresh in the middle of the wizard picks up
//! where the user left off.
//!
//! Steps:
//! 1. Provider + API key → validated against OpenRouter → partial config saved
//! 2. Theme + model → model fetched via `/settings/available-models` equivalent
//! 3. Agreement → accept flips `setup_complete=true` + `agreement_accepted=<version>`

use axum::{
    Form,
    extract::State,
    http::{HeaderMap, StatusCode},
    response::{Html, IntoResponse, Redirect, Response},
};
use serde::Deserialize;
use tera::Context;
use tower_sessions::Session;

use crate::{
    AppState,
    middleware::session_state,
    services::{config, openrouter, themes},
};

// =============================================================================
// Common form — every step POSTs the same set of fields; each step only
// looks at the ones it cares about.
// =============================================================================

#[derive(Deserialize, Default)]
pub struct SetupForm {
    #[serde(default)]
    pub setup_action: String,

    // Step 1
    #[serde(default)]
    pub provider: String,
    #[serde(default)]
    pub api_key: String,

    // Step 2
    #[serde(default)]
    pub theme: String,
    #[serde(default)]
    pub theme_mode: String,
    #[serde(default)]
    pub model: String,
}

// =============================================================================
// Public handlers — Axum routes to GET/POST /setup/
// =============================================================================

pub async fn view(
    State(state): State<AppState>,
    session: Session,
    headers: HeaderMap,
) -> Response {
    dispatch(state, session, headers, None).await
}

pub async fn submit(
    State(state): State<AppState>,
    session: Session,
    headers: HeaderMap,
    Form(form): Form<SetupForm>,
) -> Response {
    dispatch(state, session, headers, Some(form)).await
}

async fn dispatch(
    state: AppState,
    session: Session,
    headers: HeaderMap,
    form: Option<SetupForm>,
) -> Response {
    let cfg = config::load_config(&state.data_dir).await;
    if config::is_app_ready(&cfg) {
        return Redirect::to("/chat/").into_response();
    }

    // Initialize step from config if the session doesn't have one yet.
    let step = match session_state::setup_step(&session).await {
        Some(s) => s,
        None => {
            let s = initial_step(&cfg);
            session_state::set_setup_step(&session, s).await;
            s
        }
    };

    // "Back" works from any step except 1.
    if let Some(f) = form.as_ref()
        && f.setup_action == "back"
    {
        if step > 1 {
            session_state::set_setup_step(&session, step - 1).await;
        }
        return Redirect::to("/setup/").into_response();
    }

    match step {
        1 => step1(&state, &session, &headers, form).await,
        2 => step2(&state, &session, &headers, form).await,
        3 => step3(&state, &session, &headers, form, &cfg).await,
        _ => {
            // Bogus session value — reset.
            session_state::set_setup_step(&session, 1).await;
            Redirect::to("/setup/").into_response()
        }
    }
}

/// Starting step for a fresh wizard session, derived from what's already in
/// config.json:
///
/// - `setup_complete=true` but `agreement_accepted` drifted → step 3 (agreement re-prompt)
/// - API key + model already present (beta upgrade path) → step 3
/// - Otherwise → step 1
fn initial_step(cfg: &config::AppConfig) -> u8 {
    if cfg.setup_complete {
        return 3;
    }
    if !cfg.api_key.is_empty() && !cfg.model.is_empty() {
        return 3;
    }
    1
}

// =============================================================================
// Step 1 — provider + API key
// =============================================================================

async fn step1(
    state: &AppState,
    session: &Session,
    headers: &HeaderMap,
    form: Option<SetupForm>,
) -> Response {
    let mut cfg = config::load_config(&state.data_dir).await;

    if let Some(f) = form {
        let provider = if f.provider.is_empty() {
            "openrouter".to_string()
        } else {
            f.provider
        };
        let api_key = f.api_key.trim().to_string();

        if api_key.is_empty() {
            return render_step1(state, session, headers, StepArgs {
                provider: &provider,
                api_key: "",
                error: Some("Please enter an API key"),
            })
            .await;
        }

        if provider == "openrouter" && !openrouter::validate_api_key(&state.http, &api_key).await {
            tracing::error!("API key validation failed");
            return render_step1(state, session, headers, StepArgs {
                provider: &provider,
                api_key: &api_key,
                error: Some("Invalid API key. Please check your key and try again."),
            })
            .await;
        }

        // Preserve any prior keys — matches Python's `setdefault` semantics —
        // so a partial re-run doesn't clobber fields the user already set.
        cfg.provider = provider;
        cfg.api_key = api_key;
        if cfg.default_persona.is_empty() {
            cfg.default_persona = "assistant".to_string();
        }
        if cfg.context_history_limit == 0 {
            cfg.context_history_limit = 50;
        }
        if let Err(err) = config::save_config(&state.data_dir, &cfg).await {
            tracing::error!(error = %err, "step1: save_config failed");
            return (StatusCode::INTERNAL_SERVER_ERROR, "config save failed").into_response();
        }

        session_state::set_setup_step(session, 2).await;
        return Redirect::to("/setup/").into_response();
    }

    let provider = if cfg.provider.is_empty() {
        "openrouter".to_string()
    } else {
        cfg.provider.clone()
    };
    render_step1(state, session, headers, StepArgs {
        provider: &provider,
        api_key: &cfg.api_key,
        error: None,
    })
    .await
}

struct StepArgs<'a> {
    provider: &'a str,
    api_key: &'a str,
    error: Option<&'a str>,
}

async fn render_step1(
    state: &AppState,
    session: &Session,
    _headers: &HeaderMap,
    args: StepArgs<'_>,
) -> Response {
    let mut ctx = base_ctx(session).await;
    let providers = config::get_providers();
    ctx.insert("providers", providers);
    ctx.insert("selected_provider", args.provider);
    ctx.insert("api_key", args.api_key);
    if let Some(e) = args.error {
        ctx.insert("error", e);
    }
    render(&state.tera, "setup/step1.html", &ctx)
}

// =============================================================================
// Step 2 — theme + model
// =============================================================================

async fn step2(
    state: &AppState,
    session: &Session,
    headers: &HeaderMap,
    form: Option<SetupForm>,
) -> Response {
    let mut cfg = config::load_config(&state.data_dir).await;
    if cfg.api_key.is_empty() {
        // Key went missing between steps — Python bounces back to step 1.
        tracing::error!("step2: no API key in config");
        session_state::set_setup_step(session, 1).await;
        return Redirect::to("/setup/").into_response();
    }

    if let Some(f) = form {
        let selected_model = f.model.trim().to_string();
        let selected_theme = if f.theme.is_empty() {
            "liminal-salt".to_string()
        } else {
            f.theme.trim().to_string()
        };
        let selected_mode = if f.theme_mode.is_empty() {
            "dark".to_string()
        } else {
            f.theme_mode.trim().to_string()
        };

        if selected_model.is_empty() {
            // Re-render with an error, including the model list so the user
            // can correct and resubmit.
            return render_step2(
                state,
                session,
                headers,
                Step2Args {
                    api_key: &cfg.api_key,
                    selected_model: &selected_model,
                    selected_theme: &selected_theme,
                    selected_mode: &selected_mode,
                    error: Some("Please select a model"),
                },
            )
            .await;
        }

        cfg.model = selected_model;
        cfg.theme = selected_theme;
        cfg.theme_mode = selected_mode;
        if let Err(err) = config::save_config(&state.data_dir, &cfg).await {
            tracing::error!(error = %err, "step2: save_config failed");
            return (StatusCode::INTERNAL_SERVER_ERROR, "config save failed").into_response();
        }
        tracing::info!(
            model = %cfg.model,
            theme = %cfg.theme,
            mode = %cfg.theme_mode,
            "setup step 2 complete"
        );

        session_state::set_setup_step(session, 3).await;
        return Redirect::to("/setup/").into_response();
    }

    let selected_theme = if cfg.theme.is_empty() {
        "liminal-salt".to_string()
    } else {
        cfg.theme.clone()
    };
    let selected_mode = if cfg.theme_mode.is_empty() {
        "dark".to_string()
    } else {
        cfg.theme_mode.clone()
    };
    render_step2(
        state,
        session,
        headers,
        Step2Args {
            api_key: &cfg.api_key,
            selected_model: &cfg.model,
            selected_theme: &selected_theme,
            selected_mode: &selected_mode,
            error: None,
        },
    )
    .await
}

struct Step2Args<'a> {
    api_key: &'a str,
    selected_model: &'a str,
    selected_theme: &'a str,
    selected_mode: &'a str,
    error: Option<&'a str>,
}

async fn render_step2(
    state: &AppState,
    session: &Session,
    _headers: &HeaderMap,
    args: Step2Args<'_>,
) -> Response {
    let mut ctx = base_ctx(session).await;

    // Fetch models. Failure is a distinct error path — matches Python's
    // "could not fetch, go back and check the key" branch.
    let models = openrouter::get_formatted_model_list(&state.http, args.api_key).await;
    let themes_list = themes::list_themes().await;
    let models_json = serde_json::to_string(&models).unwrap_or_else(|_| "[]".into());
    let themes_json = serde_json::to_string(&themes_list).unwrap_or_else(|_| "[]".into());

    let error_msg = if models.is_empty() {
        Some("Could not fetch models from OpenRouter. Go back and re-enter your API key.")
    } else {
        args.error
    };

    ctx.insert("available_models", &models);
    ctx.insert("available_models_json", &models_json);
    ctx.insert("model_count", &models.len());
    ctx.insert("selected_model", args.selected_model);
    ctx.insert("themes", &themes_list);
    ctx.insert("themes_json", &themes_json);
    ctx.insert("selected_theme", args.selected_theme);
    ctx.insert("selected_mode", args.selected_mode);
    if let Some(e) = error_msg {
        ctx.insert("error", e);
    }
    render(&state.tera, "setup/step2.html", &ctx)
}

// =============================================================================
// Step 3 — agreement
// =============================================================================

async fn step3(
    state: &AppState,
    session: &Session,
    _headers: &HeaderMap,
    form: Option<SetupForm>,
    cfg: &config::AppConfig,
) -> Response {
    if let Some(f) = form.as_ref()
        && f.setup_action == "accept"
    {
        let mut updated = cfg.clone();
        updated.setup_complete = true;
        updated.agreement_accepted = config::current_agreement_version().to_string();
        if let Err(err) = config::save_config(&state.data_dir, &updated).await {
            tracing::error!(error = %err, "step3: save_config failed");
            return (StatusCode::INTERNAL_SERVER_ERROR, "config save failed").into_response();
        }
        tracing::info!(
            version = %updated.agreement_accepted,
            "setup complete; agreement accepted"
        );
        session_state::clear_setup_step(session).await;
        return Redirect::to("/chat/").into_response();
    }

    // Back button only when we're in the initial walkthrough. If setup was
    // already complete and we're re-prompting for an updated agreement, the
    // user has no earlier steps to revisit.
    let can_go_back = !cfg.setup_complete;

    let mut ctx = base_ctx(session).await;
    ctx.insert("agreement_version", config::current_agreement_version());
    ctx.insert("agreement_body", &config::AGREEMENT.body);
    ctx.insert("can_go_back", &can_go_back);
    render(&state.tera, "setup/step3.html", &ctx)
}

// =============================================================================
// Helpers
// =============================================================================

async fn base_ctx(session: &Session) -> Context {
    let mut ctx = Context::new();
    ctx.insert(
        "csrf_token",
        &session_state::current_csrf_token(session).await,
    );
    // Theme context — setup pages extend base.html which reads these.
    ctx.insert("color_theme", "liminal-salt");
    ctx.insert("theme_mode", "dark");
    ctx
}

fn render(tera: &tera::Tera, template: &str, ctx: &Context) -> Response {
    match tera.render(template, ctx) {
        Ok(html) => Html(html).into_response(),
        Err(err) => {
            tracing::error!(template, error = ?err, "setup render failed");
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("render failed: {err:?}"),
            )
                .into_response()
        }
    }
}
