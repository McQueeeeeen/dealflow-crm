use axum::extract::State;
use axum::routing::get;
use axum::{Json, Router};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use uuid::Uuid;

use crate::auth::AuthUser;
use crate::error::{AppError, AppResult};
use crate::extract::ApiQuery;
use crate::models::{AuditAction, EntityType};
use crate::pagination::{Page, Pagination, Params};
use crate::state::AppState;

pub fn router() -> Router<AppState> {
    Router::new().route("/audit", get(list_audit))
}

#[derive(Debug, Serialize, FromRow)]
pub struct AuditEntry {
    pub id: i64,
    pub actor_id: Option<Uuid>,
    pub actor_name: Option<String>,
    pub entity_type: String,
    pub entity_id: Uuid,
    pub action: AuditAction,
    pub changes: serde_json::Value,
    pub created_at: DateTime<Utc>,
}

#[derive(Deserialize)]
pub struct AuditQuery {
    pub entity_type: Option<String>,
    pub entity_id: Option<Uuid>,
    pub actor_id: Option<Uuid>,
    pub page: Option<u32>,
    pub per_page: Option<u32>,
}

const FILTER: &str = "
    WHERE a.organization_id = $1
      AND ($2::text IS NULL OR a.entity_type = $2)
      AND ($3::uuid IS NULL OR a.entity_id = $3)
      AND ($4::uuid IS NULL OR a.actor_id = $4)";

pub async fn list_audit(
    State(state): State<AppState>,
    user: AuthUser,
    ApiQuery(query): ApiQuery<AuditQuery>,
) -> AppResult<Json<Page<AuditEntry>>> {
    let entity_type = match query.entity_type.as_deref() {
        None => None,
        Some(raw) => Some(
            EntityType::parse(raw)
                .ok_or_else(|| AppError::validation("entity_type", "is not a known entity"))?
                .as_str(),
        ),
    };
    let pagination = Pagination::new(query.page, query.per_page);

    let params = Params::default()
        .bind(user.org_id)?
        .bind(entity_type)?
        .bind(query.entity_id)?
        .bind(query.actor_id)?;
    let sql = format!(
        "SELECT a.id, a.actor_id, u.full_name AS actor_name, a.entity_type, a.entity_id,
                a.action, a.changes, a.created_at
         FROM audit_log a LEFT JOIN users u ON u.id = a.actor_id
         {FILTER}"
    );
    let order = "a.created_at DESC, a.id DESC";
    Ok(Json(pagination.fetch(&state.pool, &sql, order, params).await?))
}
