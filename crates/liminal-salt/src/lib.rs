pub mod assets;
pub mod handlers;
pub mod middleware;
pub mod routes;
pub mod services;
pub mod tera_extra;

use std::{path::PathBuf, sync::Arc};

use tera::Tera;

use crate::services::memory_worker::MemoryWorker;

/// Shared state every Axum handler needs access to. Constructed once in
/// `main`; `reqwest::Client` internally uses `Arc`, so cheap clones keep one
/// HTTP connection pool alive for the whole process.
#[derive(Clone)]
pub struct AppState {
    pub tera: Arc<Tera>,
    pub data_dir: PathBuf,
    pub sessions_dir: PathBuf,
    /// Bundled default prompts ship inside the crate at
    /// `<manifest>/default_prompts/`; resolved once at boot. M4 (Tauri) swaps
    /// this for embedded assets.
    pub bundled_prompts_dir: PathBuf,
    pub http: reqwest::Client,
    /// Shared memory worker — owns per-persona + per-session "already running"
    /// mutexes and the status map for `/memory/status/` polling. Cheap to
    /// clone (`Arc<Inner>` under the hood).
    pub memory: MemoryWorker,
}
