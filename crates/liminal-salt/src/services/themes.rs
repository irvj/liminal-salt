//! Theme listing — scans `chat/static/themes/*.json`, picks out the `id` + `name`
//! fields, returns them sorted for the UI picker. The theme JSON itself is
//! consumed client-side by the theme-picker JS; this module is just the
//! directory-listing + metadata-extraction step.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, PartialEq)]
pub struct Theme {
    pub id: String,
    pub name: String,
}

#[derive(Deserialize)]
struct ThemeFile {
    #[serde(default)]
    id: Option<String>,
    #[serde(default)]
    name: Option<String>,
}

/// The canonical theme directory. Under the Rust layout that's
/// `<crate>/../../chat/static/themes/`; in M2 the Tauri build will embed
/// these so the resolver changes to an `app_data_dir()` / embedded path.
pub fn themes_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../chat/static/themes")
}

/// List available themes. Reads each `.json` file in `themes_dir()`, pulls
/// `id` and `name`. Malformed files are skipped silently (matches Python's
/// `utils.get_theme_list`). Sorted by id for stable ordering in the UI.
pub async fn list_themes() -> Vec<Theme> {
    list_themes_in(&themes_dir()).await
}

/// Same as `list_themes` but takes the directory as a parameter — used by
/// tests to drive against a tempdir and by callers that want to override the
/// resolver.
pub async fn list_themes_in(dir: &Path) -> Vec<Theme> {
    let mut entries = match tokio::fs::read_dir(dir).await {
        Ok(e) => e,
        Err(_) => return Vec::new(),
    };
    let mut themes = Vec::new();
    while let Ok(Some(entry)) = entries.next_entry().await {
        let filename = entry.file_name().to_string_lossy().to_string();
        let Some(stem) = filename.strip_suffix(".json") else { continue };
        let bytes = match tokio::fs::read(entry.path()).await {
            Ok(b) => b,
            Err(_) => continue,
        };
        let parsed: ThemeFile = match serde_json::from_slice(&bytes) {
            Ok(p) => p,
            Err(_) => continue,
        };
        let id = parsed.id.unwrap_or_else(|| stem.to_string());
        let name = parsed.name.unwrap_or_else(|| {
            // Fall back to title-cased stem — "ayu" → "Ayu".
            let mut chars = stem.chars();
            match chars.next() {
                Some(c) => c.to_uppercase().collect::<String>() + chars.as_str(),
                None => String::new(),
            }
        });
        themes.push(Theme { id, name });
    }
    themes.sort_by(|a, b| a.id.cmp(&b.id));
    themes
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::io::AsyncWriteExt;

    async fn write(path: &Path, body: &str) {
        if let Some(parent) = path.parent() {
            tokio::fs::create_dir_all(parent).await.unwrap();
        }
        let mut f = tokio::fs::File::create(path).await.unwrap();
        f.write_all(body.as_bytes()).await.unwrap();
    }

    #[tokio::test]
    async fn lists_valid_themes_sorted() {
        let tmp = tempfile::tempdir().unwrap();
        write(
            &tmp.path().join("zebra.json"),
            r#"{"id":"zebra","name":"Zebra"}"#,
        )
        .await;
        write(
            &tmp.path().join("alpha.json"),
            r#"{"id":"alpha","name":"Alpha"}"#,
        )
        .await;
        let themes = list_themes_in(tmp.path()).await;
        assert_eq!(themes.len(), 2);
        assert_eq!(themes[0].id, "alpha");
        assert_eq!(themes[1].id, "zebra");
    }

    #[tokio::test]
    async fn missing_id_falls_back_to_stem() {
        let tmp = tempfile::tempdir().unwrap();
        write(&tmp.path().join("fallback.json"), r#"{"name":"Whatever"}"#).await;
        let themes = list_themes_in(tmp.path()).await;
        assert_eq!(themes[0].id, "fallback");
        assert_eq!(themes[0].name, "Whatever");
    }

    #[tokio::test]
    async fn missing_name_title_cases_stem() {
        let tmp = tempfile::tempdir().unwrap();
        write(&tmp.path().join("gruvbox.json"), r#"{"id":"gruvbox"}"#).await;
        let themes = list_themes_in(tmp.path()).await;
        assert_eq!(themes[0].name, "Gruvbox");
    }

    #[tokio::test]
    async fn malformed_json_is_skipped() {
        let tmp = tempfile::tempdir().unwrap();
        write(&tmp.path().join("broken.json"), "not json").await;
        write(
            &tmp.path().join("good.json"),
            r#"{"id":"good","name":"Good"}"#,
        )
        .await;
        let themes = list_themes_in(tmp.path()).await;
        assert_eq!(themes.len(), 1);
        assert_eq!(themes[0].id, "good");
    }

    #[tokio::test]
    async fn non_json_files_are_ignored() {
        let tmp = tempfile::tempdir().unwrap();
        write(&tmp.path().join("README.md"), "ignore me").await;
        write(&tmp.path().join("x.json"), r#"{"id":"x","name":"X"}"#).await;
        let themes = list_themes_in(tmp.path()).await;
        assert_eq!(themes.len(), 1);
        assert_eq!(themes[0].id, "x");
    }

    #[tokio::test]
    async fn missing_directory_returns_empty() {
        let themes = list_themes_in(Path::new("/nonexistent/path")).await;
        assert!(themes.is_empty());
    }
}
