use std::{net::SocketAddr, path::PathBuf, sync::Arc};

use tera::Tera;
use tower_http::{services::ServeDir, trace::TraceLayer};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt, EnvFilter};

mod routes;
mod services;

#[derive(Clone)]
pub struct AppState {
    pub tera: Arc<Tera>,
}

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
    let tera = Tera::new(
        manifest_dir
            .join("templates")
            .join("**")
            .join("*.html")
            .to_str()
            .expect("template glob is utf-8"),
    )?;
    let state = AppState {
        tera: Arc::new(tera),
    };

    // Static assets still live at the repo-root chat/static/ path (unchanged from Django).
    // crates/liminal-salt/ → ../../chat/static
    let static_dir = manifest_dir.join("../../chat/static");

    let app = routes::build_router(state)
        .nest_service("/static", ServeDir::new(&static_dir))
        .layer(TraceLayer::new_for_http());

    let addr = SocketAddr::from(([127, 0, 0, 1], 8420));
    tracing::info!("liminal-salt listening on http://{addr}");
    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app).await?;
    Ok(())
}
