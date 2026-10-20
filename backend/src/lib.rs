pub mod api;
pub mod audit;
pub mod auth;
pub mod authz;
pub mod config;
pub mod error;
pub mod extract;
pub mod models;
pub mod openapi;
pub mod pagination;
pub mod state;
pub mod validation;

use axum::routing::get;
use axum::{Json, Router};
use tower_http::trace::TraceLayer;
use utoipa::OpenApi;
use utoipa_scalar::{Scalar, Servable};

use crate::openapi::ApiDoc;
use crate::state::AppState;

pub static MIGRATOR: sqlx::migrate::Migrator = sqlx::migrate!("./migrations");

pub fn app(state: AppState) -> Router {
    Router::new()
        .nest("/api/v1", api::router())
        .merge(api::health_router())
        .route("/openapi.json", get(|| async { Json(ApiDoc::openapi()) }))
        .merge(Scalar::with_url("/docs", ApiDoc::openapi()))
        .layer(TraceLayer::new_for_http())
        .with_state(state)
}
