//! Chat-flow HTTP handlers. Thin: parse → call service → render → return.
//! All file I/O goes through `services::session`; all LLM calls go through
//! `services::chat` / `services::summarizer`.

use std::collections::BTreeMap;

use axum::{
    Form,
    extract::{Multipart, State},
    http::{HeaderMap, HeaderValue, StatusCode},
    response::{Html, IntoResponse, Response},
};
use serde::Deserialize;
use tera::Context;
use tower_sessions::Session;

use crate::{
    AppState,
    middleware::session_state,
    services::{
        chat as chat_svc, config, llm::LlmClient, prompt, session as session_svc, summarizer,
    },
};
use crate::services::session::{Mode, SessionSummary};

// =============================================================================
// Helpers
// =============================================================================

fn is_htmx(headers: &HeaderMap) -> bool {
    headers
        .get("HX-Request")
        .map(|v| v.as_bytes() == b"true")
        .unwrap_or(false)
}

#[derive(serde::Serialize)]
pub struct PersonaGroup {
    pub persona: String,
    pub sessions: Vec<SessionSummary>,
}

/// Partition sessions into pinned + persona-grouped. Persona groups are
/// ordered by most-recent-session-id descending, matching Python's sidebar
/// ordering. Each group is already newest-first because `list_sessions` sorts
/// by id desc.
fn group_sessions(
    sessions: Vec<SessionSummary>,
) -> (Vec<SessionSummary>, Vec<PersonaGroup>) {
    let mut pinned = Vec::new();
    let mut buckets: BTreeMap<String, Vec<SessionSummary>> = BTreeMap::new();
    for s in sessions {
        if s.pinned {
            pinned.push(s);
        } else {
            buckets.entry(s.persona.clone()).or_default().push(s);
        }
    }
    let mut groups: Vec<PersonaGroup> = buckets
        .into_iter()
        .map(|(persona, sessions)| PersonaGroup { persona, sessions })
        .collect();
    groups.sort_by(|a, b| {
        b.sessions
            .first()
            .map(|s| s.id.as_str())
            .unwrap_or("")
            .cmp(a.sessions.first().map(|s| s.id.as_str()).unwrap_or(""))
    });
    (pinned, groups)
}

/// Build a Context seeded with fields every chat template expects: csrf token,
/// theme, sidebar session groups, current session highlight.
async fn base_chat_context(state: &AppState, session: &Session) -> Context {
    let mut ctx = Context::new();
    ctx.insert(
        "csrf_token",
        &session_state::current_csrf_token(session).await,
    );

    let cfg = config::load_config(&state.data_dir).await;
    let theme = if cfg.theme.is_empty() {
        "liminal-salt".to_string()
    } else {
        cfg.theme.clone()
    };
    ctx.insert("color_theme", &theme);
    let mode = if cfg.theme_mode.is_empty() {
        "dark".to_string()
    } else {
        cfg.theme_mode.clone()
    };
    ctx.insert("theme_mode", &mode);

    let current = session_state::current_session_id(session).await;
    ctx.insert("current_session", &current.clone().unwrap_or_default());

    let sessions = session_svc::list_sessions(&state.sessions_dir).await;
    let (pinned, grouped) = group_sessions(sessions);
    ctx.insert("pinned_sessions", &pinned);
    // Tera iterates a map as `for k, v in map` with BTreeMap (alphabetical) or
    // preserve-order if configured. We render from a `Vec<(String, Vec<_>)>`
    // which Tera happily treats as key/value pairs when iterated.
    ctx.insert("grouped_sessions", &grouped);

    ctx.insert("default_persona", &cfg.default_persona);
    ctx.insert("default_model", &cfg.model);
    ctx
}

