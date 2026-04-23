//! OpenRouter provider. `catalog` handles the model-list + key-validation
//! surface; `chat` implements the `ChatLlm` completion path.

pub mod catalog;
pub mod chat;

// Re-export catalog's public surface so callers can reach it as
// `providers::openrouter::<name>` without the `::catalog::` hop.
pub use catalog::{
    DisplayModel, Model, ModelPricing, fetch_available_models, format_model_pricing,
    get_formatted_model_list, validate_api_key,
};
