pub mod jwt;
pub mod password;
pub mod refresh;
pub mod throttle;

use axum::extract::FromRequestParts;
use axum::http::header::AUTHORIZATION;
use axum::http::request::Parts;
use uuid::Uuid;

use crate::error::AppError;
use crate::models::Role;
use crate::state::AppState;

#[derive(Debug, Clone, Copy)]
pub struct AuthUser {
    pub user_id: Uuid,
    pub org_id: Uuid,
    pub role: Role,
}

impl FromRequestParts<AppState> for AuthUser {
    type Rejection = AppError;

    async fn from_request_parts(parts: &mut Parts, state: &AppState) -> Result<Self, AppError> {
        let token = parts
            .headers
            .get(AUTHORIZATION)
            .and_then(|value| value.to_str().ok())
            .and_then(|value| value.strip_prefix("Bearer "))
            .ok_or(AppError::Unauthorized)?;
        let claims = state.jwt.verify(token)?;

        let row: Option<(Role, bool)> = sqlx::query_as(
            "SELECT role, is_active FROM users WHERE id = $1 AND organization_id = $2",
        )
        .bind(claims.sub)
        .bind(claims.org)
        .fetch_optional(&state.pool)
        .await?;

        match row {
            Some((role, true)) => Ok(Self {
                user_id: claims.sub,
                org_id: claims.org,
                role,
            }),
            _ => Err(AppError::Unauthorized),
        }
    }
}