/// Render either `chat/chat.html` (full page) or `chat/chat_main.html` /
/// `chat/chat_home.html` (HTMX partial) depending on the request.
async fn render_view(state: &AppState, session: &Session, headers: &HeaderMap) -> Response {
    let current_id = session_state::current_session_id(session).await;
    let mut ctx = base_chat_context(state, session).await;

    let cfg = config::load_config(&state.data_dir).await;

    // Load the current session if present & valid, else render home.
    let session_data = match &current_id {
        Some(id) => session_svc::load_session(&state.sessions_dir, id).await,
        None => None,
    };

    let show_home = session_data.is_none();
    ctx.insert("show_home", &show_home);

    if show_home {
        // Home page: persona picker + first-message form.
        let personas = prompt::available_personas(&state.data_dir).await;
        ctx.insert("personas", &personas);
        // Phase 4 will fill in real persona → model / persona → mode maps.
        ctx.insert("persona_models_json", "{}");
        ctx.insert("persona_modes_json", "{}");
    } else if let Some(data) = session_data {
        let id = current_id.unwrap_or_default();
        ctx.insert("session_id", &id);
        ctx.insert("title", &data.title);
        ctx.insert("persona", &data.persona);
        let mode_str = match data.mode {
            Mode::Chatbot => "chatbot",
            Mode::Roleplay => "roleplay",
        };
        ctx.insert("mode", mode_str);
        ctx.insert("messages", &data.messages);
        ctx.insert("model", &cfg.model);
        ctx.insert("draft", &data.draft.clone().unwrap_or_default());
        ctx.insert("scenario", &data.scenario.clone().unwrap_or_default());
        ctx.insert("thread_memory", &data.thread_memory);
        ctx.insert("thread_memory_updated_at", &data.thread_memory_updated_at);
        ctx.insert("thread_memory_interval_minutes", &0u32);
        ctx.insert("thread_memory_message_floor", &4u32);
        ctx.insert("thread_memory_size_limit", &4000u32);
        ctx.insert("thread_memory_has_override", &false);
    }

    let htmx = is_htmx(headers);
    ctx.insert("is_htmx", &htmx);

    let template = if htmx {
        if show_home {
            "chat/chat_home.html"
        } else {
            "chat/chat_main.html"
        }
    } else {
        "chat/chat.html"
    };

    match state.tera.render(template, &ctx) {
        Ok(html) => Html(html).into_response(),
        Err(err) => {
            tracing::error!(template, error = ?err, "template render failed");
            (StatusCode::INTERNAL_SERVER_ERROR, format!("template render failed: {err:?}")).into_response()
        }
    }
}

async fn render_sidebar_fragment(state: &AppState, session: &Session) -> Response {
    let mut ctx = Context::new();
    ctx.insert(
        "csrf_token",
        &session_state::current_csrf_token(session).await,
    );
    let current = session_state::current_session_id(session).await;
    ctx.insert("current_session", &current.unwrap_or_default());
    let sessions = session_svc::list_sessions(&state.sessions_dir).await;
    let (pinned, grouped) = group_sessions(sessions);
    ctx.insert("pinned_sessions", &pinned);
    ctx.insert("grouped_sessions", &grouped);

    match state.tera.render("chat/sidebar_sessions.html", &ctx) {
        Ok(html) => Html(html).into_response(),
        Err(err) => {
            tracing::error!(error = %err, "sidebar render failed");
            (StatusCode::INTERNAL_SERVER_ERROR, "sidebar render failed").into_response()
        }
    }
}

async fn persist_timezone(session: &Session, tz: Option<&str>) {
    if let Some(t) = tz
        && !t.is_empty()
    {
        session_state::set_user_timezone(session, t).await;
    }
}

fn build_llm(state: &AppState, cfg: &config::AppConfig) -> LlmClient {
    LlmClient::new(cfg.openrouter_api_key.clone(), cfg.model.clone())
        .with_http_client(state.http.clone())
}

// =============================================================================
// Handlers
// =============================================================================

pub async fn view(
    State(state): State<AppState>,
    session: Session,
    headers: HeaderMap,
) -> Response {
    render_view(&state, &session, &headers).await
}

pub async fn new_chat(
    State(state): State<AppState>,
    session: Session,
    headers: HeaderMap,
) -> Response {
    session_state::set_current_session_id(&session, None).await;
    render_view(&state, &session, &headers).await
}

#[derive(Deserialize)]
pub struct SwitchForm {
    session_id: String,
}

pub async fn switch(
    State(state): State<AppState>,
    session: Session,
    headers: HeaderMap,
    Form(form): Form<SwitchForm>,
) -> Response {
    if session_svc::valid_session_id(&form.session_id) {
        session_state::set_current_session_id(&session, Some(&form.session_id)).await;
    }
    render_view(&state, &session, &headers).await
}

#[derive(Deserialize)]
pub struct StartChatForm {
    message: String,
    persona: String,
    #[serde(default)]
    mode: String,
    #[serde(default)]
    scenario: String,
    #[serde(default)]
    timezone: String,
}

