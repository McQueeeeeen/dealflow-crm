use axum::extract::State;
use axum::http::StatusCode;
use axum::routing::{delete, get};
use axum::{Json, Router};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use uuid::Uuid;

use crate::auth::AuthUser;
use crate::dql;
use crate::error::{AppError, AppResult};
use crate::extract::{ApiJson, ApiPath};
use crate::state::AppState;
use crate::validation::Validator;

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/views", get(list_views).post(create_view))
        .route("/views/{id}", delete(delete_view))
}

#[derive(Debug, Serialize, FromRow)]
pub struct SavedView {
    pub id: Uuid,
    pub name: String,
    pub filter: String,
    pub shared: bool,
    pub user_id: Uuid,
    pub owner_name: String,
    pub created_at: DateTime<Utc>,
}

#[derive(Deserialize)]
pub struct CreateView {
    pub name: String,
    pub filter: String,
    #[serde(default)]
    pub shared: bool,
}

const SELECT_VIEW: &str = "
    SELECT v.id, v.name, v.filter, v.shared, v.user_id, u.full_name AS owner_name, v.created_at
    FROM saved_views v JOIN users u ON u.id = v.user_id";

pub async fn list_views(State(state): State<AppState>, user: AuthUser) -> AppResult<Json<Vec<SavedView>>> {
    let views = sqlx::query_as(&format!(
        "{SELECT_VIEW} WHERE v.organization_id = $1 AND (v.user_id = $2 OR v.shared)
         ORDER BY v.user_id <> $2, lower(v.name), v.id"
    ))
    .bind(user.org_id)
    .bind(user.user_id)
    .fetch_all(&state.pool)
    .await?;
    Ok(Json(views))
}

pub async fn create_view(
    State(state): State<AppState>,
    user: AuthUser,
    ApiJson(req): ApiJson<CreateView>,
) -> AppResult<(StatusCode, Json<SavedView>)> {
    let (name, filter) = (req.name.trim(), req.filter.trim());
    Validator::new()
        .required("name", name, 60)
        .required("filter", filter, 500)
        .finish()?;
    dql::parse(filter, Utc::now().date_naive())?;
    let id = Uuid::now_v7();
    sqlx::query(
        "INSERT INTO saved_views (id, organization_id, user_id, name, filter, shared)
         VALUES ($1, $2, $3, $4, $5, $6)",
    )
    .bind(id)
    .bind(user.org_id)
    .bind(user.user_id)
    .bind(name)
    .bind(filter)
    .bind(req.shared)
    .execute(&state.pool)
    .await?;
    let view = sqlx::query_as(&format!("{SELECT_VIEW} WHERE v.id = $1"))
        .bind(id)
        .fetch_one(&state.pool)
        .await?;
    Ok((StatusCode::CREATED, Json(view)))
}

pub async fn delete_view(
    State(state): State<AppState>,
    user: AuthUser,
    ApiPath(id): ApiPath<Uuid>,
) -> AppResult<StatusCode> {
    let owner: Uuid = sqlx::query_scalar(
        "SELECT user_id FROM saved_views
         WHERE id = $1 AND organization_id = $2 AND (user_id = $3 OR shared)",
    )
    .bind(id)
    .bind(user.org_id)
    .bind(user.user_id)
    .fetch_optional(&state.pool)
    .await?
    .ok_or(AppError::NotFound("view"))?;
    if owner != user.user_id {
        user.require_admin()?;
    }
    sqlx::query("DELETE FROM saved_views WHERE id = $1")
        .bind(id)
        .execute(&state.pool)
        .await?;
    Ok(StatusCode::NO_CONTENT)
}
