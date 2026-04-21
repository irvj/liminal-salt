pub mod middleware;
pub mod routes;
pub mod services;
pub mod tera_extra;

use std::{path::PathBuf, sync::Arc};

use tera::Tera;

/// Shared state every Axum handler needs access to. Constructed once in
/// `main`; `reqwest::Client` internally uses `Arc`, so cheap clones keep one
/// HTTP connection pool alive for the whole process.
#[derive(Clone)]
pub struct AppState {
    pub tera: Arc<Tera>,
    pub data_dir: PathBuf,
    pub sessions_dir: PathBuf,
    pub http: reqwest::Client,
}
