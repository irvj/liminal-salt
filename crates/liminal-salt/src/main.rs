use std::{net::SocketAddr, path::PathBuf, sync::Arc, time::Duration};

use axum::middleware as axum_mw;
use tera::Tera;
use tower_http::{services::ServeDir, trace::TraceLayer};
use tower_sessions::{Expiry, MemoryStore, SessionManagerLayer, cookie::time::Duration as CookieDuration};
use tracing_subscriber::{EnvFilter, layer::SubscriberExt, util::SubscriberInitExt};

use liminal_salt::{
    AppState,
    middleware::{app_ready, csrf},
    routes,
    services::{config, memory_worker::MemoryWorker, prompt},
    tera_extra,
};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::registry()
        .with(
            EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "liminal_salt=debug,tower_http=info".into()),
        )
        .with(tracing_subscriber::fmt::layer())
        .init();

    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));

    let mut tera = Tera::new(
        manifest_dir
            .join("templates")
            .join("**")
            .join("*.html")
            .to_str()
            .expect("template glob is utf-8"),
    )?;
    tera_extra::register(&mut tera);

    let data_dir = config::data_dir();
    tokio::fs::create_dir_all(&data_dir).await?;
    let sessions_dir = config::sessions_dir(&data_dir);
    tokio::fs::create_dir_all(&sessions_dir).await?;

    // Bundled default personas ship inside the crate; `seed_default_personas`
    // copies them into `<data_dir>/personas/` on first boot.
    let bundled_personas = manifest_dir.join("default_personas");
    prompt::seed_default_personas(&data_dir, &bundled_personas).await;

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

    // Static assets (JS, CSS, themes, favicon) ship inside the crate and are
    // served by tower-http's ServeDir at /static/. In M2 (Tauri) they'll get
    // embedded via rust-embed; the resolver in `data_dir()` is the only other
    // path literal that changes.
    let static_dir = manifest_dir.join("static");

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
        .nest_service("/static", ServeDir::new(&static_dir))
        .layer(axum_mw::from_fn_with_state(
            state.clone(),
            app_ready::require_app_ready,
        ))
        .layer(axum_mw::from_fn(csrf::require_csrf))
        .layer(session_layer)
        .layer(TraceLayer::new_for_http());

    let addr = SocketAddr::from(([127, 0, 0, 1], 8420));
    tracing::info!("liminal-salt listening on http://{addr}");
    let listener = tokio::net::TcpListener::bind(addr).await?;

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
