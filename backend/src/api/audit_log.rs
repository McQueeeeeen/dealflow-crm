use axum::extract::State;
use axum::routing::get;
use axum::{Json, Router};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use utoipa::{IntoParams, ToSchema};
use uuid::Uuid;

use crate::auth::AuthUser;
use crate::error::{AppError, AppResult};
use crate::extract::ApiQuery;
use crate::models::{AuditAction, EntityType};
use crate::pagination::{Page, Pagination};
use crate::state::AppState;

pub fn router() -> Router<AppState> {
    Router::new().route("/audit", get(list_audit))
}

#[derive(Debug, Serialize, ToSchema, FromRow)]
pub struct AuditEntry {
    pub id: i64,
    pub actor_id: Option<Uuid>,
    pub actor_name: Option<String>,
    pub entity_type: String,
    pub entity_id: Uuid,
    pub action: AuditAction,
    /// For updates: `{"field": {"from": old, "to": new}}`. For creates and deletes: a snapshot.
    pub changes: serde_json::Value,
    pub created_at: DateTime<Utc>,
}

#[derive(Deserialize, IntoParams)]
#[into_params(parameter_in = Query)]
pub struct AuditQuery {
    /// One of company, contact, deal, task, user, stage.
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

#[utoipa::path(
    get, path = "/api/v1/audit", tag = "audit",
    params(AuditQuery),
    responses((status = 200, description = "Newest first", body = Page<AuditEntry>)),
    security(("bearer" = []))
)]
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

    let total: i64 = sqlx::query_scalar(&format!("SELECT count(*) FROM audit_log a {FILTER}"))
        .bind(user.org_id)
        .bind(entity_type)
        .bind(query.entity_id)
        .bind(query.actor_id)
        .fetch_one(&state.pool)
        .await?;
    let entries = sqlx::query_as(&format!(
        "SELECT a.id, a.actor_id, u.full_name AS actor_name, a.entity_type, a.entity_id,
                a.action, a.changes, a.created_at
         FROM audit_log a LEFT JOIN users u ON u.id = a.actor_id
         {FILTER}
         ORDER BY a.created_at DESC, a.id DESC
         LIMIT $5 OFFSET $6"
    ))
    .bind(user.org_id)
    .bind(entity_type)
    .bind(query.entity_id)
    .bind(query.actor_id)
    .bind(pagination.limit())
    .bind(pagination.offset())
    .fetch_all(&state.pool)
    .await?;
    Ok(Json(pagination.wrap(entries, total)))
}
