//! Stateless filesystem primitives used by `context_files::ContextScope` to
//! manage local-directory references.
//!
//! This module holds **no persistent state** — the enabled/disabled flags for
//! local-directory files live in each scope's `config.json`, owned by
//! `context_files.rs`. Everything here is a pure FS helper: path validation,
//! directory scanning, file reading, directory browsing for UI.

use std::path::{Path, PathBuf};

use serde::Serialize;

/// Cap per Python (`MAX_FILES_PER_DIRECTORY`). Silently stops after N.
const MAX_FILES_PER_DIRECTORY: usize = 200;
const ALLOWED_EXTENSIONS: &[&str] = &["md", "txt"];

// =============================================================================
// Types
// =============================================================================

#[derive(Clone, Debug, Serialize)]
pub struct LocalFile {
    pub name: String,
    pub exists: bool,
}

#[derive(Clone, Debug, Serialize)]
pub struct BrowseEntry {
    pub name: String,
    pub path: String,
    pub is_dir: bool,
}

#[derive(Clone, Debug, Serialize)]
pub struct BrowseResult {
    pub path: String,
    pub parent: Option<String>,
    pub entries: Vec<BrowseEntry>,
}

// =============================================================================
// Resolution + validation
// =============================================================================

/// Best-effort canonicalize. Returns `None` if the path doesn't resolve —
/// callers treat that as "directory no longer exists" and skip it.
pub fn resolve(dir_path: &str) -> Option<PathBuf> {
    std::fs::canonicalize(dir_path).ok()
}

/// Validate a user-supplied directory path for use as a local context source.
/// Returns the resolved absolute path, or a human-readable error for the UI.
///
/// Rules (mirroring Python):
/// - Must exist and be a directory.
/// - Must be readable (we check by attempting to read the dir).
/// - Must NOT be inside the app's `data/` directory (prevents the user from
///   pointing at their own session files, which would create feedback loops).
pub fn validate_directory_path(dir_path: &str) -> Result<PathBuf, String> {
    if dir_path.trim().is_empty() {
        return Err("Directory path is empty.".to_string());
    }
    let resolved = std::fs::canonicalize(dir_path).map_err(|e| format!("Cannot resolve path: {e}"))?;
    let meta = std::fs::metadata(&resolved).map_err(|e| format!("Cannot stat path: {e}"))?;
    if !meta.is_dir() {
        return Err("Path is not a directory.".to_string());
    }
    // Quick readability check.
    if std::fs::read_dir(&resolved).is_err() {
        return Err("Directory is not readable.".to_string());
    }
    // Block paths inside the app's data/ tree.
    if let Ok(manifest) = std::env::var("CARGO_MANIFEST_DIR") {
        let data_root = Path::new(&manifest).join("../../data");
        if let Ok(data_abs) = std::fs::canonicalize(&data_root)
            && resolved.starts_with(&data_abs)
        {
            return Err(format!(
                "Path is inside the app's data directory ({}). Pick a directory outside.",
                data_abs.display()
            ));
        }
    }
    Ok(resolved)
}

// =============================================================================
// Directory scan
// =============================================================================

/// Non-recursive scan of `dir` for `.md` / `.txt` files, sorted alphabetically,
/// capped at 200 entries. Returns each file with `exists=true` (since we just
/// saw it on disk).
pub async fn scan_directory(dir: &Path) -> Vec<LocalFile> {
    let mut entries = match tokio::fs::read_dir(dir).await {
        Ok(e) => e,
        Err(_) => return Vec::new(),
    };
    let mut names: Vec<String> = Vec::new();
    while let Ok(Some(entry)) = entries.next_entry().await {
        if names.len() >= MAX_FILES_PER_DIRECTORY {
            break;
        }
        let ft = match entry.file_type().await {
            Ok(t) => t,
            Err(_) => continue,
        };
        if !ft.is_file() {
            continue;
        }
        let name = entry.file_name().to_string_lossy().to_string();
        if has_allowed_extension(&name) {
            names.push(name);
        }
    }
    names.sort();
    names
        .into_iter()
        .map(|name| LocalFile { name, exists: true })
        .collect()
}

