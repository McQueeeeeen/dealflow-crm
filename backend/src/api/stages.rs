use std::collections::HashSet;

use axum::extract::State;
use axum::http::StatusCode;
use axum::routing::{get, put};
use axum::{Json, Router};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use uuid::Uuid;

use crate::audit;
use crate::auth::AuthUser;
use crate::error::{AppError, AppResult};
use crate::extract::{ApiJson, ApiPath};
use crate::models::{EntityType, StageKind};
use crate::state::AppState;
use crate::validation::Validator;

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/stages", get(list_stages).post(create_stage))
        .route("/stages/order", put(reorder_stages))
        .route("/stages/{id}", put(update_stage).delete(delete_stage))
}

#[derive(Debug, Clone, Serialize, FromRow)]
pub struct Stage {
    pub id: Uuid,
    pub name: String,
    pub position: i32,
    pub probability: i16,
    pub kind: StageKind,
}

#[derive(Deserialize)]
pub struct CreateStage {
    pub name: String,
    pub probability: i16,
    pub kind: StageKind,
}

#[derive(Deserialize)]
pub struct UpdateStage {
    pub name: String,
    pub probability: i16,
}

#[derive(Deserialize)]
pub struct StageOrder {
    pub stage_ids: Vec<Uuid>,
}

const STAGE_COLUMNS: &str = "id, name, position, probability, kind";

pub async fn fetch_stages(pool: &sqlx::PgPool, org_id: Uuid) -> AppResult<Vec<Stage>> {
    Ok(sqlx::query_as(&format!(
        "SELECT {STAGE_COLUMNS} FROM pipeline_stages WHERE organization_id = $1 ORDER BY position"
    ))
    .bind(org_id)
    .fetch_all(pool)
    .await?)
}

fn validate(name: &str, probability: i16) -> AppResult<()> {
    Validator::new()
        .required("name", name, 60)
        .check(
            (0..=100).contains(&probability),
            "probability",
            "must be between 0 and 100",
        )
        .finish()
}

pub async fn list_stages(
    State(state): State<AppState>,
    user: AuthUser,
) -> AppResult<Json<Vec<Stage>>> {
    Ok(Json(fetch_stages(&state.pool, user.org_id).await?))
}

pub async fn create_stage(
    State(state): State<AppState>,
    actor: AuthUser,
    ApiJson(req): ApiJson<CreateStage>,
) -> AppResult<(StatusCode, Json<Stage>)> {
    actor.require_admin()?;
    let name = req.name.trim();
    validate(name, req.probability)?;

    let mut tx = state.pool.begin().await?;
    let stage: Stage = sqlx::query_as(&format!(
        "INSERT INTO pipeline_stages (id, organization_id, name, position, probability, kind)
         VALUES ($1, $2, $3,
                 (SELECT coalesce(max(position) + 1, 0) FROM pipeline_stages WHERE organization_id = $2),
                 $4, $5)
         RETURNING {STAGE_COLUMNS}"
    ))
    .bind(Uuid::now_v7())
    .bind(actor.org_id)
    .bind(name)
    .bind(req.probability)
    .bind(req.kind)
    .fetch_one(&mut *tx)
    .await?;
    audit::created(&mut tx, &actor, EntityType::Stage, stage.id, &stage).await?;
    tx.commit().await?;
    Ok((StatusCode::CREATED, Json(stage)))
}

pub async fn update_stage(
    State(state): State<AppState>,
    actor: AuthUser,
    ApiPath(id): ApiPath<Uuid>,
    ApiJson(req): ApiJson<UpdateStage>,
) -> AppResult<Json<Stage>> {
    actor.require_admin()?;
    let name = req.name.trim();
    validate(name, req.probability)?;

    let mut tx = state.pool.begin().await?;
    let before: Stage = sqlx::query_as(&format!(
        "SELECT {STAGE_COLUMNS} FROM pipeline_stages
         WHERE id = $1 AND organization_id = $2 FOR UPDATE"
    ))
    .bind(id)
    .bind(actor.org_id)
    .fetch_optional(&mut *tx)
    .await?
    .ok_or(AppError::NotFound("stage"))?;
    let after: Stage = sqlx::query_as(&format!(
        "UPDATE pipeline_stages SET name = $3, probability = $4
         WHERE id = $1 AND organization_id = $2
         RETURNING {STAGE_COLUMNS}"
    ))
    .bind(id)
    .bind(actor.org_id)
    .bind(name)
    .bind(req.probability)
    .fetch_one(&mut *tx)
    .await?;
    audit::updated(&mut tx, &actor, EntityType::Stage, id, &before, &after).await?;
    tx.commit().await?;
    Ok(Json(after))
}

pub async fn reorder_stages(
    State(state): State<AppState>,
    actor: AuthUser,
    ApiJson(req): ApiJson<StageOrder>,
) -> AppResult<Json<Vec<Stage>>> {
    actor.require_admin()?;
    let existing: HashSet<Uuid> = fetch_stages(&state.pool, actor.org_id)
        .await?
        .into_iter()
        .map(|stage| stage.id)
        .collect();
    let requested: HashSet<Uuid> = req.stage_ids.iter().copied().collect();
    if requested.len() != req.stage_ids.len() || requested != existing {
        return Err(AppError::validation(
            "stage_ids",
            "must list every stage exactly once",
        ));
    }

    let mut tx = state.pool.begin().await?;
    sqlx::query(
        "UPDATE pipeline_stages s SET position = o.position - 1
         FROM unnest($2::uuid[]) WITH ORDINALITY AS o(id, position)
         WHERE s.id = o.id AND s.organization_id = $1",
    )
    .bind(actor.org_id)
    .bind(&req.stage_ids)
    .execute(&mut *tx)
    .await?;
    tx.commit().await?;
    Ok(Json(fetch_stages(&state.pool, actor.org_id).await?))
}

pub async fn delete_stage(
    State(state): State<AppState>,
    actor: AuthUser,
    ApiPath(id): ApiPath<Uuid>,
) -> AppResult<StatusCode> {
    actor.require_admin()?;
    let stages = fetch_stages(&state.pool, actor.org_id).await?;
    let stage = stages
        .iter()
        .find(|stage| stage.id == id)
        .ok_or(AppError::NotFound("stage"))?;
    let open_stages = stages.iter().filter(|s| s.kind == StageKind::Open).count();
    if stage.kind == StageKind::Open && open_stages == 1 {
        return Err(AppError::Conflict(
            "the pipeline needs at least one open stage".into(),
        ));
    }

    let mut tx = state.pool.begin().await?;
    sqlx::query("DELETE FROM pipeline_stages WHERE id = $1 AND organization_id = $2")
        .bind(id)
        .bind(actor.org_id)
        .execute(&mut *tx)
        .await
        .map_err(|err| match AppError::from(err) {
            AppError::Validation(_) | AppError::Conflict(_) => {
                AppError::Conflict("move or delete the deals in this stage first".into())
            }
            other => other,
        })?;
    audit::deleted(&mut tx, &actor, EntityType::Stage, id, stage).await?;
    tx.commit().await?;
    Ok(StatusCode::NO_CONTENT)
}
