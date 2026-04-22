//! Thin typed helpers over `tower-sessions` for the three keys the chat flow
//! cares about: CSRF token, current session id, user timezone.
//!
//! The backing store is `MemoryStore` — state lives in-process and resets on
//! server restart. The browser's session-id cookie becomes a dangling handle
//! after restart; `tower-sessions` auto-creates a fresh row.

use rand::Rng;
use tower_sessions::Session;

const CSRF_KEY: &str = "csrf_token";
const CURRENT_SESSION_KEY: &str = "current_session";
const TIMEZONE_KEY: &str = "user_timezone";
const SETUP_STEP_KEY: &str = "setup_step";

/// Fetch (or lazily mint) the CSRF token for the current session. A 32-byte
/// random token, hex-encoded to 64 chars.
pub async fn csrf_token(session: &Session) -> String {
    if let Ok(Some(existing)) = session.get::<String>(CSRF_KEY).await {
        return existing;
    }
    let token = new_token();
    let _ = session.insert(CSRF_KEY, &token).await;
    token
}

/// The token that a POST's `X-CSRFToken` header (or `csrfmiddlewaretoken`
/// form field) must match. Returns empty string if none has been minted yet
/// — the CSRF middleware treats that as "reject."
pub async fn current_csrf_token(session: &Session) -> String {
    session
        .get::<String>(CSRF_KEY)
        .await
        .ok()
        .flatten()
        .unwrap_or_default()
}

pub async fn current_session_id(session: &Session) -> Option<String> {
    session.get::<String>(CURRENT_SESSION_KEY).await.ok().flatten()
}

pub async fn set_current_session_id(session: &Session, id: Option<&str>) {
    match id {
        Some(v) => {
            let _ = session.insert(CURRENT_SESSION_KEY, v).await;
        }
        None => {
            let _ = session.remove::<String>(CURRENT_SESSION_KEY).await;
        }
    }
}

pub async fn user_timezone(session: &Session) -> String {
    session
        .get::<String>(TIMEZONE_KEY)
        .await
        .ok()
        .flatten()
        .unwrap_or_else(|| "UTC".to_string())
}

pub async fn set_user_timezone(session: &Session, tz: &str) {
    let _ = session.insert(TIMEZONE_KEY, tz).await;
}

/// Setup wizard step — 1, 2, or 3. `None` means the user isn't mid-wizard; the
/// handler picks an initial step based on current config state.
pub async fn setup_step(session: &Session) -> Option<u8> {
    session.get::<u8>(SETUP_STEP_KEY).await.ok().flatten()
}

pub async fn set_setup_step(session: &Session, step: u8) {
    let _ = session.insert(SETUP_STEP_KEY, step).await;
}

pub async fn clear_setup_step(session: &Session) {
    let _ = session.remove::<u8>(SETUP_STEP_KEY).await;
}

fn new_token() -> String {
    let mut buf = [0u8; 32];
    rand::rng().fill_bytes(&mut buf);
    let mut hex = String::with_capacity(64);
    for b in buf {
        hex.push_str(&format!("{b:02x}"));
    }
    hex
}
