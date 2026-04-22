//! /session/* endpoints (scenario, fork-to-roleplay).

use axum::{
    Form,
    extract::State,
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Response},
};
use serde::Deserialize;
use tower_sessions::Session;

use crate::{
    AppState,
    handlers::chat::view as chat_view,
    middleware::session_state,
    services::session as session_svc,
};

#[derive(Deserialize)]
pub struct ScenarioForm {
    session_id: String,
    #[serde(default)]
    scenario: String,
}

pub async fn save_scenario(
    State(state): State<AppState>,
    Form(form): Form<ScenarioForm>,
) -> Response {
    if let Err(err) =
        session_svc::save_scenario(&state.sessions_dir, &form.session_id, &form.scenario).await
    {
        tracing::warn!(session_id = %form.session_id, error = %err, "save_scenario failed");
    }
    StatusCode::NO_CONTENT.into_response()
}

#[derive(Deserialize)]
pub struct ForkForm {
    #[serde(default)]
    session_id: String,
}

pub async fn fork_to_roleplay(
    State(state): State<AppState>,
    session: Session,
    headers: HeaderMap,
    Form(form): Form<ForkForm>,
) -> Response {
    let source = if form.session_id.is_empty() {
        session_state::current_session_id(&session)
            .await
            .unwrap_or_default()
    } else {
        form.session_id
    };
    if source.is_empty() {
        return (StatusCode::BAD_REQUEST, "no source session").into_response();
    }

    match session_svc::fork_to_roleplay(&state.sessions_dir, &source).await {
        Ok(new_id) => {
            session_state::set_current_session_id(&session, Some(&new_id)).await;
            chat_view(State(state), session, headers).await
        }
        Err(err) => {
            tracing::warn!(source_session_id = %source, error = %err, "fork_to_roleplay failed");
            (StatusCode::BAD_REQUEST, "fork failed").into_response()
        }
    }
}
