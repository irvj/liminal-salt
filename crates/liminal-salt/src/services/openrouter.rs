//! Direct HTTP to OpenRouter for the parts of the app that aren't LLM chat
//! completions — API-key validation and the model catalog.
//!
//! `services/llm.rs` is still the single path to `/chat/completions`; this
//! module handles `/auth/key` and `/models`. Keeping them separate means the
//! chat path stays a tight, async-ChatLlm-shaped surface while the setup /
//! settings flows get their own simple request→JSON helpers.

use std::time::Duration;

use reqwest::Client;
use serde::{Deserialize, Serialize};

const OPENROUTER_AUTH_URL: &str = "https://openrouter.ai/api/v1/auth/key";
const OPENROUTER_MODELS_URL: &str = "https://openrouter.ai/api/v1/models";
const NETWORK_TIMEOUT: Duration = Duration::from_secs(10);

// =============================================================================
// Types
// =============================================================================

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct ModelPricing {
    #[serde(default)]
    pub prompt: String,
    #[serde(default)]
    pub completion: String,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct Model {
    pub id: String,
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub pricing: Option<ModelPricing>,
    #[serde(default)]
    pub context_length: u64,
}

#[derive(Clone, Debug, Serialize)]
pub struct DisplayModel {
    pub id: String,
    pub display: String,
}

// =============================================================================
// API calls
// =============================================================================

/// Validate an OpenRouter API key by hitting `/auth/key`. 200 → valid; any
/// other status, network error, or timeout → false. Matches Python's
/// `config_manager.validate_api_key` — log-and-return-bool, no error propagation.
pub async fn validate_api_key(http: &Client, api_key: &str) -> bool {
    if api_key.is_empty() {
        return false;
    }
    let request = http
        .get(OPENROUTER_AUTH_URL)
        .bearer_auth(api_key)
        .timeout(NETWORK_TIMEOUT);
    match request.send().await {
        Ok(resp) if resp.status().is_success() => {
            tracing::info!("API key validated");
            true
        }
        Ok(resp) => {
            tracing::error!(status = %resp.status(), "API key validation failed");
            false
        }
        Err(err) => {
            tracing::error!(error = %err, "network error validating API key");
            false
        }
    }
}

/// Fetch the full model catalog. Returns `None` on any failure — matches
/// Python's "log the error, surface empty-list to the caller" behavior so
/// the wizard / settings UI can show a generic "couldn't fetch" message
/// instead of leaking HTTP internals.
pub async fn fetch_available_models(http: &Client, api_key: &str) -> Option<Vec<Model>> {
    #[derive(Deserialize)]
    struct ModelsResponse {
        #[serde(default)]
        data: Vec<Model>,
    }

    let request = http
        .get(OPENROUTER_MODELS_URL)
        .bearer_auth(api_key)
        .timeout(NETWORK_TIMEOUT);
    let resp = match request.send().await {
        Ok(r) => r,
        Err(err) => {
            tracing::error!(error = %err, "network error fetching models");
            return None;
        }
    };
    if !resp.status().is_success() {
        tracing::error!(status = %resp.status(), "OpenRouter /models returned non-success");
        return None;
    }
    match resp.json::<ModelsResponse>().await {
        Ok(body) => {
            tracing::info!(count = body.data.len(), "fetched OpenRouter models");
            Some(body.data)
        }
        Err(err) => {
            tracing::error!(error = %err, "failed to parse OpenRouter /models response");
            None
        }
    }
}

// =============================================================================
// Display transforms
// =============================================================================

/// Format a model's pricing as a human-readable per-1M-tokens string.
/// Matches Python's `utils.format_model_pricing`: "Free" when both are zero,
/// "$X/$Y per 1M" otherwise. 4-decimal precision below $0.01; 2 otherwise.
pub fn format_model_pricing(pricing: Option<&ModelPricing>) -> String {
    let Some(p) = pricing else { return String::new() };
    let prompt: f64 = p.prompt.parse().unwrap_or(0.0);
    let completion: f64 = p.completion.parse().unwrap_or(0.0);
    if prompt == 0.0 && completion == 0.0 {
        return "Free".to_string();
    }
    let per_m_prompt = prompt * 1_000_000.0;
    let per_m_completion = completion * 1_000_000.0;
    let fmt = |v: f64| -> String {
        if v < 0.01 { format!("${v:.4}") } else { format!("${v:.2}") }
    };
    format!("{}/{} per 1M", fmt(per_m_prompt), fmt(per_m_completion))
}

/// One-shot "fetch and prepare for display" — matches Python's
/// `utils.get_formatted_model_list`. Empty key or network failure → empty
/// list, so the UI shows a clean "no models available" state.
pub async fn get_formatted_model_list(
    http: &Client,
    api_key: &str,
) -> Vec<DisplayModel> {
    if api_key.is_empty() {
        return Vec::new();
    }
    let Some(models) = fetch_available_models(http, api_key).await else {
        return Vec::new();
    };
    format_models(&models)
}

/// Group by provider (slash-prefix of model id), sort providers alphabetically,
/// sort models within each group by name, then flatten back into a single
/// list with `Provider: Name - $X/$Y` display strings.
fn format_models(models: &[Model]) -> Vec<DisplayModel> {
    use std::collections::BTreeMap;

    let mut groups: BTreeMap<String, Vec<&Model>> = BTreeMap::new();
    for m in models {
        let provider = m.id.split_once('/').map(|(p, _)| p).unwrap_or("Other");
        groups.entry(provider.to_string()).or_default().push(m);
    }

    let mut out = Vec::with_capacity(models.len());
    for (provider, mut items) in groups {
        items.sort_by(|a, b| a.name.cmp(&b.name));
        let provider_display = provider
            .replace('-', " ")
            .split_whitespace()
            .map(|w| {
                let mut chars = w.chars();
                match chars.next() {
                    Some(c) => c.to_uppercase().collect::<String>() + chars.as_str(),
                    None => String::new(),
                }
            })
            .collect::<Vec<_>>()
            .join(" ");

        for m in items {
            // Skip the provider prefix if the model name already starts with it
            // (matches Python's case-insensitive `startswith` check).
            let name = if m.name.to_lowercase().starts_with(&provider.to_lowercase()) {
                m.name.clone()
            } else {
                format!("{}: {}", provider_display, m.name)
            };
            let pricing = format_model_pricing(m.pricing.as_ref());
            let display = if pricing.is_empty() {
                name
            } else {
                format!("{name} - {pricing}")
            };
            out.push(DisplayModel {
                id: m.id.clone(),
                display,
            });
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn m(id: &str, name: &str, prompt: &str, completion: &str) -> Model {
        Model {
            id: id.to_string(),
            name: name.to_string(),
            pricing: Some(ModelPricing {
                prompt: prompt.to_string(),
                completion: completion.to_string(),
            }),
            context_length: 0,
        }
    }

    #[test]
    fn format_pricing_free_when_both_zero() {
        let p = ModelPricing {
            prompt: "0".into(),
            completion: "0".into(),
        };
        assert_eq!(format_model_pricing(Some(&p)), "Free");
    }

    #[test]
    fn format_pricing_dollars_per_million() {
        // $3 per 1M prompt, $15 per 1M completion.
        let p = ModelPricing {
            prompt: "0.000003".into(),
            completion: "0.000015".into(),
        };
        assert_eq!(format_model_pricing(Some(&p)), "$3.00/$15.00 per 1M");
    }

    #[test]
    fn format_pricing_four_decimals_below_a_cent() {
        // Tiny prompt cost — needs 4 decimals.
        let p = ModelPricing {
            prompt: "0.000000001".into(),
            completion: "0".into(),
        };
        // $0.001/$0.00 — completion is zero but prompt isn't, so not "Free".
        assert_eq!(format_model_pricing(Some(&p)), "$0.0010/$0.0000 per 1M");
    }

    #[test]
    fn format_pricing_none_returns_empty() {
        assert_eq!(format_model_pricing(None), "");
    }

    #[test]
    fn format_models_groups_and_sorts() {
        let models = vec![
            m("anthropic/claude", "Claude", "0", "0"),
            m("openai/gpt-4", "GPT-4", "0", "0"),
            m("anthropic/claude-opus", "Claude Opus", "0", "0"),
            m("openai/gpt-3.5", "GPT-3.5", "0", "0"),
        ];
        let out = format_models(&models);
        // BTreeMap sorts providers alphabetically: anthropic, openai.
        assert_eq!(out[0].id, "anthropic/claude");
        assert_eq!(out[1].id, "anthropic/claude-opus");
        assert_eq!(out[2].id, "openai/gpt-3.5");
        assert_eq!(out[3].id, "openai/gpt-4");
    }

    #[test]
    fn format_models_strips_redundant_provider_prefix() {
        // Name already starts with provider → don't double-prefix.
        let models = vec![m("anthropic/claude", "Anthropic Claude Opus", "0", "0")];
        let out = format_models(&models);
        assert_eq!(out[0].display, "Anthropic Claude Opus - Free");
        // Name doesn't start with provider → prefix added.
        let models = vec![m("openai/gpt-4", "GPT-4", "0", "0")];
        let out = format_models(&models);
        assert_eq!(out[0].display, "Openai: GPT-4 - Free");
    }

    #[test]
    fn format_models_unknown_provider_goes_to_other() {
        let models = vec![Model {
            id: "bare-id".to_string(),
            name: "Bare".to_string(),
            pricing: None,
            context_length: 0,
        }];
        let out = format_models(&models);
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].id, "bare-id");
    }
}
