use axum::{extract::State, response::Html, routing::get, Router};

use crate::AppState;

pub fn build_router(state: AppState) -> Router {
    Router::new()
        .route("/health", get(health))
        .route("/hello", get(hello))
        .with_state(state)
}

async fn health() -> &'static str {
    "ok"
}

async fn hello(State(state): State<AppState>) -> Html<String> {
    let ctx = tera::Context::new();
    match state.tera.render("hello.html", &ctx) {
        Ok(html) => Html(html),
        Err(err) => {
            tracing::error!("tera render failed: {err}");
            Html(format!("<pre>tera error: {err}</pre>"))
        }
    }
}
