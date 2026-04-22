//! Chat flow — load session, append user message, call LLM (with retry),
//! append assistant message, save. Ports `chat/services/chat_core.py` semantics.
//!
//! Unlike the Python `ChatCore` class, this module is stateless: each
//! `send_message` call loads the session fresh and writes through
//! `session::save_chat_history`. That preserves the "ChatCore doesn't own the
//! file" invariant from CLAUDE.md.

use std::path::Path;
use std::time::Duration;

use chrono::{DateTime, TimeZone, Utc};
use chrono_tz::Tz;

use crate::services::llm::{ChatLlm, LlmError, LlmMessage};
use crate::services::session::{self, Message, Role};

const SEND_TIMEOUT: Duration = Duration::from_secs(120);
const MAX_RETRIES: u32 = 2;
const RETRY_DELAY: Duration = Duration::from_secs(2);

/// All the per-request knobs for a chat turn. The handler fills this in from
/// AppState + config + session; `send_message` is pure with respect to it.
pub struct SendContext<'a> {
    pub sessions_dir: &'a Path,
    pub session_id: &'a str,
    pub system_prompt: &'a str,
    pub user_timezone: &'a str,
    pub assistant_timezone: Option<&'a str>,
    pub context_history_limit: usize,
}

/// Why a chat turn failed. `Display` renders the user-facing error body; the
/// handler passes `to_string()` into the template as `error_message`.
#[derive(Debug)]
pub enum ChatError {
    SessionNotFound(String),
    LlmFailed(LlmError),
}

impl std::fmt::Display for ChatError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::SessionNotFound(id) => write!(f, "session not found: {id}"),
            Self::LlmFailed(err) => write!(f, "{err}"),
        }
    }
}

impl std::error::Error for ChatError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::SessionNotFound(_) => None,
            Self::LlmFailed(err) => Some(err),
        }
    }
}

/// Append a user message, run the LLM, append the assistant response, save
/// the session. On success returns the assistant's reply; on failure returns
/// a typed error the handler can shape into a response or short-circuit on.
///
/// `skip_user_save = true` means the user message is already persisted
/// upstream (e.g. by `start_chat` which saves before dispatching to `send`).
pub async fn send_message<L: ChatLlm>(
    ctx: &SendContext<'_>,
    llm: &L,
    user_input: &str,
    skip_user_save: bool,
) -> Result<String, ChatError> {
    // 1. Load the session. No-session is a hard error — without it we can't
    //    persist the exchange.
    let mut session = session::load_session(ctx.sessions_dir, ctx.session_id)
        .await
        .ok_or_else(|| ChatError::SessionNotFound(ctx.session_id.to_string()))?;

    if !skip_user_save {
        session.messages.push(Message {
            role: Role::User,
            content: user_input.to_string(),
            timestamp: session::now_timestamp(),
        });
    }

    // 2. Build payload with prepended time context + per-user-message time prefix.
    let payload = build_payload(
        ctx.system_prompt,
        &session.messages,
        ctx.context_history_limit,
        ctx.user_timezone,
        ctx.assistant_timezone,
    );

    // 3. Call LLM with retries. 2-attempt policy with a 2s backoff between tries.
    let assistant_text = run_with_retry(llm, &payload).await.map_err(|err| {
        tracing::warn!(
            session_id = ctx.session_id,
            error = %err,
            "LLM call failed after retries",
        );
        ChatError::LlmFailed(err)
    })?;

    // 4. Append assistant message + persist. `save_chat_history` RMWs through
    //    the session lock so scenario/thread_memory/pinned/draft survive.
    session.messages.push(Message {
        role: Role::Assistant,
        content: assistant_text.clone(),
        timestamp: session::now_timestamp(),
    });

    let saved = session::save_chat_history(
        ctx.sessions_dir,
        ctx.session_id,
        &session.title,
        &session.persona,
        session.messages,
        None, // don't touch title_locked here — title generation is a separate step
    )
    .await;
    if !saved {
        tracing::warn!(session_id = ctx.session_id, "save_chat_history returned false");
    }

    Ok(assistant_text)
}

