use axum::extract::State;
use axum::http::StatusCode;
use axum::routing::{get, put};
use axum::{Json, Router};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use uuid::Uuid;

use crate::audit;
use crate::auth::password::{self, MAX_PASSWORD_LEN, MIN_PASSWORD_LEN};
use crate::auth::AuthUser;
use crate::error::{AppError, AppResult};
use crate::extract::{ApiJson, ApiPath};
use crate::models::{EntityType, Role};
use crate::state::AppState;
use crate::validation::{normalize_email, Validator};

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/users", get(list_users).post(create_user))
        .route("/users/{id}", put(update_user))
}

#[derive(Debug, Serialize, FromRow)]
pub struct User {
    pub id: Uuid,
    pub email: String,
    pub full_name: String,
    pub role: Role,
    pub is_active: bool,
    pub created_at: DateTime<Utc>,
}

#[derive(Deserialize)]
pub struct CreateUser {
    pub email: String,
    pub full_name: String,
    pub role: Role,
    pub password: String,
}

#[derive(Deserialize)]
pub struct UpdateUser {
    pub full_name: String,
    pub role: Role,
    pub is_active: bool,
}

const USER_COLUMNS: &str = "id, email, full_name, role, is_active, created_at";

pub async fn list_users(
    State(state): State<AppState>,
    user: AuthUser,
) -> AppResult<Json<Vec<User>>> {
    let users = sqlx::query_as(&format!(
        "SELECT {USER_COLUMNS} FROM users WHERE organization_id = $1
         ORDER BY is_active DESC, full_name"
    ))
    .bind(user.org_id)
    .fetch_all(&state.pool)
    .await?;
    Ok(Json(users))
}

pub async fn create_user(
    State(state): State<AppState>,
    actor: AuthUser,
    ApiJson(req): ApiJson<CreateUser>,
) -> AppResult<(StatusCode, Json<User>)> {
    actor.require_admin()?;
    let email = normalize_email(&req.email);
    let full_name = req.full_name.trim();
    Validator::new()
        .email("email", &email)
        .required("full_name", full_name, 120)
        .check(
            (MIN_PASSWORD_LEN..=MAX_PASSWORD_LEN).contains(&req.password.chars().count()),
            "password",
            &format!("must be {MIN_PASSWORD_LEN} to {MAX_PASSWORD_LEN} characters"),
        )
        .finish()?;

    let hash = password::hash_password(&req.password)?;
    let mut tx = state.pool.begin().await?;
    let created: User = sqlx::query_as(&format!(
        "INSERT INTO users (id, organization_id, email, full_name, password_hash, role)
         VALUES ($1, $2, $3, $4, $5, $6)
         RETURNING {USER_COLUMNS}"
    ))
    .bind(Uuid::now_v7())
    .bind(actor.org_id)
    .bind(&email)
    .bind(full_name)
    .bind(&hash)
    .bind(req.role)
    .fetch_one(&mut *tx)
    .await?;
    audit::created(&mut tx, &actor, EntityType::User, created.id, &created).await?;
    tx.commit().await?;
    Ok((StatusCode::CREATED, Json(created)))
}

pub async fn update_user(
    State(state): State<AppState>,
    actor: AuthUser,
    ApiPath(id): ApiPath<Uuid>,
    ApiJson(req): ApiJson<UpdateUser>,
) -> AppResult<Json<User>> {
    actor.require_admin()?;
    let full_name = req.full_name.trim();
    Validator::new()
        .required("full_name", full_name, 120)
        .finish()?;
    if id == actor.user_id && (req.role != Role::Admin || !req.is_active) {
        return Err(AppError::Conflict(
            "you cannot remove your own admin access".into(),
        ));
    }

    let mut tx = state.pool.begin().await?;
    let before: User = sqlx::query_as(&format!(
        "SELECT {USER_COLUMNS} FROM users WHERE id = $1 AND organization_id = $2 FOR UPDATE"
    ))
    .bind(id)
    .bind(actor.org_id)
    .fetch_optional(&mut *tx)
    .await?
    .ok_or(AppError::NotFound("user"))?;

    let after: User = sqlx::query_as(&format!(
        "UPDATE users SET full_name = $3, role = $4, is_active = $5, updated_at = now()
         WHERE id = $1 AND organization_id = $2
         RETURNING {USER_COLUMNS}"
    ))
    .bind(id)
    .bind(actor.org_id)
    .bind(full_name)
    .bind(req.role)
    .bind(req.is_active)
    .fetch_one(&mut *tx)
    .await?;

    if !after.is_active {
        sqlx::query(
            "UPDATE refresh_tokens SET revoked_at = now() WHERE user_id = $1 AND revoked_at IS NULL",
        )
        .bind(id)
        .execute(&mut *tx)
        .await?;
    }

    audit::updated(&mut tx, &actor, EntityType::User, id, &before, &after).await?;
    tx.commit().await?;
    Ok(Json(after))
}