pub async fn start_chat(
    State(state): State<AppState>,
    session: Session,
    headers: HeaderMap,
    Form(form): Form<StartChatForm>,
) -> Response {
    persist_timezone(&session, Some(&form.timezone)).await;

    let mode = if form.mode == "roleplay" {
        Mode::Roleplay
    } else {
        Mode::Chatbot
    };

    // Create session with the user's initial message pre-saved.
    let id = session_svc::generate_session_id();
    let initial_messages = vec![session_svc::Message {
        role: crate::services::session::Role::User,
        content: form.message.clone(),
        timestamp: session_svc::now_timestamp(),
    }];
    let created = session_svc::create_session(
        &state.sessions_dir,
        &id,
        &form.persona,
        "New Chat",
        mode,
        initial_messages.clone(),
    )
    .await;

    if created.is_none() {
        return (StatusCode::INTERNAL_SERVER_ERROR, "session create failed").into_response();
    }

    // Save scenario if provided (roleplay only).
    if matches!(mode, Mode::Roleplay) && !form.scenario.is_empty() {
        session_svc::save_scenario(&state.sessions_dir, &id, &form.scenario).await;
    }

    session_state::set_current_session_id(&session, Some(&id)).await;

    // Render the main chat view with `pending_message` — the template fires an
    // HTMX `hx-trigger="load"` POST to /chat/send/ so the user sees the
    // thinking indicator while the LLM responds.
    let mut ctx = base_chat_context(&state, &session).await;
    ctx.insert("session_id", &id);
    ctx.insert("title", "New Chat");
    ctx.insert("persona", &form.persona);
    ctx.insert(
        "mode",
        match mode {
            Mode::Chatbot => "chatbot",
            Mode::Roleplay => "roleplay",
        },
    );
    // Render with the user message in place so it's visible immediately.
    // The auto-send fires with `skip_user_save=true` so it won't be double-appended.
    ctx.insert("messages", &initial_messages);
    let cfg = config::load_config(&state.data_dir).await;
    ctx.insert("model", &cfg.model);
    ctx.insert("draft", "");
    ctx.insert("scenario", &form.scenario);
    ctx.insert("thread_memory", "");
    ctx.insert("thread_memory_updated_at", "");
    ctx.insert("thread_memory_interval_minutes", &0u32);
    ctx.insert("thread_memory_message_floor", &4u32);
    ctx.insert("thread_memory_size_limit", &4000u32);
    ctx.insert("thread_memory_has_override", &false);
    ctx.insert("pending_message", &form.message);
    ctx.insert("is_htmx", &is_htmx(&headers));
    ctx.insert("show_home", &false);

    match state.tera.render("chat/chat_main.html", &ctx) {
        Ok(html) => Html(html).into_response(),
        Err(err) => {
            tracing::error!(error = %err, "start_chat render failed");
            (StatusCode::INTERNAL_SERVER_ERROR, "render failed").into_response()
        }
    }
}

#[derive(Deserialize)]
pub struct SendForm {
    message: String,
    #[serde(default)]
    skip_user_save: String,
    #[serde(default)]
    timezone: String,
}

pub async fn send(
    State(state): State<AppState>,
    session: Session,
    Form(form): Form<SendForm>,
) -> Response {
    persist_timezone(&session, Some(&form.timezone)).await;

    let Some(session_id) = session_state::current_session_id(&session).await else {
        return (StatusCode::BAD_REQUEST, "no current session").into_response();
    };

    let Some(existing) = session_svc::load_session(&state.sessions_dir, &session_id).await else {
        return (StatusCode::NOT_FOUND, "session not found").into_response();
    };

    let cfg = config::load_config(&state.data_dir).await;
    let llm = build_llm(&state, &cfg);

    let system_prompt = prompt::build_system_prompt(&state.data_dir, &existing).await;

    let user_tz = session_state::user_timezone(&session).await;
    let history_limit = if cfg.context_history_limit == 0 {
        50
    } else {
        cfg.context_history_limit as usize
    };
    let ctx_out = chat_svc::SendContext {
        sessions_dir: &state.sessions_dir,
        session_id: &session_id,
        system_prompt: &system_prompt,
        user_timezone: &user_tz,
        assistant_timezone: None,
        context_history_limit: history_limit,
    };

    let skip = form.skip_user_save == "true" || form.skip_user_save == "1";
    let outcome = chat_svc::send_message(&ctx_out, &llm, &form.message, skip).await;

    // After-the-fact: if the session still has no title_locked and this wasn't
    // an error, generate one. Read the session fresh to avoid TOCTOU on the
    // just-persisted messages.
    let mut title_changed: Option<String> = None;
    if !outcome.is_error
        && let Some(post) = session_svc::load_session(&state.sessions_dir, &session_id).await
        && !post.title_locked.unwrap_or(false)
    {
        // Use the first user + first assistant message for the summary prompt.
        let first_user = post
            .messages
            .iter()
            .find(|m| matches!(m.role, crate::services::session::Role::User))
            .map(|m| m.content.clone())
            .unwrap_or_default();
        let first_assistant = post
            .messages
            .iter()
            .find(|m| matches!(m.role, crate::services::session::Role::Assistant))
            .map(|m| m.content.clone())
            .unwrap_or_default();

        if !first_user.is_empty() && !first_assistant.is_empty() {
            let title = summarizer::generate_title(&llm, &first_user, &first_assistant).await;
            // Persist with title_locked=true so this runs exactly once.
            session_svc::save_chat_history(
                &state.sessions_dir,
                &session_id,
                &title,
                &post.persona,
                post.messages,
                Some(true),
            )
            .await;
            title_changed = Some(title);
        }
    }

    // Render the assistant_fragment partial — HTMX appends this to
    // #messages-inner (hx-swap="beforeend").
    let mut ctx = Context::new();
    ctx.insert(
        "csrf_token",
        &session_state::current_csrf_token(&session).await,
    );
    ctx.insert("assistant_message", &outcome.response);
    ctx.insert("assistant_timestamp", &session_svc::now_timestamp());

    let body = match state.tera.render("chat/assistant_fragment.html", &ctx) {
        Ok(html) => html,
        Err(err) => {
            tracing::error!(error = %err, "assistant fragment render failed");
            return (StatusCode::INTERNAL_SERVER_ERROR, "render failed").into_response();
        }
    };

    let mut response = Html(body).into_response();
    if let Some(title) = title_changed {
        if let Ok(v) = HeaderValue::from_str(&title) {
            response.headers_mut().insert("X-Chat-Title", v);
        }
        if let Ok(v) = HeaderValue::from_str(&session_id) {
            response.headers_mut().insert("X-Chat-Session-Id", v);
        }
    }
    response
}

