//! ContextScope — owns `data/user_context/**/config.json` plus the uploaded
//! `.md`/`.txt` files and the local-directory references that live alongside
//! them.
//!
//! There are two scopes at runtime: **global** (`data/user_context/`) and
//! **per-persona** (`data/user_context/personas/{name}/`). Both share the
//! same config.json shape and the same set of operations — the only runtime
//! differences are the base directory and the header label emitted by
//! `load_enabled_context()`.

use std::{
    collections::BTreeMap,
    path::{Path, PathBuf},
};

use serde::{Deserialize, Serialize};
use tokio::io::AsyncWriteExt;

use crate::services::local_context;

const CONFIG_FILE: &str = "config.json";

// =============================================================================
// On-disk shape
// =============================================================================

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(default)]
pub struct ScopeConfig {
    pub files: BTreeMap<String, FileState>,
    pub local_directories: BTreeMap<String, LocalDirectoryState>,
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize)]
pub struct FileState {
    pub enabled: bool,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(default)]
pub struct LocalDirectoryState {
    pub files: BTreeMap<String, FileState>,
}

// =============================================================================
// Summary types
// =============================================================================

#[derive(Clone, Debug, Serialize)]
pub struct ContextFileEntry {
    pub name: String,
    pub enabled: bool,
}

#[derive(Clone, Debug, Serialize)]
pub struct LocalDirectoryEntry {
    pub path: String,
    pub exists: bool,
    pub files: Vec<LocalFileEntry>,
}

#[derive(Clone, Debug, Serialize)]
pub struct LocalFileEntry {
    pub name: String,
    pub enabled: bool,
    pub exists_on_disk: bool,
}

// =============================================================================
// ContextScope
// =============================================================================

pub struct ContextScope {
    base_dir: PathBuf,
    scope_label: &'static str,
    header_description: &'static str,
}

impl ContextScope {
    /// The global user-level context scope at `data/user_context/`.
    pub fn global(data_dir: &Path) -> Self {
        Self {
            base_dir: data_dir.join("user_context"),
            scope_label: "USER",
            header_description: "The following files were provided by the user as additional context.",
        }
    }

    /// The per-persona context scope at `data/user_context/personas/{name}/`.
    pub fn persona(data_dir: &Path, persona_name: &str) -> Self {
        Self {
            base_dir: data_dir
                .join("user_context")
                .join("personas")
                .join(persona_name),
            scope_label: "PERSONA",
            header_description: "The following files provide additional context for this persona.",
        }
    }

    pub fn base_dir(&self) -> &Path {
        &self.base_dir
    }

    fn config_path(&self) -> PathBuf {
        self.base_dir.join(CONFIG_FILE)
    }

    // ---------------------------------------------------------------------
    // Config I/O
    // ---------------------------------------------------------------------

    pub async fn load_config(&self) -> ScopeConfig {
        let path = self.config_path();
        let bytes = match tokio::fs::read(&path).await {
            Ok(b) => b,
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => return ScopeConfig::default(),
            Err(err) => {
                tracing::error!(?path, error = %err, "context config read failed");
                return ScopeConfig::default();
            }
        };
        serde_json::from_slice(&bytes).unwrap_or_else(|err| {
            tracing::error!(?path, error = %err, "context config parse failed");
            ScopeConfig::default()
        })
    }

    async fn save_config(&self, config: &ScopeConfig) -> std::io::Result<()> {
        tokio::fs::create_dir_all(&self.base_dir).await?;
        let bytes = serde_json::to_vec_pretty(config)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
        write_file_durable(&self.config_path(), &bytes).await
    }

    // ---------------------------------------------------------------------
    // Uploaded files
    // ---------------------------------------------------------------------

    pub async fn list_files(&self) -> Vec<ContextFileEntry> {
        let config = self.load_config().await;
        let mut out: Vec<ContextFileEntry> = config
            .files
            .into_iter()
            .map(|(name, state)| ContextFileEntry {
                name,
                enabled: state.enabled,
            })
            .collect();
        out.sort_by(|a, b| a.name.cmp(&b.name));
        out
    }

    /// Write the uploaded file bytes and mark it enabled in config. Filename
    /// is sanitized via `basename` to block directory traversal.
    pub async fn upload_file(&self, filename: &str, bytes: &[u8]) -> Option<String> {
        let safe = sanitize_filename(filename)?;
        tokio::fs::create_dir_all(&self.base_dir).await.ok()?;
        let path = self.base_dir.join(&safe);
        if write_file_durable(&path, bytes).await.is_err() {
            return None;
        }
        let mut cfg = self.load_config().await;
        cfg.files.insert(safe.clone(), FileState { enabled: true });
        if self.save_config(&cfg).await.is_err() {
            return None;
        }
        Some(safe)
    }

