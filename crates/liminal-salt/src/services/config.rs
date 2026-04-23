use std::{
    collections::BTreeMap,
    path::{Path, PathBuf},
    sync::LazyLock,
};

use regex::Regex;
use serde::{Deserialize, Serialize};

/// App configuration. Loaded from `<data_dir>/config.json` and re-saved whenever
/// settings change. Field names serialize as `snake_case` (matches persona
/// configs and session JSON). `extras` catches any unknown keys so they
/// round-trip through load → save untouched.
#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(default)]
pub struct AppConfig {
    pub setup_complete: bool,
    pub agreement_accepted: String,
    /// Legacy field, kept during the multi-provider transition. Handlers still
    /// write here; `load_config` mirrors its value into `api_key` on every load
    /// so the provider-neutral read path stays in sync. Removed once writers
    /// switch to `api_key` directly.
    pub openrouter_api_key: String,
    /// Provider-neutral key field. Authoritative on read; populated from
    /// `openrouter_api_key` by `migrate_legacy_fields` during the transition.
    pub api_key: String,
    pub provider: String,
    pub model: String,
    pub default_persona: String,
    pub theme: String,
    pub theme_mode: String,
    pub context_history_limit: u32,

    #[serde(flatten)]
    pub extras: BTreeMap<String, serde_json::Value>,
}

/// Normalize a freshly-loaded config for the multi-provider transition:
/// mirror the legacy `openrouter_api_key` into `api_key` when present, and
/// backfill `provider` for configs that predate that field.
///
/// The legacy field wins on mismatch because writers still target it during
/// this phase of the refactor. Once writers switch to `api_key` (commit 3),
/// the legacy field is removed and this migration reads it out of `extras`.
fn migrate_legacy_fields(cfg: &mut AppConfig) {
    if !cfg.openrouter_api_key.is_empty() {
        cfg.api_key = cfg.openrouter_api_key.clone();
    }
    if cfg.provider.is_empty() && !cfg.api_key.is_empty() {
        cfg.provider = "openrouter".to_string();
    }
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
        Ok(mut cfg) => {
            migrate_legacy_fields(&mut cfg);
            cfg
        }
        Err(err) => {
            tracing::error!(?path, error = %err, "config file corrupt");
            AppConfig::default()
        }
    }
}

/// Save config atomically: write to `<path>.tmp`, fsync, rename. Concurrent
/// per-request reads (`load_config`) never see a truncated file.
pub async fn save_config(data_dir: &Path, config: &AppConfig) -> std::io::Result<()> {
    let bytes = serde_json::to_vec_pretty(config)
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
    crate::services::fs::write_atomic(&config_file(data_dir), &bytes).await
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn migration_mirrors_legacy_key_and_backfills_provider() {
        let mut cfg = AppConfig {
            openrouter_api_key: "sk-or-test".to_string(),
            ..Default::default()
        };
        migrate_legacy_fields(&mut cfg);
        assert_eq!(cfg.api_key, "sk-or-test");
        assert_eq!(cfg.openrouter_api_key, "sk-or-test");
        assert_eq!(cfg.provider, "openrouter");
    }

    #[test]
    fn migration_preserves_new_only_config_and_backfills_provider() {
        let mut cfg = AppConfig {
            api_key: "sk-new".to_string(),
            ..Default::default()
        };
        migrate_legacy_fields(&mut cfg);
        assert_eq!(cfg.api_key, "sk-new");
        assert_eq!(cfg.openrouter_api_key, "");
        assert_eq!(cfg.provider, "openrouter");
    }

    #[test]
    fn migration_legacy_wins_during_transition() {
        // Writers still target openrouter_api_key; a stale api_key on disk
        // must be overwritten on load so reads stay in sync with writes.
        let mut cfg = AppConfig {
            openrouter_api_key: "sk-or-fresh".to_string(),
            api_key: "sk-stale".to_string(),
            ..Default::default()
        };
        migrate_legacy_fields(&mut cfg);
        assert_eq!(cfg.api_key, "sk-or-fresh");
    }

    #[test]
    fn migration_no_op_on_empty_config() {
        let mut cfg = AppConfig::default();
        migrate_legacy_fields(&mut cfg);
        assert_eq!(cfg.api_key, "");
        assert_eq!(cfg.openrouter_api_key, "");
        assert_eq!(cfg.provider, "");
    }

    #[tokio::test]
    async fn load_config_migrates_legacy_openrouter_key_from_disk() {
        let tmp = tempfile::tempdir().unwrap();
        let legacy_json = r#"{
            "setup_complete": true,
            "agreement_accepted": "1.0",
            "openrouter_api_key": "sk-or-legacy",
            "model": "anthropic/claude-opus-4"
        }"#;
        tokio::fs::write(tmp.path().join("config.json"), legacy_json)
            .await
            .unwrap();

        let cfg = load_config(tmp.path()).await;
        assert_eq!(cfg.api_key, "sk-or-legacy");
        assert_eq!(cfg.openrouter_api_key, "sk-or-legacy");
        assert_eq!(cfg.provider, "openrouter");
        assert_eq!(cfg.model, "anthropic/claude-opus-4");
        assert!(cfg.setup_complete);
    }
}
