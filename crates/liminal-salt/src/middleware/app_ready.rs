//! App-ready gate. Mirrors Python's per-view `is_app_ready(config)` check:
//! until the user finishes the setup wizard AND accepts the current agreement
//! version, every non-exempt path is redirected to `/setup/`.
//!
//! Exempt paths:
//! - `/setup/*` — the wizard itself, plus its POST endpoints
//! - `/static/*` — CSS/JS/assets (served by tower-http before this layer anyway)
//! - `/health` — liveness probe
//! - `/api/themes/` — the wizard's step 2 reads this for the theme picker
//!
//! Sits after `session_layer` (so `tower-sessions` has initialized) and before
//! `csrf_layer` (so we don't waste the CSRF check on a redirected request).

use axum::{
    extract::{Request, State},
    http::{StatusCode, header},
    middleware::Next,
    response::{IntoResponse, Redirect, Response},
};

use crate::{AppState, services::config};

pub async fn require_app_ready(
    State(state): State<AppState>,
    req: Request,
    next: Next,
) -> Response {
    let path = req.uri().path();
    if is_exempt(path) {
        return next.run(req).await;
    }

    let cfg = config::load_config(&state.data_dir).await;
    if config::is_app_ready(&cfg) {
        return next.run(req).await;
    }

    // Fetch → redirect. HTMX-aware: HX-Redirect header makes the client do a
    // client-side navigation instead of swapping the redirect HTML into a
    // partial target. For plain GET we return a 302; for anything else we
    // return 401 so the client doesn't try to treat a redirect body as data.
    let is_htmx = req
        .headers()
        .get("HX-Request")
        .map(|v| v.as_bytes() == b"true")
        .unwrap_or(false);

    if is_htmx {
        let mut resp = (StatusCode::OK, "").into_response();
        resp.headers_mut()
            .insert("HX-Redirect", header::HeaderValue::from_static("/setup/"));
        return resp;
    }
    if req.method() == axum::http::Method::GET {
        Redirect::to("/setup/").into_response()
    } else {
        (StatusCode::UNAUTHORIZED, "setup not complete").into_response()
    }
}

fn is_exempt(path: &str) -> bool {
    path == "/setup/"
        || path.starts_with("/setup/")
        || path == "/static"
        || path.starts_with("/static/")
        || path == "/health"
        || path == "/api/themes/"
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exempt_paths_match() {
        assert!(is_exempt("/setup/"));
        assert!(is_exempt("/setup/anything/deep"));
        assert!(is_exempt("/static/js/utils.js"));
        assert!(is_exempt("/health"));
        assert!(is_exempt("/api/themes/"));
    }

    #[test]
    fn non_exempt_paths_pass_through() {
        assert!(!is_exempt("/chat/"));
        assert!(!is_exempt("/memory/"));
        assert!(!is_exempt("/settings/"));
        assert!(!is_exempt("/api/save-theme/"));
        assert!(!is_exempt("/"));
    }
}
