//! CSRF middleware. Validates POST requests by comparing a per-session token
//! against one of:
//! - the `X-CSRFToken` header (preferred — HTMX and every `fetch()` JSON call
//!   send this), or
//! - the `csrfmiddlewaretoken` field in a `application/x-www-form-urlencoded`
//!   body (the `saveEditedMessage` path — fetch(FormData) without header), or
//! - the `csrfmiddlewaretoken` field in a `multipart/form-data` body (the
//!   persona / context-file upload POSTs in `components.js`).

use axum::{
    body::{Body, to_bytes},
    extract::Request,
    http::{Method, StatusCode, header},
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

    // Slow path: form body (urlencoded or multipart) — read, scan for the
    // token field, reconstruct the request so the handler can still consume it.
    let content_type = req
        .headers()
        .get(header::CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .map(|s| s.to_string());

    if let Some(ct) = content_type {
        let ct_lower = ct.trim().to_ascii_lowercase();
        let is_urlencoded = ct_lower.starts_with("application/x-www-form-urlencoded");
        let multipart_boundary = if ct_lower.starts_with("multipart/form-data") {
            boundary_from(&ct)
        } else {
            None
        };

        if is_urlencoded || multipart_boundary.is_some() {
            let (parts, body) = req.into_parts();
            let bytes = to_bytes(body, MAX_BODY_BYTES)
                .await
                .map_err(|_| StatusCode::PAYLOAD_TOO_LARGE)?;
            let valid = if is_urlencoded {
                form_field_matches(&bytes, &expected)
            } else {
                multipart_field_matches(&bytes, multipart_boundary.as_deref().unwrap(), &expected)
            };
            let req = Request::from_parts(parts, Body::from(bytes));
            if valid {
                return Ok(next.run(req).await);
            }
        }
    }

    Err(StatusCode::FORBIDDEN)
}

fn is_checked_method(method: &Method) -> bool {
    matches!(*method, Method::POST | Method::PUT | Method::PATCH | Method::DELETE)
}

/// Extract the `boundary=` parameter from a `multipart/form-data` content type.
fn boundary_from(content_type: &str) -> Option<String> {
    for part in content_type.split(';') {
        let part = part.trim();
        if let Some(rest) = part.strip_prefix("boundary=") {
            let trimmed = rest.trim_matches('"');
            return Some(trimmed.to_string());
        }
    }
    None
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

/// Scan a multipart body for a text field with the given name and compare its
/// value to `expected`. Naive string scan — enough for the "find one simple
/// text field" case; not a full multipart parser.
fn multipart_field_matches(body: &[u8], boundary: &str, expected: &str) -> bool {
    const TARGET_NAME: &str = "csrfmiddlewaretoken";
    let Ok(text) = std::str::from_utf8(body) else {
        return false;
    };
    let delim = format!("--{boundary}");
    for part in text.split(&*delim) {
        // Look for the Content-Disposition header with name="csrfmiddlewaretoken".
        let marker = format!("name=\"{TARGET_NAME}\"");
        let Some(hdr_end) = part.find(&marker) else {
            continue;
        };
        // Skip past the headers-to-body separator (CRLFCRLF). Browsers and
        // curl both emit CRLF; tolerate bare LF too for dev reverse-proxies.
        let after = &part[hdr_end..];
        let body_start = after.find("\r\n\r\n").map(|i| i + 4)
            .or_else(|| after.find("\n\n").map(|i| i + 2));
        let Some(start) = body_start else { continue; };
        let value_region = &after[start..];
        // Trim trailing CRLF that precedes the next boundary delimiter.
        let value = value_region.trim_end_matches("\r\n").trim_end_matches('\n');
        // Value may still contain trailing '--' if this was the terminator part —
        // but that's only on the final marker, not on a data part.
        if constant_time_eq(value.as_bytes(), expected.as_bytes()) {
            return true;
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
    fn boundary_from_extracts_plain_and_quoted() {
        assert_eq!(
            boundary_from("multipart/form-data; boundary=----WebKitFormBoundary"),
            Some("----WebKitFormBoundary".to_string())
        );
        assert_eq!(
            boundary_from(r#"multipart/form-data; boundary="with spaces""#),
            Some("with spaces".to_string())
        );
        assert_eq!(boundary_from("application/json"), None);
    }

    #[test]
    fn multipart_field_matches_finds_token() {
        let boundary = "xXxBOUNDARYxXx";
        let body = format!(
            "--{b}\r\n\
             Content-Disposition: form-data; name=\"content\"\r\n\r\n\
             hello world\r\n\
             --{b}\r\n\
             Content-Disposition: form-data; name=\"csrfmiddlewaretoken\"\r\n\r\n\
             abc123\r\n\
             --{b}--\r\n",
            b = boundary
        );
        assert!(multipart_field_matches(body.as_bytes(), boundary, "abc123"));
        assert!(!multipart_field_matches(body.as_bytes(), boundary, "wrong"));
        // Missing field → no match.
        let body_no_token = format!(
            "--{b}\r\nContent-Disposition: form-data; name=\"other\"\r\n\r\nvalue\r\n--{b}--\r\n",
            b = boundary
        );
        assert!(!multipart_field_matches(body_no_token.as_bytes(), boundary, "abc123"));
    }

    #[test]
    fn constant_time_eq_matches_std_eq() {
        assert!(constant_time_eq(b"hello", b"hello"));
        assert!(!constant_time_eq(b"hello", b"world"));
        assert!(!constant_time_eq(b"short", b"longer"));
        assert!(constant_time_eq(b"", b""));
    }
}
