use axum::extract::State;
use axum::routing::{get, post};
use axum::{Json, Router};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use sqlx::FromRow;
use uuid::Uuid;

use crate::audit;
use crate::auth::AuthUser;
use crate::error::{AppError, AppResult};
use crate::extract::{ApiPath, ApiQuery};
use crate::models::{AuditAction, EntityType};
use crate::pagination::{Page, Pagination, Params};
use crate::state::AppState;

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/audit", get(list_audit))
        .route("/audit/{id}/revert", post(revert))
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
    Ok(Json(
        pagination.fetch(&state.pool, &sql, order, params).await?,
    ))
}

const REVERTIBLE: [(EntityType, &str, &[&str]); 3] = [
    (
        EntityType::Deal,
        "deals",
        &[
            "title",
            "value_cents",
            "company_id",
            "contact_id",
            "owner_id",
            "expected_close_date",
        ],
    ),
    (
        EntityType::Contact,
        "contacts",
        &[
            "first_name",
            "last_name",
            "email",
            "phone",
            "job_title",
            "company_id",
            "owner_id",
        ],
    ),
    (
        EntityType::Company,
        "companies",
        &["name", "domain", "industry", "owner_id"],
    ),
];

#[derive(Serialize)]
pub struct Reverted {
    pub entity_type: String,
    pub entity_id: Uuid,
    pub fields: Vec<&'static str>,
}

pub async fn revert(
    State(state): State<AppState>,
    actor: AuthUser,
    ApiPath(id): ApiPath<i64>,
) -> AppResult<Json<Reverted>> {
    let mut tx = state.pool.begin().await?;
    let (entity_type, entity_id, action, changes): (String, Uuid, AuditAction, Value) =
        sqlx::query_as(
            "SELECT entity_type, entity_id, action, changes FROM audit_log
             WHERE id = $1 AND organization_id = $2",
        )
        .bind(id)
        .bind(actor.org_id)
        .fetch_optional(&mut *tx)
        .await?
        .ok_or(AppError::NotFound("change"))?;
    let Some(&(entity, table, revertible)) = REVERTIBLE
        .iter()
        .find(|(e, _, _)| e.as_str() == entity_type)
        .filter(|_| action == AuditAction::Updated)
    else {
        return Err(AppError::BadRequest(
            "only edits to deals, contacts and companies can be undone".into(),
        ));
    };
    let fields: Vec<&'static str> = revertible
        .iter()
        .copied()
        .filter(|f| changes.get(*f).is_some())
        .collect();
    if fields.is_empty() {
        return Err(AppError::BadRequest(
            "nothing in this change can be undone".into(),
        ));
    }
    let side = |key: &str| -> Value {
        Value::Object(
            fields
                .iter()
                .map(|f| (f.to_string(), changes[*f][key].clone()))
                .collect::<Map<_, _>>(),
        )
    };
    let (restore, expected) = (side("from"), side("to"));

    let current: Option<(Uuid, bool, Value)> = sqlx::query_as(&format!(
        "SELECT owner_id, to_jsonb(t) @> $3, to_jsonb(t) - 'organization_id' - 'created_at' - 'updated_at'
         FROM {table} t WHERE id = $1 AND organization_id = $2 FOR UPDATE"
    ))
    .bind(entity_id)
    .bind(actor.org_id)
    .bind(&expected)
    .fetch_optional(&mut *tx)
    .await?;
    let (owner, unchanged, before) = current.ok_or(AppError::NotFound("record"))?;
    actor.require_can_modify(owner)?;
    if !unchanged {
        return Err(AppError::Conflict(
            "these fields changed again since, undo the later change first".into(),
        ));
    }

    let assignments: Vec<String> = fields.iter().map(|f| format!("{f} = r.{f}")).collect();
    let version = if table == "deals" {
        ", version = t.version + 1"
    } else {
        ""
    };
    let after: Value = sqlx::query_scalar(&format!(
        "UPDATE {table} t SET {}, updated_at = now(){version}
         FROM jsonb_populate_record(NULL::{table}, $2) r
         WHERE t.id = $1
         RETURNING to_jsonb(t) - 'organization_id' - 'created_at' - 'updated_at'",
        assignments.join(", ")
    ))
    .bind(entity_id)
    .bind(&restore)
    .fetch_one(&mut *tx)
    .await?;
    audit::updated(&mut tx, &actor, entity, entity_id, &before, &after).await?;
    tx.commit().await?;
    Ok(Json(Reverted {
        entity_type,
        entity_id,
        fields,
    }))
}
