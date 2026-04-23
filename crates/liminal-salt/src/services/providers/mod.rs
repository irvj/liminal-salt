//! Provider-neutral LLM dispatch. `Provider` is a closed enum — adding a new
//! backend means adding a variant and matching in the dispatch methods. That
//! gives compile-time exhaustiveness for free, at the cost of touching this
//! central file for each new provider (a feature: one place to audit coverage).
//!
//! `ProviderChatLlm` is the completion-side wrapper. `ChatLlm::complete` on
//! the enum dispatches to the underlying provider's concrete client. This
//! avoids trait objects (and the async-fn-in-trait dyn-compat limitations)
//! while keeping callers generic.

pub mod openrouter;

use crate::services::llm::{ChatLlm, LlmError, LlmMessage};

/// Every provider the app knows how to dispatch to. Add a variant + match
/// arms in every method below when onboarding a new provider.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Provider {
    OpenRouter,
}

pub const ALL: &[Provider] = &[Provider::OpenRouter];

pub fn by_id(id: &str) -> Option<Provider> {
    match id {
        "openrouter" => Some(Provider::OpenRouter),
        _ => None,
    }
}

/// Metadata for UI rendering. Serializable so the setup + settings templates
/// can consume the list as JSON without a transform step.
#[derive(Clone, Debug, serde::Serialize)]
pub struct ProviderMetadata {
    pub id: &'static str,
    pub name: &'static str,
    pub api_key_url: &'static str,
    pub api_key_placeholder: &'static str,
}

pub fn metadata_list() -> Vec<ProviderMetadata> {
    ALL.iter().map(|p| p.metadata()).collect()
}

impl Provider {
    pub fn id(&self) -> &'static str {
        match self {
            Provider::OpenRouter => "openrouter",
        }
    }

    pub fn metadata(&self) -> ProviderMetadata {
        match self {
            Provider::OpenRouter => ProviderMetadata {
                id: "openrouter",
                name: "OpenRouter",
                api_key_url: "https://openrouter.ai/keys",
                api_key_placeholder: "sk-or-v1-...",
            },
        }
    }

    pub async fn validate_key(&self, http: &reqwest::Client, api_key: &str) -> bool {
        match self {
            Provider::OpenRouter => openrouter::validate_api_key(http, api_key).await,
        }
    }

    pub async fn list_models(
        &self,
        http: &reqwest::Client,
        api_key: &str,
    ) -> Vec<openrouter::DisplayModel> {
        match self {
            Provider::OpenRouter => openrouter::get_formatted_model_list(http, api_key).await,
        }
    }

    pub fn build_chat_llm(
        &self,
        http: &reqwest::Client,
        api_key: &str,
        model: &str,
    ) -> ProviderChatLlm {
        match self {
            Provider::OpenRouter => ProviderChatLlm::OpenRouter(
                openrouter::chat::LlmClient::from_config(http, api_key, model),
            ),
        }
    }
}

/// Concrete `ChatLlm` impl holding whichever provider's client the caller
/// built. Match-dispatches to the underlying impl — no trait objects, no
/// `Box<dyn>`, just an enum.
pub enum ProviderChatLlm {
    OpenRouter(openrouter::chat::LlmClient),
}

impl ChatLlm for ProviderChatLlm {
    async fn complete(&self, messages: &[LlmMessage]) -> Result<String, LlmError> {
        match self {
            ProviderChatLlm::OpenRouter(c) => c.complete(messages).await,
        }
    }
}

impl ProviderChatLlm {
    pub fn with_timeout(self, timeout: std::time::Duration) -> Self {
        match self {
            ProviderChatLlm::OpenRouter(c) => ProviderChatLlm::OpenRouter(c.with_timeout(timeout)),
        }
    }
}
