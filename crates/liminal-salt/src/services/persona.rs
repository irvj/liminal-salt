//! Persona CRUD — owns `data/personas/{name}/` (identity `.md` files) and the
//! per-persona `config.json`. All persona-scoped state lives here; write flows
//! use `services::fs::write_atomic` so concurrent readers never see a
//! truncated file.
//!
//! Public functions: validation → filesystem op via `fs::write_atomic` →
//! typed `Result<_, PersonaError>`.

use std::{
    path::{Path, PathBuf},
    sync::LazyLock,
};

use regex::Regex;
use serde::{Deserialize, Serialize};

use crate::services::{memory, session};

// =============================================================================
// Validation
// =============================================================================

static PERSONA_NAME_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^[a-zA-Z0-9_]+$").expect("valid regex"));

/// True iff the name is safe to use as a directory name. Alphanumeric plus
/// underscore — matches Python's `_validate_persona_name`.
pub fn valid_persona_name(name: &str) -> bool {
    !name.is_empty() && name.len() <= 64 && PERSONA_NAME_RE.is_match(name)
}

// =============================================================================
// Paths
// =============================================================================

pub fn personas_dir(data_dir: &Path) -> PathBuf {
    data_dir.join("personas")
}

pub fn persona_dir(data_dir: &Path, name: &str) -> PathBuf {
    personas_dir(data_dir).join(name)
}

pub fn identity_file(data_dir: &Path, name: &str) -> PathBuf {
    persona_dir(data_dir, name).join("identity.md")
}

pub fn config_file(data_dir: &Path, name: &str) -> PathBuf {
    persona_dir(data_dir, name).join("config.json")
}

fn persona_user_context_dir(data_dir: &Path, name: &str) -> PathBuf {
    data_dir.join("user_context").join("personas").join(name)
}

// =============================================================================
// Persona config.json shape
// =============================================================================

/// Per-persona configuration. All fields optional — Python stores only the
/// keys that differ from global defaults. `extras` preserves unknown keys
/// through a load → save roundtrip.
#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(default)]
pub struct PersonaConfig {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,

    /// `"roleplay"` or absent. Python never persists `"chatbot"` (it's the
    /// unwritten baseline).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub default_mode: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub default_thread_memory_settings: Option<ThreadMemoryDefaults>,

    // Memory settings (consumed by `memory` + `memory_worker`).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub user_history_max_threads: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub user_history_messages_per_thread: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub memory_size_limit: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub auto_memory_interval: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub auto_memory_message_floor: Option<u32>,

    /// Catch-all for unknown keys so they survive load → save.
    #[serde(flatten)]
    pub extras: std::collections::BTreeMap<String, serde_json::Value>,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize, PartialEq, Eq)]
pub struct ThreadMemoryDefaults {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub interval_minutes: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub message_floor: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub size_limit: Option<u32>,
}

// =============================================================================
// Summary / listing
// =============================================================================

#[derive(Clone, Debug, Serialize)]
pub struct PersonaSummary {
    pub name: String,
    pub has_identity: bool,
}

/// All persona folders under `<data_dir>/personas/` that contain at least one
/// `.md` file. Sorted alphabetically.
pub async fn list_personas(data_dir: &Path) -> Vec<String> {
    let root = personas_dir(data_dir);
    let mut entries = match tokio::fs::read_dir(&root).await {
        Ok(e) => e,
        Err(_) => return Vec::new(),
    };
    let mut names = Vec::new();
    while let Ok(Some(entry)) = entries.next_entry().await {
        let Ok(ft) = entry.file_type().await else { continue };
        if !ft.is_dir() {
            continue;
        }
        let name = entry.file_name().to_string_lossy().to_string();
        if !valid_persona_name(&name) {
            continue;
        }
        if dir_has_markdown(&entry.path()).await {
            names.push(name);
        }
    }
    names.sort();
    names
}

async fn dir_has_markdown(dir: &Path) -> bool {
    let Ok(mut entries) = tokio::fs::read_dir(dir).await else {
        return false;
    };
    while let Ok(Some(entry)) = entries.next_entry().await {
        if entry.file_name().to_string_lossy().ends_with(".md") {
            return true;
        }
    }
    false
}

pub async fn persona_exists(data_dir: &Path, name: &str) -> bool {
    if !valid_persona_name(name) {
        return false;
    }
    tokio::fs::try_exists(persona_dir(data_dir, name))
        .await
        .unwrap_or(false)
}