fn has_allowed_extension(name: &str) -> bool {
    let Some(ext) = Path::new(name).extension().and_then(|s| s.to_str()) else {
        return false;
    };
    ALLOWED_EXTENSIONS.contains(&ext.to_ascii_lowercase().as_str())
}

// =============================================================================
// File read
// =============================================================================

/// Failure modes for reading a local-context file. Split apart so the UI can
/// say "invalid UTF-8" specifically (invisible replacement chars in a prompt
/// degrade LLM output; silent acceptance is worse than surfacing the mismatch).
#[derive(Debug, thiserror::Error)]
pub enum ReadError {
    #[error("{0}")]
    Io(#[from] std::io::Error),
    #[error("file is not valid UTF-8")]
    InvalidUtf8,
}

/// Read a single local-context file. Logs a warning on every failure path so
/// scheduler + prompt-assembly callers don't have to.
pub async fn read_file(path: &Path) -> Result<String, ReadError> {
    let bytes = match tokio::fs::read(path).await {
        Ok(b) => b,
        Err(err) => {
            tracing::warn!(?path, error = %err, "local context file read failed");
            return Err(ReadError::Io(err));
        }
    };
    String::from_utf8(bytes).map_err(|_| {
        tracing::warn!(?path, "local context file is not valid UTF-8; skipping");
        ReadError::InvalidUtf8
    })
}

// =============================================================================
// Directory browser (for the UI's "add local directory" modal)
// =============================================================================

/// List the immediate children of a directory: directories first (for
/// navigation), then `.md`/`.txt` files. Used by the directory-picker modal.
/// `show_hidden=false` skips entries starting with `.`.
pub async fn browse_directory(path: &Path, show_hidden: bool) -> Option<BrowseResult> {
    let resolved = std::fs::canonicalize(path).ok()?;
    if !tokio::fs::metadata(&resolved)
        .await
        .map(|m| m.is_dir())
        .unwrap_or(false)
    {
        return None;
    }

    let mut dir_entries: Vec<BrowseEntry> = Vec::new();
    let mut file_entries: Vec<BrowseEntry> = Vec::new();

    let Ok(mut entries) = tokio::fs::read_dir(&resolved).await else {
        return None;
    };
    while let Ok(Some(entry)) = entries.next_entry().await {
        let name = entry.file_name().to_string_lossy().to_string();
        if !show_hidden && name.starts_with('.') {
            continue;
        }
        let ft = match entry.file_type().await {
            Ok(t) => t,
            Err(_) => continue,
        };
        let path_str = entry.path().to_string_lossy().to_string();
        if ft.is_dir() {
            dir_entries.push(BrowseEntry {
                name,
                path: path_str,
                is_dir: true,
            });
        } else if ft.is_file() && has_allowed_extension(&name) {
            file_entries.push(BrowseEntry {
                name,
                path: path_str,
                is_dir: false,
            });
        }
    }
    dir_entries.sort_by(|a, b| a.name.cmp(&b.name));
    file_entries.sort_by(|a, b| a.name.cmp(&b.name));
    dir_entries.append(&mut file_entries);

    Some(BrowseResult {
        parent: resolved
            .parent()
            .map(|p| p.to_string_lossy().to_string()),
        path: resolved.to_string_lossy().to_string(),
        entries: dir_entries,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;
    use tokio::io::AsyncWriteExt;

    #[test]
    fn allowed_extensions() {
        assert!(has_allowed_extension("notes.md"));
        assert!(has_allowed_extension("README.TXT"));
        assert!(!has_allowed_extension("script.py"));
        assert!(!has_allowed_extension("no_extension"));
    }

    #[tokio::test]
    async fn read_file_surfaces_invalid_utf8() {
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join("bad.md");
        let mut f = tokio::fs::File::create(&path).await.unwrap();
        // Latin-1 "caf\xe9" — valid in ISO-8859-1, invalid as UTF-8.
        f.write_all(&[b'c', b'a', b'f', 0xE9]).await.unwrap();
        f.sync_all().await.unwrap();

        match read_file(&path).await {
            Err(ReadError::InvalidUtf8) => {}
            other => panic!("expected InvalidUtf8, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn read_file_round_trips_valid_utf8() {
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join("good.md");
        tokio::fs::write(&path, "café 😀").await.unwrap();
        assert_eq!(read_file(&path).await.unwrap(), "café 😀");
    }
}