#[derive(Deserialize)]
pub struct DeleteForm {
    #[serde(default)]
    session_id: String,
}

pub async fn delete(
    State(state): State<AppState>,
    session: Session,
    headers: HeaderMap,
    Form(form): Form<DeleteForm>,
) -> Response {
    let target = if form.session_id.is_empty() {
        session_state::current_session_id(&session).await.unwrap_or_default()
    } else {
        form.session_id
    };
    if !target.is_empty() {
        session_svc::delete_session(&state.sessions_dir, &target).await;
        // Clear current_session if we just deleted it.
        if session_state::current_session_id(&session).await.as_deref() == Some(target.as_str()) {
            session_state::set_current_session_id(&session, None).await;
        }
    }
    render_view(&state, &session, &headers).await
}

#[derive(Deserialize)]
pub struct PinForm {
    session_id: String,
}

pub async fn pin(
    State(state): State<AppState>,
    session: Session,
    Form(form): Form<PinForm>,
) -> Response {
    session_svc::toggle_pin(&state.sessions_dir, &form.session_id).await;
    render_sidebar_fragment(&state, &session).await
}

#[derive(Deserialize)]
pub struct RenameForm {
    session_id: String,
    new_title: String,
}

pub async fn rename(
    State(state): State<AppState>,
    session: Session,
    Form(form): Form<RenameForm>,
) -> Response {
    session_svc::rename_session(&state.sessions_dir, &form.session_id, &form.new_title).await;
    render_sidebar_fragment(&state, &session).await
}

#[derive(Deserialize)]
pub struct DraftForm {
    session_id: String,
    #[serde(default)]
    draft: String,
}

pub async fn save_draft(
    State(state): State<AppState>,
    Form(form): Form<DraftForm>,
) -> Response {
    session_svc::save_draft(&state.sessions_dir, &form.session_id, &form.draft).await;
    StatusCode::NO_CONTENT.into_response()
}

pub async fn retry(State(state): State<AppState>, session: Session) -> Response {
    let Some(id) = session_state::current_session_id(&session).await else {
        return (StatusCode::BAD_REQUEST, "no current session").into_response();
    };

    let Some((last_user_content, _)) =
        session_svc::remove_last_assistant_message(&state.sessions_dir, &id).await
    else {
        return (StatusCode::BAD_REQUEST, "nothing to retry").into_response();
    };

    // Dispatch through `send` logic with skip_user_save=true so we don't
    // double-append the user message.
    let form = SendForm {
        message: last_user_content,
        skip_user_save: "true".to_string(),
        timezone: String::new(),
    };
    send(State(state), session, Form(form)).await
}

/// The frontend submits `saveEditedMessage` via `fetch(FormData)` — that is,
/// `multipart/form-data`, not urlencoded — so we use axum's `Multipart`
/// extractor rather than `Form`.
pub async fn edit_message(
    State(state): State<AppState>,
    session: Session,
    mut multipart: Multipart,
) -> Response {
    let Some(id) = session_state::current_session_id(&session).await else {
        return (StatusCode::BAD_REQUEST, "no current session").into_response();
    };

    let mut content: Option<String> = None;
    while let Ok(Some(field)) = multipart.next_field().await {
        if field.name() == Some("content")
            && let Ok(text) = field.text().await
        {
            content = Some(text);
            break;
        }
    }

    let Some(content) = content else {
        return (StatusCode::BAD_REQUEST, "content required").into_response();
    };

    let ok = session_svc::update_last_user_message(&state.sessions_dir, &id, &content).await;
    if ok {
        StatusCode::OK.into_response()
    } else {
        (StatusCode::BAD_REQUEST, "edit failed").into_response()
    }
}
