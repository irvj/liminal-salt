//! Placeholder handlers for endpoints owned by later phases. They return
//! lightweight responses so frontend clicks surface "not yet implemented"
//! cleanly rather than 404-ing into broken state.

use axum::{
    http::StatusCode,
    response::{Html, IntoResponse, Response},
};

const NOT_YET_BODY: &str = r#"<div class="p-8 text-center text-foreground-secondary">
    <h2 class="text-2xl mb-4">Coming soon</h2>
    <p>This page lands in a later migration phase.</p>
    <p class="mt-4"><a href="/chat/" class="underline">Back to chat</a></p>
</div>"#;

/// GET for pages users can open from the sidebar footer (/memory/, /persona/,
/// /settings/). Returns a placeholder body so the page doesn't 404.
pub async fn page_not_yet() -> Response {
    (StatusCode::OK, Html(NOT_YET_BODY)).into_response()
}

/// Any other stubbed endpoint (wipe, delete, etc.) — empty 501 is appropriate.
pub async fn not_implemented() -> Response {
    (StatusCode::NOT_IMPLEMENTED, "not implemented in Phase 3").into_response()
}

/// Minimal JSON stub for /api/themes/ — empty list lets utils.js's theme
/// picker initialize without errors.
pub async fn themes_empty() -> Response {
    (
        StatusCode::OK,
        [("content-type", "application/json")],
        "[]",
    )
        .into_response()
}

pub async fn theme_save_ok() -> Response {
    (
        StatusCode::OK,
        [("content-type", "application/json")],
        r#"{"success":true}"#,
    )
        .into_response()
}
