//! Provider-neutral LLM completion trait + shared types. Concrete clients
//! live under `services/providers/`; this module only defines the seam.
//!
//! New LLM features (summarization, memory merge, etc.) add methods to their
//! owning manager that call `ChatLlm::complete` — never a direct reqwest
//! elsewhere in the codebase.

use serde::Serialize;

use crate::services::session::Role;

#[derive(Debug, thiserror::Error)]
pub enum LlmError {
    #[error("no API key configured")]
    NoApiKey,
    #[error("network error: {0}")]
    Network(#[from] reqwest::Error),
    #[error("HTTP {status}: {body}")]
    BadStatus { status: u16, body: String },
    #[error("bad response: {0}")]
    BadResponse(String),
}

/// Role + content payload as accepted by chat-completions APIs. Distinct
/// from `session::Message` because APIs do not accept (or want) per-message
/// timestamps. Callers convert before dispatch.
#[derive(Clone, Debug, Serialize)]
pub struct LlmMessage {
    pub role: Role,
    pub content: String,
}

impl LlmMessage {
    pub fn new(role: Role, content: impl Into<String>) -> Self {
        Self {
            role,
            content: content.into(),
        }
    }
}

/// Abstraction for "something that can complete an LLM chat turn." Concrete
/// production impls live under `services/providers/`; tests use a fake that
/// returns a canned response without hitting the network.
pub trait ChatLlm: Send + Sync {
    fn complete(
        &self,
        messages: &[LlmMessage],
    ) -> impl std::future::Future<Output = Result<String, LlmError>> + Send;
}