async fn run_with_retry<L: ChatLlm>(
    llm: &L,
    payload: &[LlmMessage],
) -> Result<String, LlmError> {
    let mut last_err: Option<LlmError> = None;
    for attempt in 0..MAX_RETRIES {
        if attempt > 0 {
            tracing::info!(attempt = attempt + 1, max = MAX_RETRIES, "LLM retry");
            tokio::time::sleep(RETRY_DELAY).await;
        }
        match tokio::time::timeout(SEND_TIMEOUT, llm.complete(payload)).await {
            Ok(Ok(text)) => return Ok(text),
            Ok(Err(err)) => {
                tracing::warn!(attempt = attempt + 1, error = %err, "LLM attempt failed");
                last_err = Some(err);
            }
            Err(_) => {
                tracing::warn!(attempt = attempt + 1, "LLM attempt timed out");
                last_err = Some(LlmError::BadResponse("request timed out".into()));
            }
        }
    }
    Err(last_err.unwrap_or_else(|| LlmError::BadResponse("unknown".into())))
}

fn build_payload(
    system_prompt: &str,
    messages: &[Message],
    context_history_limit: usize,
    user_tz: &str,
    assistant_tz: Option<&str>,
) -> Vec<LlmMessage> {
    let mut out: Vec<LlmMessage> = Vec::new();

    if !system_prompt.is_empty() {
        let time_context = time_context_block(user_tz, assistant_tz);
        out.push(LlmMessage::new(
            Role::System,
            format!("{time_context}{system_prompt}"),
        ));
    }

    let window = context_history_limit.saturating_mul(2);
    let recent = if messages.len() > window {
        &messages[messages.len() - window..]
    } else {
        messages
    };

    let user_zone = parse_tz(user_tz);
    let asst_zone = assistant_tz
        .filter(|a| *a != user_tz)
        .and_then(parse_tz);

    for msg in recent {
        match msg.role {
            Role::User => {
                let prefix = user_time_prefix(&msg.timestamp, user_zone, asst_zone);
                out.push(LlmMessage::new(
                    Role::User,
                    format!("{prefix}\n{}", msg.content),
                ));
            }
            Role::Assistant | Role::System => {
                out.push(LlmMessage::new(msg.role, msg.content.clone()));
            }
        }
    }

    out
}

fn parse_tz(name: &str) -> Option<Tz> {
    name.parse().ok()
}

fn format_local(ts: DateTime<Utc>, zone: Option<Tz>) -> String {
    match zone {
        Some(tz) => ts
            .with_timezone(&tz)
            .format("%A, %B %d, %Y at %I:%M %p")
            .to_string(),
        None => ts.format("%A, %B %d, %Y at %I:%M %p UTC").to_string(),
    }
}

fn time_context_block(user_tz: &str, assistant_tz: Option<&str>) -> String {
    let now = Utc::now();
    let user_zone = parse_tz(user_tz);
    let asst_zone = assistant_tz
        .filter(|a| *a != user_tz)
        .and_then(parse_tz);

    let user_time = format_local(now, user_zone);

    if let Some(tz) = asst_zone {
        let asst_time = now
            .with_timezone(&tz)
            .format("%A, %B %d, %Y at %I:%M %p")
            .to_string();
        format!(
            "*** CURRENT TIME ***\nUser's time: {user_time}\nYour time: {asst_time}\n\n\
             When asked about or considering the time, use the times above. These are \
             accurate and updated with each message. Do not guess, assume, or make up \
             times. Do not say you lack real-time access — you are being given the \
             current time.\n\n",
        )
    } else {
        format!(
            "*** CURRENT TIME: {user_time} ***\nWhen asked about or considering the time, \
             use the time above. This time is accurate and updated with each message. \
             Do not guess, assume, or make up times. Do not say you lack real-time access \
             — you are being given the current time.\n\n",
        )
    }
}

fn user_time_prefix(timestamp: &str, user_zone: Option<Tz>, asst_zone: Option<Tz>) -> String {
    let parsed = DateTime::parse_from_rfc3339(timestamp)
        .map(|dt| dt.with_timezone(&Utc))
        .unwrap_or_else(|_| Utc.timestamp_opt(0, 0).single().unwrap_or_else(Utc::now));

    let user_time = format_local(parsed, user_zone);
    match asst_zone {
        Some(tz) => {
            let asst_time = parsed
                .with_timezone(&tz)
                .format("%A, %B %d, %Y at %I:%M %p")
                .to_string();
            format!("[User's time: {user_time} | Your time: {asst_time}]")
        }
        None => format!("[{user_time}]"),
    }
}