// =============================================================================
// Identity
// =============================================================================

/// Concatenated identity content (all `.md` files in the persona dir, alpha
/// order, joined by newlines). Returns empty string if the persona is missing.
pub async fn load_identity(data_dir: &Path, name: &str) -> String {
    if !valid_persona_name(name) {
        return String::new();
    }
    let dir = persona_dir(data_dir, name);
    let mut entries = match tokio::fs::read_dir(&dir).await {
        Ok(e) => e,
        Err(_) => return String::new(),
    };
    let mut files: Vec<(String, PathBuf)> = Vec::new();
    while let Ok(Some(entry)) = entries.next_entry().await {
        let name = entry.file_name().to_string_lossy().to_string();
        if name.ends_with(".md") {
            files.push((name, entry.path()));
        }
    }
    files.sort_by(|a, b| a.0.cmp(&b.0));

    let mut out = String::new();
    for (_, path) in files {
        if let Ok(body) = tokio::fs::read_to_string(&path).await {
            out.push_str(&body);
            out.push('\n');
        }
    }
    out.trim().to_string()
}

/// Returns the first paragraph of the first `.md` file, truncated — used by
/// persona settings UIs for a quick preview. Empty string if missing.
pub async fn get_preview(data_dir: &Path, name: &str) -> String {
    if !valid_persona_name(name) {
        return String::new();
    }
    let dir = persona_dir(data_dir, name);
    let Ok(mut entries) = tokio::fs::read_dir(&dir).await else {
        return String::new();
    };
    let mut first: Option<PathBuf> = None;
    let mut first_name: Option<String> = None;
    while let Ok(Some(entry)) = entries.next_entry().await {
        let fname = entry.file_name().to_string_lossy().to_string();
        if !fname.ends_with(".md") {
            continue;
        }
        if first_name
            .as_deref()
            .is_none_or(|existing| fname.as_str() < existing)
        {
            first_name = Some(fname.clone());
            first = Some(entry.path());
        }
    }
    let Some(path) = first else {
        return String::new();
    };
    tokio::fs::read_to_string(&path)
        .await
        .unwrap_or_default()
}

/// Overwrite the identity file (identity.md). Creates the persona directory
/// if missing. Returns false on invalid name or write error.
pub async fn save_identity(data_dir: &Path, name: &str, content: &str) -> bool {
    if !valid_persona_name(name) {
        return false;
    }
    let dir = persona_dir(data_dir, name);
    if let Err(err) = tokio::fs::create_dir_all(&dir).await {
        tracing::error!(?dir, error = %err, "persona dir create failed");
        return false;
    }
    crate::services::fs::write_atomic(&identity_file(data_dir, name), content.as_bytes())
        .await
        .is_ok()
}

// =============================================================================
// CRUD
// =============================================================================

#[derive(Debug, thiserror::Error)]
pub enum PersonaError {
    #[error("invalid persona name")]
    InvalidName,
    #[error("persona already exists")]
    AlreadyExists,
    #[error("persona not found")]
    NotFound,
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
}

/// Create a new persona directory + identity file.
pub async fn create_persona(
    data_dir: &Path,
    name: &str,
    identity_content: &str,
) -> Result<(), PersonaError> {
    if !valid_persona_name(name) {
        return Err(PersonaError::InvalidName);
    }
    let dir = persona_dir(data_dir, name);
    if tokio::fs::try_exists(&dir).await.unwrap_or(false) {
        return Err(PersonaError::AlreadyExists);
    }
    tokio::fs::create_dir_all(&dir).await?;
    crate::services::fs::write_atomic(&identity_file(data_dir, name), identity_content.as_bytes()).await?;
    Ok(())
}

/// Delete the persona directory, its memory file, and its user-context scope.
/// Best-effort — logs but doesn't abort on partial failure.
pub async fn delete_persona(data_dir: &Path, name: &str) -> Result<(), PersonaError> {
    if !valid_persona_name(name) {
        return Err(PersonaError::InvalidName);
    }
    let dir = persona_dir(data_dir, name);
    if !tokio::fs::try_exists(&dir).await.unwrap_or(false) {
        return Err(PersonaError::NotFound);
    }

    if let Err(err) = tokio::fs::remove_dir_all(&dir).await {
        tracing::error!(persona = name, error = %err, "persona dir delete failed");
        return Err(PersonaError::Io(err));
    }

    // Cascade: memory file via its owning service.
    if let Err(err) = memory::delete_memory(data_dir, name).await {
        tracing::warn!(persona = name, error = %err, "memory file delete failed");
    }

    // Cascade: persona user-context dir.
    let ctx = persona_user_context_dir(data_dir, name);
    if tokio::fs::try_exists(&ctx).await.unwrap_or(false)
        && let Err(err) = tokio::fs::remove_dir_all(&ctx).await
    {
        tracing::warn!(persona = name, error = %err, "persona user-context delete failed");
    }

    Ok(())
}

