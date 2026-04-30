//! Bundled assets compiled into the binary.
//!
//! `rust-embed` with the `debug-embed` feature reads from disk in debug builds
//! (preserving template + static hot-reload) and embeds in release builds.
//! Same code path in both modes — the dev workflow (`cargo run` + browser
//! refresh) is unchanged, while release and Tauri builds become self-contained
//! single binaries.
//!
//! Each `RustEmbed` struct below is the canonical reference for one category
//! of bundled content. To add new bundled content: extend an existing struct's
//! folder, or add a new struct here, and document who reads it.
//!
//! Consumers call `Foo::get(path)` / `Foo::iter()` directly; there is no
//! runtime indirection. The two helpers in this module turn a bundle into a
//! ready-to-use runtime object (`Tera`, axum response).
//!
//! `AGREEMENT.md` lives at the repo root for GitHub discoverability and is
//! embedded via `include_str!` in `services::config` rather than through a
//! `RustEmbed` struct here — one file, no enumeration needed.

use std::borrow::Cow;

use axum::{
    body::Bytes,
    extract::Path,
    http::{StatusCode, header},
    response::{IntoResponse, Response},
};
use rust_embed::RustEmbed;
use tera::Tera;

/// Tera HTML templates. Loaded into a Tera registry at boot via [`build_tera`].
#[derive(RustEmbed)]
#[folder = "$CARGO_MANIFEST_DIR/templates/"]
#[include = "**/*.html"]
pub struct Templates;

/// Frontend static assets served at `/static/*` by [`serve_static`].
/// JS (`utils.js`, `components.js`), vendor JS (htmx, alpine), CSS, theme
/// JSONs, favicon.
#[derive(RustEmbed)]
#[folder = "$CARGO_MANIFEST_DIR/static/"]
pub struct Static;

/// Bundled default personas. Copied into `<data_dir>/personas/` on first boot
/// by `services::prompt::seed_default_personas`. Each top-level entry is a
/// persona folder containing `identity.md` and `config.json`.
#[derive(RustEmbed)]
#[folder = "$CARGO_MANIFEST_DIR/default_personas/"]
pub struct DefaultPersonas;

/// Bundled default LLM instruction prompts. Owned by `services::prompts`:
/// seeded into `<data_dir>/prompts/` on first boot, used as the fallback for
/// `prompts::load`, and as the source for "Reset to default."
#[derive(RustEmbed)]
#[folder = "$CARGO_MANIFEST_DIR/default_prompts/"]
pub struct DefaultPrompts;

/// Build the Tera registry from embedded templates and register
/// project-specific filters. Equivalent to the pre-refactor
/// `Tera::new(glob)` + `tera_extra::register` pair.
pub fn build_tera() -> anyhow::Result<Tera> {
    let mut tera = Tera::default();
    let mut entries: Vec<(String, String)> = Vec::new();
    for name in Templates::iter() {
        let file = Templates::get(&name).ok_or_else(|| {
            anyhow::anyhow!("template {name} disappeared between iter and get")
        })?;
        let body = std::str::from_utf8(&file.data)
            .map_err(|err| anyhow::anyhow!("template {name} not utf-8: {err}"))?
            .to_string();
        entries.push((name.into_owned(), body));
    }
    tera.add_raw_templates(entries.iter().map(|(n, b)| (n.as_str(), b.as_str())))?;
    crate::tera_extra::register(&mut tera);
    Ok(tera)
}

/// Axum handler serving embedded `Static` content at `/static/{*path}`.
/// `Content-Type` is derived from the path extension via `mime_guess`;
/// missing files return 404. Body is zero-copy in release/embedded mode
/// (`Cow::Borrowed(&'static [u8])` → `Bytes::from_static`); dev mode reads
/// from disk and produces an owned `Vec<u8>`.
pub async fn serve_static(Path(path): Path<String>) -> Response {
    let Some(file) = Static::get(&path) else {
        return StatusCode::NOT_FOUND.into_response();
    };
    let mime = mime_guess::from_path(&path).first_or_octet_stream();
    let body = match file.data {
        Cow::Borrowed(s) => Bytes::from_static(s),
        Cow::Owned(v) => Bytes::from(v),
    };
    ([(header::CONTENT_TYPE, mime.as_ref().to_string())], body).into_response()
}
