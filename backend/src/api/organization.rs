use axum::extract::State;
use axum::http::StatusCode;
use axum::routing::put;
use axum::Router;
use serde::Deserialize;

use crate::auth::AuthUser;
use crate::error::{AppError, AppResult};
use crate::extract::ApiJson;
use crate::state::AppState;

pub fn router() -> Router<AppState> {
    Router::new().route("/organization", put(update_organization))
}

#[derive(Deserialize)]
pub struct OrganizationSettings {
    pub stale_after_days: i16,
}

pub async fn update_organization(
    State(state): State<AppState>,
    actor: AuthUser,
    ApiJson(req): ApiJson<OrganizationSettings>,
) -> AppResult<StatusCode> {
    actor.require_admin()?;
    if !(1..=365).contains(&req.stale_after_days) {
        return Err(AppError::validation(
            "stale_after_days",
            "must be between 1 and 365 days",
        ));
    }
    sqlx::query("UPDATE organizations SET stale_after_days = $2 WHERE id = $1")
        .bind(actor.org_id)
        .bind(req.stale_after_days)
        .execute(&state.pool)
        .await?;
    Ok(StatusCode::NO_CONTENT)
}
