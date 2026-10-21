pub mod analytics;
pub mod audit_log;
pub mod auth;
pub mod automations;
pub mod companies;
pub mod contacts;
pub mod deals;
pub mod duplicates;
pub mod notes;
pub mod organization;
pub mod search;
pub mod simulation;
pub mod stages;
pub mod targets;
pub mod tasks;
pub mod transfer;
pub mod users;
pub mod views;

use axum::extract::State;
use axum::http::StatusCode;
use axum::routing::get;
use axum::{Json, Router};
use serde_json::{json, Value};

use crate::state::AppState;

pub fn router() -> Router<AppState> {
    Router::new()
        .merge(auth::router())
        .merge(automations::router())
        .merge(users::router())
        .merge(stages::router())
        .merge(companies::router())
        .merge(contacts::router())
        .merge(deals::router())
        .merge(duplicates::router())
        .merge(tasks::router())
        .merge(notes::router())
        .merge(audit_log::router())
        .merge(analytics::router())
        .merge(organization::router())
        .merge(search::router())
        .merge(simulation::router())
        .merge(targets::router())
        .merge(transfer::router())
        .merge(views::router())
        .merge(crate::live::router())
}

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
