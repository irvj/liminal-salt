pub mod assets;
pub mod handlers;
pub mod middleware;
pub mod routes;
pub mod services;
pub mod tera_extra;

use std::{net::SocketAddr, path::PathBuf, sync::Arc, time::Duration};

use axum::middleware as axum_mw;
use tera::Tera;
use tower_http::trace::TraceLayer;
use tower_sessions::{
    Expiry, MemoryStore, SessionManagerLayer, cookie::time::Duration as CookieDuration,
};

use crate::{
    middleware::{app_ready, csrf},
    services::{config, memory_worker::MemoryWorker, prompt, prompts},
};

/// Shared state every Axum handler needs access to. Constructed once in
/// `run_server`; `reqwest::Client` internally uses `Arc`, so cheap clones keep
/// one HTTP connection pool alive for the whole process.
///
/// Bundled assets (templates, static, default personas, default prompts) are
/// not held here — they live in the `assets` module as compile-time embedded
/// resources accessed via `crate::assets::*`.
#[derive(Clone)]
pub struct AppState {
    pub tera: Arc<Tera>,
    pub data_dir: PathBuf,
    pub sessions_dir: PathBuf,
    pub http: reqwest::Client,
    /// Shared memory worker — owns per-persona + per-session "already running"
    /// mutexes and the status map for `/memory/status/` polling. Cheap to
    /// clone (`Arc<Inner>` under the hood).
    pub memory: MemoryWorker,
}

/// Boot the Axum server: build state, seed bundled defaults, start the memory
/// schedulers, bind a TCP listener at `addr`, and serve until `ctrl_c`.
///
/// This is the single entry point for both the CLI binary and a future Tauri
/// shell — both can drive the same Axum app from the same library function.
/// Tracing-subscriber setup is the caller's responsibility (CLI wires it in
/// `main`; a Tauri shell will use its own subscriber).
pub async fn run_server(addr: SocketAddr) -> anyhow::Result<()> {
    let tera = assets::build_tera()?;

    let data_dir = config::data_dir();
    tokio::fs::create_dir_all(&data_dir).await?;
    let sessions_dir = config::sessions_dir(&data_dir);
    tokio::fs::create_dir_all(&sessions_dir).await?;

    // Bundled defaults ship embedded in the binary; seeders materialize them
    // into `<data_dir>/{personas,prompts}/` on first boot. Existing user
    // files are never overwritten.
    prompt::seed_default_personas(&data_dir).await;
    prompts::seed_default_prompts(&data_dir).await;

    let state = AppState {
        tera: Arc::new(tera),
        data_dir,
        sessions_dir,
        http: reqwest::Client::builder()
            .timeout(Duration::from_secs(30))
            .build()?,
        memory: MemoryWorker::new(),
    };

    // Kick off the two memory schedulers. They're stopped via ctrl_c below
    // so a scheduler mid-LLM-call gets to finish before the process exits.
    let scheduler_handles = state.memory.start_schedulers(state.clone());

    // Session state (current session id, user timezone, CSRF token) lives in a
    // process-local memory store. Two-week cookie expiry on inactivity.
    let session_store = MemoryStore::default();
    let session_layer = SessionManagerLayer::new(session_store)
        .with_name("liminal_salt_session")
        // `Secure = true` (the default) would make browsers reject the cookie
        // on plain http://localhost, silently breaking every POST because a
        // fresh session (with a new CSRF token) gets created per request.
        .with_secure(false)
        .with_expiry(Expiry::OnInactivity(CookieDuration::weeks(2)));

    // Layer order (outer → inner as written; inner runs first at request time):
    //   TraceLayer  (outermost, sees every request)
    //   session_layer  (must run before any middleware that reads the session)
    //   csrf_layer  (needs session)
    //   app_ready  (needs session for the redirect; runs after csrf so we
    //               don't burn CSRF on a request we're about to redirect)
    let app = routes::build_router(state.clone())
        .layer(axum_mw::from_fn_with_state(
            state.clone(),
            app_ready::require_app_ready,
        ))
        .layer(axum_mw::from_fn(csrf::require_csrf))
        .layer(session_layer)
        .layer(TraceLayer::new_for_http());

    let listener = tokio::net::TcpListener::bind(addr).await?;
    let bound = listener.local_addr()?;

    println!();
    println!("Liminal Salt v{}", env!("CARGO_PKG_VERSION"));
    println!("Listening on http://{bound}");
    println!("Press Ctrl-C to stop.");
    println!();

    axum::serve(listener, app)
        .with_graceful_shutdown(async {
            let _ = tokio::signal::ctrl_c().await;
            tracing::info!("ctrl_c received, shutting down");
        })
        .await?;

    // Stop the schedulers AFTER the server drains so any in-flight request
    // that would dispatch to the worker still finds the worker alive.
    MemoryWorker::stop_schedulers(scheduler_handles).await;
    tracing::info!("schedulers stopped");
    Ok(())
}