/// Rename a persona. Orchestrates the 4-way cascade:
/// 1. rename `data/personas/{old}/` → `data/personas/{new}/`
/// 2. rename `data/memory/{old}.md` → `data/memory/{new}.md` (if exists)
/// 3. rename `data/user_context/personas/{old}/` → `.../{new}/` (if exists)
/// 4. rewrite every session file's `persona` field (via `session::update_persona_across_sessions`)
///
/// Best-effort after step 1 — if a later step fails it's logged but not rolled
/// back. Matches CLAUDE.md's "log and continue" acceptance from the roadmap.
pub async fn rename_persona(
    data_dir: &Path,
    old_name: &str,
    new_name: &str,
) -> Result<(), PersonaError> {
    if !valid_persona_name(old_name) || !valid_persona_name(new_name) {
        return Err(PersonaError::InvalidName);
    }
    if old_name == new_name {
        return Ok(());
    }
    let old_dir = persona_dir(data_dir, old_name);
    let new_dir = persona_dir(data_dir, new_name);
    if !tokio::fs::try_exists(&old_dir).await.unwrap_or(false) {
        return Err(PersonaError::NotFound);
    }
    if tokio::fs::try_exists(&new_dir).await.unwrap_or(false) {
        return Err(PersonaError::AlreadyExists);
    }

    // Step 1: directory rename. Fatal if this fails.
    tokio::fs::rename(&old_dir, &new_dir).await?;

    // Step 2: memory file via its owning service. Log and continue.
    if let Err(err) = memory::rename_memory(data_dir, old_name, new_name).await {
        tracing::warn!(old_name, new_name, error = %err, "memory rename failed");
    }

    // Step 3: persona user-context dir.
    let old_ctx = persona_user_context_dir(data_dir, old_name);
    if tokio::fs::try_exists(&old_ctx).await.unwrap_or(false) {
        let new_ctx = persona_user_context_dir(data_dir, new_name);
        if let Err(err) = tokio::fs::rename(&old_ctx, &new_ctx).await {
            tracing::warn!(old_name, new_name, error = %err, "persona context rename failed");
        }
    }

    // Step 4: session file rewrite. session_manager already handles per-session locking.
    session::update_persona_across_sessions(
        &data_dir.join("sessions"),
        old_name,
        new_name,
    )
    .await;

    Ok(())
}

// =============================================================================
// Persona config.json
// =============================================================================

pub async fn load_persona_config(data_dir: &Path, name: &str) -> PersonaConfig {
    if !valid_persona_name(name) {
        return PersonaConfig::default();
    }
    let path = config_file(data_dir, name);
    let bytes = match tokio::fs::read(&path).await {
        Ok(b) => b,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => return PersonaConfig::default(),
        Err(err) => {
            tracing::error!(?path, error = %err, "persona config read failed");
            return PersonaConfig::default();
        }
    };
    match serde_json::from_slice(&bytes) {
        Ok(cfg) => cfg,
        Err(err) => {
            tracing::error!(?path, error = %err, "persona config parse failed");
            PersonaConfig::default()
        }
    }
}

pub async fn save_persona_config(
    data_dir: &Path,
    name: &str,
    config: &PersonaConfig,
) -> Result<(), PersonaError> {
    if !valid_persona_name(name) {
        return Err(PersonaError::InvalidName);
    }
    let dir = persona_dir(data_dir, name);
    tokio::fs::create_dir_all(&dir).await?;
    let bytes = serde_json::to_vec_pretty(config)
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
    crate::services::fs::write_atomic(&config_file(data_dir, name), &bytes).await?;
    Ok(())
}

// =============================================================================
// Shared writer
// =============================================================================


#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn valid_persona_name_basics() {
        assert!(valid_persona_name("assistant"));
        assert!(valid_persona_name("my_persona_1"));
        assert!(!valid_persona_name(""));
        assert!(!valid_persona_name("with space"));
        assert!(!valid_persona_name("../escape"));
        assert!(!valid_persona_name("has-hyphen"));
    }
}
