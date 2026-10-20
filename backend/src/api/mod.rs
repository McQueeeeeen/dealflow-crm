pub mod analytics;
pub mod audit_log;
pub mod auth;
pub mod companies;
pub mod contacts;
pub mod deals;
pub mod notes;
pub mod stages;
pub mod tasks;
pub mod users;

use axum::extract::State;
use axum::http::StatusCode;
use axum::routing::get;
use axum::{Json, Router};
use serde_json::{json, Value};

use crate::state::AppState;

pub fn router() -> Router<AppState> {
    Router::new()
        .merge(auth::router())
        .merge(users::router())
        .merge(stages::router())
        .merge(companies::router())
        .merge(contacts::router())
        .merge(deals::router())
        .merge(tasks::router())
        .merge(notes::router())
        .merge(audit_log::router())
        .merge(analytics::router())
}

#[utoipa::path(
    get, path = "/health", tag = "system",
    responses(
        (status = 200, description = "API and database are reachable"),
        (status = 503, description = "Database is unreachable"),
    )
)]
pub async fn health(State(state): State<AppState>) -> (StatusCode, Json<Value>) {
    match sqlx::query("SELECT 1").execute(&state.pool).await {
        Ok(_) => (StatusCode::OK, Json(json!({ "status": "ok" }))),
        Err(err) => {
            tracing::error!(error = %err, "health check failed");
            (
                StatusCode::SERVICE_UNAVAILABLE,
                Json(json!({ "status": "database_unavailable" })),
            )
        }
    }
}

pub fn health_router() -> Router<AppState> {
    Router::new().route("/health", get(health))
}
