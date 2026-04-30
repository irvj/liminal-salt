//! Theme listing — enumerates `themes/*.json` files in the embedded `Static`
//! asset, picks out the `id` + `name` fields, returns them sorted for the UI
//! picker. The theme JSON itself is consumed client-side by the theme-picker
//! JS via `/static/themes/{id}.json`; this module is just the
//! enumeration + metadata-extraction step.

use serde::{Deserialize, Serialize};

use crate::assets::Static;

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

/// List available themes. Iterates each `.json` file under `themes/` in the
/// embedded `Static` bundle, pulls `id` and `name`. Malformed files are
/// skipped silently. Sorted by id for stable ordering.
pub fn list() -> Vec<Theme> {
    let mut themes: Vec<Theme> = Static::iter()
        .filter_map(|path| {
            let stem = path
                .strip_prefix("themes/")
                .and_then(|s| s.strip_suffix(".json"))?;
            let file = Static::get(&path)?;
            parse_theme(stem, &file.data)
        })
        .collect();
    themes.sort_by(|a, b| a.id.cmp(&b.id));
    themes
}

/// Pure parser: theme stem (e.g. `"ayu"`) + raw JSON bytes → `Theme`. Falls
/// back to stem-derived id and title-cased stem for missing fields.
fn parse_theme(stem: &str, bytes: &[u8]) -> Option<Theme> {
    let parsed: ThemeFile = serde_json::from_slice(bytes).ok()?;
    let id = parsed.id.unwrap_or_else(|| stem.to_string());
    let name = parsed.name.unwrap_or_else(|| {
        let mut chars = stem.chars();
        match chars.next() {
            Some(c) => c.to_uppercase().collect::<String>() + chars.as_str(),
            None => String::new(),
        }
    });
    Some(Theme { id, name })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_full_record() {
        let body = br#"{"id":"alpha","name":"Alpha"}"#;
        let t = parse_theme("alpha", body).unwrap();
        assert_eq!(t.id, "alpha");
        assert_eq!(t.name, "Alpha");
    }

    #[test]
    fn parse_missing_id_falls_back_to_stem() {
        let body = br#"{"name":"Whatever"}"#;
        let t = parse_theme("fallback", body).unwrap();
        assert_eq!(t.id, "fallback");
        assert_eq!(t.name, "Whatever");
    }

    #[test]
    fn parse_missing_name_title_cases_stem() {
        let body = br#"{"id":"gruvbox"}"#;
        let t = parse_theme("gruvbox", body).unwrap();
        assert_eq!(t.name, "Gruvbox");
    }

    #[test]
    fn parse_malformed_json_returns_none() {
        assert!(parse_theme("broken", b"not json").is_none());
    }

    #[test]
    fn list_returns_bundled_themes_sorted() {
        let themes = list();
        assert!(!themes.is_empty(), "bundled themes missing");
        let mut ids: Vec<&str> = themes.iter().map(|t| t.id.as_str()).collect();
        let original = ids.clone();
        ids.sort();
        assert_eq!(ids, original, "themes must be sorted by id");
    }
}
