//! CSRF middleware. Validates POST requests by comparing a per-session token
//! against one of:
//! - the `X-CSRFToken` header (preferred — HTMX and every `fetch()` JSON call
//!   send this), or
//! - the `csrfmiddlewaretoken` field in a urlencoded form body (the few
//!   FormData-only POSTs in the existing frontend).
//!
//! Multipart form bodies are not checked for the form-field token in this
//! middleware — no Phase 3 endpoint uses multipart. File-upload endpoints
//! (Phase 4+) will need to either send the header or get their own check.

use axum::{
    body::{Body, to_bytes},
    extract::Request,
    http::{HeaderMap, Method, StatusCode, header},
    middleware::Next,
    response::Response,
};
use tower_sessions::Session;

use crate::middleware::session_state;

/// Max form body we'll buffer when looking for the token. Real chat endpoints
/// are well under this; anything bigger can send the header instead.
const MAX_BODY_BYTES: usize = 64 * 1024;

const CSRF_HEADER: &str = "X-CSRFToken";
const CSRF_FORM_FIELD: &str = "csrfmiddlewaretoken";

pub async fn require_csrf(
    session: Session,
    req: Request,
    next: Next,
) -> Result<Response, StatusCode> {
    // Mint a token eagerly on every request so GET /chat/ can embed it in the
    // page before the user submits a form.
    session_state::csrf_token(&session).await;

    if !is_checked_method(req.method()) {
        return Ok(next.run(req).await);
    }

    let expected = session_state::current_csrf_token(&session).await;
    if expected.is_empty() {
        return Err(StatusCode::FORBIDDEN);
    }

    // Fast path: header present.
    if let Some(header_value) = req.headers().get(CSRF_HEADER)
        && constant_time_eq(header_value.as_bytes(), expected.as_bytes())
    {
        return Ok(next.run(req).await);
    }

    // Slow path: urlencoded form body.
    if is_urlencoded_form(req.headers()) {
        let (parts, body) = req.into_parts();
        let bytes = to_bytes(body, MAX_BODY_BYTES)
            .await
            .map_err(|_| StatusCode::PAYLOAD_TOO_LARGE)?;
        let valid = form_field_matches(&bytes, &expected);
        let req = Request::from_parts(parts, Body::from(bytes));
        if valid {
            return Ok(next.run(req).await);
        }
    }

    Err(StatusCode::FORBIDDEN)
}

fn is_checked_method(method: &Method) -> bool {
    matches!(*method, Method::POST | Method::PUT | Method::PATCH | Method::DELETE)
}

fn is_urlencoded_form(headers: &HeaderMap) -> bool {
    headers
        .get(header::CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .map(|ct| {
            ct.trim()
                .to_ascii_lowercase()
                .starts_with("application/x-www-form-urlencoded")
        })
        .unwrap_or(false)
}

fn form_field_matches(body: &[u8], expected: &str) -> bool {
    // Tolerate non-utf8 silently — that shouldn't happen for urlencoded forms
    // and we'd reject it anyway.
    let Ok(text) = std::str::from_utf8(body) else {
        return false;
    };
    for (key, value) in form_urlencoded::parse(text.as_bytes()) {
        if key == CSRF_FORM_FIELD {
            return constant_time_eq(value.as_bytes(), expected.as_bytes());
        }
    }
    false
}

fn constant_time_eq(a: &[u8], b: &[u8]) -> bool {
    if a.len() != b.len() {
        return false;
    }
    let mut diff: u8 = 0;
    for (x, y) in a.iter().zip(b.iter()) {
        diff |= x ^ y;
    }
    diff == 0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn form_field_matches_finds_token() {
        let body = b"message=hi&csrfmiddlewaretoken=abc123&other=x";
        assert!(form_field_matches(body, "abc123"));
        assert!(!form_field_matches(body, "wrong"));
        assert!(!form_field_matches(b"no token here", "abc123"));
    }

    #[test]
    fn constant_time_eq_matches_std_eq() {
        assert!(constant_time_eq(b"hello", b"hello"));
        assert!(!constant_time_eq(b"hello", b"world"));
        assert!(!constant_time_eq(b"short", b"longer"));
        assert!(constant_time_eq(b"", b""));
    }
}
