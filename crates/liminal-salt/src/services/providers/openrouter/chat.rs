//! OpenRouter `/v1/chat/completions` client. Implements `ChatLlm` so the
//! rest of the app can dispatch completions through a provider-neutral trait.

use std::time::Duration;

use reqwest::Client;
use serde::{Deserialize, Serialize};

use crate::services::llm::{ChatLlm, LlmError, LlmMessage};

const OPENROUTER_API_URL: &str = "https://openrouter.ai/api/v1/chat/completions";

// App attribution sent to OpenRouter on every call. See
// https://openrouter.ai/docs/app-attribution.
const APP_URL: &str = "https://liminalsalt.app";
const APP_NAME: &str = "Liminal Salt";
const APP_CATEGORIES: &str = "general-chat,roleplay";

pub struct LlmClient {
    api_key: String,
    model: String,
    client: Client,
    timeout: Duration,
}

impl ChatLlm for LlmClient {
    async fn complete(&self, messages: &[LlmMessage]) -> Result<String, LlmError> {
        self.call_llm(messages).await
    }
}

impl LlmClient {
    pub fn new(api_key: impl Into<String>, model: impl Into<String>) -> Self {
        Self {
            api_key: api_key.into(),
            model: model.into(),
            client: Client::new(),
            timeout: Duration::from_secs(30),
        }
    }

    pub fn with_timeout(mut self, timeout: Duration) -> Self {
        self.timeout = timeout;
        self
    }

    /// Replace the internal HTTP client. Use to share a single `reqwest::Client`
    /// (and its connection pool) across multiple `LlmClient` instances.
    pub fn with_http_client(mut self, client: Client) -> Self {
        self.client = client;
        self
    }

    /// Convenience constructor that wires a config-derived `LlmClient` to the
    /// app's shared `reqwest::Client`. Handlers must use this rather than
    /// `LlmClient::new` directly (CLAUDE.md "Handlers do not do work").
    pub fn from_config(http: &Client, api_key: &str, model: &str) -> Self {
        Self::new(api_key, model).with_http_client(http.clone())
    }

    pub async fn call_llm(&self, messages: &[LlmMessage]) -> Result<String, LlmError> {
        if self.api_key.is_empty() {
            return Err(LlmError::NoApiKey);
        }

        #[derive(Serialize)]
        struct ReqBody<'a> {
            model: &'a str,
            messages: &'a [LlmMessage],
        }

        let response = self
            .client
            .post(OPENROUTER_API_URL)
            .bearer_auth(&self.api_key)
            .header("HTTP-Referer", APP_URL)
            .header("X-OpenRouter-Title", APP_NAME)
            .header("X-OpenRouter-Categories", APP_CATEGORIES)
            .json(&ReqBody {
                model: &self.model,
                messages,
            })
            .timeout(self.timeout)
            .send()
            .await?;

        let status = response.status();
        if !status.is_success() {
            let body = response.text().await.unwrap_or_default();
            return Err(LlmError::BadStatus {
                status: status.as_u16(),
                body,
            });
        }

        #[derive(Deserialize)]
        struct RespBody {
            choices: Vec<Choice>,
        }
        #[derive(Deserialize)]
        struct Choice {
            message: ChoiceMessage,
        }
        #[derive(Deserialize)]
        struct ChoiceMessage {
            content: String,
        }

        let parsed: RespBody = response
            .json()
            .await
            .map_err(|e| LlmError::BadResponse(format!("invalid JSON: {e}")))?;

        let content = parsed
            .choices
            .into_iter()
            .next()
            .ok_or_else(|| LlmError::BadResponse("no choices returned".into()))?
            .message
            .content;

        // Strip token artifacts some models leak, then trim.
        let cleaned = content.replace("<s>", "").replace("</s>", "");
        let cleaned = cleaned.trim();
        if cleaned.is_empty() {
            return Err(LlmError::BadResponse("empty content".into()));
        }
        Ok(cleaned.to_string())
    }
}