    pub async fn delete_file(&self, filename: &str) -> bool {
        let Some(safe) = sanitize_filename(filename) else {
            return false;
        };
        let path = self.base_dir.join(&safe);
        let _ = tokio::fs::remove_file(&path).await;
        let mut cfg = self.load_config().await;
        let removed = cfg.files.remove(&safe).is_some();
        if removed {
            let _ = self.save_config(&cfg).await;
        }
        true
    }

    /// Toggle or explicitly set the `enabled` flag. Returns the new state, or
    /// `None` if the file isn't tracked in config.
    pub async fn toggle_file(&self, filename: &str, enabled: Option<bool>) -> Option<bool> {
        let safe = sanitize_filename(filename)?;
        let mut cfg = self.load_config().await;
        let entry = cfg.files.get_mut(&safe)?;
        entry.enabled = enabled.unwrap_or(!entry.enabled);
        let new_state = entry.enabled;
        self.save_config(&cfg).await.ok()?;
        Some(new_state)
    }

    pub async fn get_file_content(&self, filename: &str) -> Option<String> {
        let safe = sanitize_filename(filename)?;
        tokio::fs::read_to_string(self.base_dir.join(&safe)).await.ok()
    }

    pub async fn save_file_content(&self, filename: &str, content: &str) -> bool {
        let Some(safe) = sanitize_filename(filename) else {
            return false;
        };
        write_file_durable(&self.base_dir.join(&safe), content.as_bytes())
            .await
            .is_ok()
    }

    // ---------------------------------------------------------------------
    // Local directories (delegates to local_context for FS primitives)
    // ---------------------------------------------------------------------

    /// Add a local directory to the config as enabled and seed all files as
    /// enabled. Returns the resolved absolute path + the file list on success,
    /// or an error message for UI display.
    pub async fn add_local_directory(
        &self,
        dir_path: &str,
    ) -> Result<(String, Vec<local_context::LocalFile>), String> {
        let resolved = local_context::validate_directory_path(dir_path)?;
        let files = local_context::scan_directory(&resolved).await;

        let mut cfg = self.load_config().await;
        let key = resolved.to_string_lossy().to_string();
        let state = cfg
            .local_directories
            .entry(key.clone())
            .or_default();
        for file in &files {
            state
                .files
                .entry(file.name.clone())
                .or_insert(FileState { enabled: true });
        }
        self.save_config(&cfg)
            .await
            .map_err(|e| format!("config save failed: {e}"))?;
        Ok((key, files))
    }

    pub async fn remove_local_directory(&self, dir_path: &str) -> bool {
        let resolved = match local_context::resolve(dir_path) {
            Some(p) => p.to_string_lossy().to_string(),
            None => dir_path.to_string(),
        };
        let mut cfg = self.load_config().await;
        let removed = cfg.local_directories.remove(&resolved).is_some()
            || cfg.local_directories.remove(dir_path).is_some();
        if removed {
            let _ = self.save_config(&cfg).await;
        }
        removed
    }

    /// List every configured local directory, merging disk state (which
    /// files actually exist) with the config's enabled flags.
    pub async fn list_local_directories(&self) -> Vec<LocalDirectoryEntry> {
        let cfg = self.load_config().await;
        let mut out = Vec::with_capacity(cfg.local_directories.len());
        for (path, state) in cfg.local_directories {
            let resolved = local_context::resolve(&path);
            let dir_exists = match &resolved {
                Some(p) => tokio::fs::metadata(p)
                    .await
                    .map(|m| m.is_dir())
                    .unwrap_or(false),
                None => false,
            };
            let disk_names: std::collections::BTreeSet<String> = if dir_exists {
                local_context::scan_directory(resolved.as_ref().unwrap())
                    .await
                    .into_iter()
                    .map(|f| f.name)
                    .collect()
            } else {
                std::collections::BTreeSet::new()
            };

            let mut files = Vec::new();
            for (name, file_state) in state.files {
                files.push(LocalFileEntry {
                    exists_on_disk: disk_names.contains(&name),
                    name,
                    enabled: file_state.enabled,
                });
            }
            files.sort_by(|a, b| a.name.cmp(&b.name));
            out.push(LocalDirectoryEntry {
                path,
                exists: dir_exists,
                files,
            });
        }
        out.sort_by(|a, b| a.path.cmp(&b.path));
        out
    }

    pub async fn toggle_local_file(
        &self,
        dir_path: &str,
        filename: &str,
        enabled: Option<bool>,
    ) -> Option<bool> {
        let safe = sanitize_filename(filename)?;
        let mut cfg = self.load_config().await;
        let dir_entry = cfg.local_directories.get_mut(dir_path)?;
        let file_state = dir_entry.files.get_mut(&safe)?;
        file_state.enabled = enabled.unwrap_or(!file_state.enabled);
        let new_state = file_state.enabled;
        self.save_config(&cfg).await.ok()?;
        Some(new_state)
    }

