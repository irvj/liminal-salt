pub mod routes;
pub mod services;

use std::sync::Arc;

use tera::Tera;

#[derive(Clone)]
pub struct AppState {
    pub tera: Arc<Tera>,
}
