use std::{
    collections::BTreeMap,
    path::{Path, PathBuf},
    sync::LazyLock,
};

use regex::Regex;
use serde::{Deserialize, Serialize};
use tokio::io::AsyncWriteExt;

/// App configuration. Loaded from `<data_dir>/config.json` and re-saved whenever
/// settings change. Field names serialize as `snake_case` (matches persona
/// configs and session JSON). `extras` catches any unknown keys so they
/// round-trip through load → save untouched.
#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(default)]
pub struct AppConfig {
    pub setup_complete: bool,
    pub agreement_accepted: String,
    pub openrouter_api_key: String,
    pub provider: String,
    pub model: String,
    pub default_persona: String,
    pub theme: String,
    pub theme_mode: String,
    pub context_history_limit: u32,

    #[serde(flatten)]
    pub extras: BTreeMap<String, serde_json::Value>,
}

/// App is accessible only when setup has finished AND the user has accepted the
/// current agreement version. Either missing → wizard.
pub fn is_app_ready(config: &AppConfig) -> bool {
    config.setup_complete && config.agreement_accepted == current_agreement_version()
}

/// Production data-dir resolver. In Tauri (M2) this is the only function that
/// changes — it will return `app_data_dir()` instead of a repo-relative path.
pub fn data_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../data")
}

pub fn sessions_dir(data_dir: &Path) -> PathBuf {
    data_dir.join("sessions")
}

pub fn config_file(data_dir: &Path) -> PathBuf {
    data_dir.join("config.json")
}

/// Load config from `<data_dir>/config.json`. Missing file or corrupt JSON both
/// return `AppConfig::default()` — matches Python's `load_config()` behavior.
pub async fn load_config(data_dir: &Path) -> AppConfig {
    let path = config_file(data_dir);
    let bytes = match tokio::fs::read(&path).await {
        Ok(b) => b,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => return AppConfig::default(),
        Err(err) => {
            tracing::error!(?path, error = %err, "config read failed");
            return AppConfig::default();
        }
    };
    match serde_json::from_slice::<AppConfig>(&bytes) {
        Ok(cfg) => cfg,
        Err(err) => {
            tracing::error!(?path, error = %err, "config file corrupt");
            AppConfig::default()
        }
    }
}

/// Save config atomically-ish: write+flush+fsync into the target file. No rename
/// dance, matching Python semantics.
pub async fn save_config(data_dir: &Path, config: &AppConfig) -> std::io::Result<()> {
    if let Some(parent) = config_file(data_dir).parent() {
        tokio::fs::create_dir_all(parent).await?;
    }
    let bytes = serde_json::to_vec_pretty(config)
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
    let mut f = tokio::fs::OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(true)
        .open(config_file(data_dir))
        .await?;
    f.write_all(&bytes).await?;
    f.sync_all().await?;
    Ok(())
}

pub async fn config_file_exists(data_dir: &Path) -> bool {
    tokio::fs::try_exists(config_file(data_dir))
        .await
        .unwrap_or(false)
}

// =============================================================================
// Agreement
// =============================================================================

static VERSION_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?i)<!--\s*version:\s*(\S+)\s*-->").expect("valid regex"));

fn agreement_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../AGREEMENT.md")
}

fn load_agreement() -> (String, String) {
    let path = agreement_path();
    let text = match std::fs::read_to_string(&path) {
        Ok(t) => t,
        Err(err) => {
            tracing::warn!(?path, error = %err, "AGREEMENT.md not found");
            return ("0.0".to_string(), String::new());
        }
    };
    let version = VERSION_RE
        .captures(&text)
        .and_then(|c| c.get(1))
        .map(|m| m.as_str().to_string())
        .unwrap_or_else(|| "0.0".to_string());
    let body = VERSION_RE
        .replace(&text, "")
        .trim_start_matches('\n')
        .trim_end()
        .to_string();
    (version, body)
}

pub struct Agreement {
    pub version: String,
    pub body: String,
}

/// Parsed once at process start. Restart to pick up edits to AGREEMENT.md.
pub static AGREEMENT: LazyLock<Agreement> = LazyLock::new(|| {
    let (version, body) = load_agreement();
    Agreement { version, body }
});

pub fn current_agreement_version() -> &'static str {
    AGREEMENT.version.as_str()
}

// =============================================================================
// Providers
// =============================================================================

/// Available API providers. Currently just OpenRouter; the setup/settings UI
/// treats this as the canonical list, so new providers only need adding here.
#[derive(Clone, Debug, Serialize)]
pub struct Provider {
    pub id: &'static str,
    pub name: &'static str,
    pub api_key_url: &'static str,
    pub api_key_placeholder: &'static str,
}

pub const PROVIDERS: &[Provider] = &[Provider {
    id: "openrouter",
    name: "OpenRouter",
    api_key_url: "https://openrouter.ai/keys",
    api_key_placeholder: "sk-or-v1-...",
}];

pub fn get_providers() -> &'static [Provider] {
    PROVIDERS
}

pub fn get_provider_by_id(id: &str) -> Option<&'static Provider> {
    PROVIDERS.iter().find(|p| p.id == id)
}
