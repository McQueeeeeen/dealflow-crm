pub mod api;
pub mod audit;
pub mod auth;
pub mod authz;
pub mod config;
pub mod dql;
pub mod error;
pub mod extract;
pub mod live;
pub mod models;
pub mod pagination;
pub mod simulation;
pub mod state;
pub mod validation;

use axum::Router;
use tower_http::trace::TraceLayer;

use crate::state::AppState;

pub static MIGRATOR: sqlx::migrate::Migrator = sqlx::migrate!("./migrations");

pub fn app(state: AppState) -> Router {
    Router::new()
        .nest("/api/v1", api::router())
        .merge(api::health_router())
        .layer(TraceLayer::new_for_http())
        .with_state(state)
}