    /// Re-scan a configured directory: add newly-discovered files as enabled
    /// (preserves existing enabled flags). Files that disappeared stay in
    /// the config with `exists_on_disk=false` when listed.
    pub async fn refresh_local_directory(&self, dir_path: &str) -> bool {
        let mut cfg = self.load_config().await;
        let Some(dir_entry) = cfg.local_directories.get_mut(dir_path) else {
            return false;
        };
        let resolved = match local_context::resolve(dir_path) {
            Some(p) => p,
            None => return false,
        };
        let disk = local_context::scan_directory(&resolved).await;
        for file in disk {
            dir_entry
                .files
                .entry(file.name)
                .or_insert(FileState { enabled: true });
        }
        self.save_config(&cfg).await.is_ok()
    }

    pub async fn get_local_file_content(
        &self,
        dir_path: &str,
        filename: &str,
    ) -> Option<String> {
        let safe = sanitize_filename(filename)?;
        let resolved = local_context::resolve(dir_path)?;
        local_context::read_file(&resolved.join(&safe)).await
    }

    // ---------------------------------------------------------------------
    // Concatenated output for the system prompt
    // ---------------------------------------------------------------------

    /// Emit all enabled uploaded files + all enabled files from configured
    /// local directories, formatted with the scope header and per-file
    /// sub-headers. Empty string when nothing is enabled.
    pub async fn load_enabled_context(&self) -> String {
        let cfg = self.load_config().await;
        let mut parts: Vec<String> = Vec::new();

        let enabled_files: Vec<(&String, &FileState)> = cfg
            .files
            .iter()
            .filter(|(_, s)| s.enabled)
            .collect();

        if !enabled_files.is_empty() {
            parts.push(format!("--- {} CONTEXT FILES ---", self.scope_label));
            parts.push(format!("{}\n", self.header_description));
            for (name, _) in enabled_files {
                let path = self.base_dir.join(name);
                let Ok(body) = tokio::fs::read_to_string(&path).await else {
                    continue;
                };
                parts.push(format!("--- {name} ---"));
                parts.push(body);
                parts.push(String::new());
            }
        }

        // Append local-directory contents (live-read each enabled file).
        let local_block = render_local_context(&cfg).await;
        if !local_block.is_empty() {
            parts.push(local_block);
        }

        parts.join("\n")
    }
}

// =============================================================================
// Helpers
// =============================================================================

fn sanitize_filename(raw: &str) -> Option<String> {
    let base = Path::new(raw)
        .file_name()
        .and_then(|s| s.to_str())?
        .to_string();
    if base.is_empty() || base == "." || base == ".." {
        return None;
    }
    Some(base)
}

async fn write_file_durable(path: &Path, bytes: &[u8]) -> std::io::Result<()> {
    if let Some(parent) = path.parent() {
        tokio::fs::create_dir_all(parent).await?;
    }
    let mut f = tokio::fs::OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(true)
        .open(path)
        .await?;
    f.write_all(bytes).await?;
    f.sync_all().await?;
    Ok(())
}

/// Format the local-directory section of a scope config. Empty string if no
/// enabled local files actually exist on disk.
async fn render_local_context(cfg: &ScopeConfig) -> String {
    if cfg.local_directories.is_empty() {
        return String::new();
    }
    let mut parts: Vec<String> = vec![
        "--- LOCAL CONTEXT FILES ---".to_string(),
        "The following files are referenced from local directories.\n".to_string(),
    ];
    let mut found_any = false;
    for (dir_path, state) in &cfg.local_directories {
        let Some(resolved) = local_context::resolve(dir_path) else {
            continue;
        };
        if !tokio::fs::metadata(&resolved)
            .await
            .map(|m| m.is_dir())
            .unwrap_or(false)
        {
            continue;
        }
        for (name, file_state) in &state.files {
            if !file_state.enabled {
                continue;
            }
            let Some(safe) = sanitize_filename(name) else {
                continue;
            };
            let full = resolved.join(&safe);
            let Some(body) = local_context::read_file(&full).await else {
                continue;
            };
            parts.push(format!("--- {safe} (from {dir_path}) ---"));
            parts.push(body);
            parts.push(String::new());
            found_any = true;
        }
    }
    if !found_any {
        String::new()
    } else {
        parts.join("\n")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sanitize_blocks_traversal() {
        assert_eq!(sanitize_filename("notes.md").as_deref(), Some("notes.md"));
        assert_eq!(
            sanitize_filename("../../etc/passwd").as_deref(),
            Some("passwd")
        );
        assert_eq!(sanitize_filename("").as_deref(), None);
        assert_eq!(sanitize_filename(".").as_deref(), None);
        assert_eq!(sanitize_filename("..").as_deref(), None);
        assert_eq!(
            sanitize_filename("dir/nested.md").as_deref(),
            Some("nested.md")
        );
    }
}
