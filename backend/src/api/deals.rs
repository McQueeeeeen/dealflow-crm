use axum::extract::State;
use axum::http::StatusCode;
use axum::routing::{get, post};
use axum::{Json, Router};
use chrono::{DateTime, NaiveDate, Utc};
use serde::{Deserialize, Serialize};
use sqlx::{FromRow, PgConnection};
use utoipa::{IntoParams, ToSchema};
use uuid::Uuid;

use crate::api::stages::{fetch_stages, Stage};
use crate::audit;
use crate::auth::AuthUser;
use crate::error::{AppError, AppResult};
use crate::extract::{ApiJson, ApiPath, ApiQuery};
use crate::models::{AuditAction, EntityType, StageKind};
use crate::pagination::{Page, Pagination};
use crate::state::AppState;
use crate::validation::{clean, like_pattern, Validator};

/// Upper bound for a single deal: ten trillion in the minor unit, far beyond any real deal
/// and well inside i64 when summed across a pipeline.
const MAX_VALUE_CENTS: i64 = 1_000_000_000_000_000;
const BOARD_GAP: f64 = 1024.0;
const BOARD_LIMIT: i64 = 1000;

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/deals", get(list_deals).post(create_deal))
        .route("/deals/board", get(board))
        .route(
            "/deals/{id}",
            get(get_deal).put(update_deal).delete(delete_deal),
        )
        .route("/deals/{id}/move", post(move_deal))
        .route("/deals/{id}/stage-history", get(stage_history))
}

#[derive(Debug, Clone, Serialize, ToSchema, FromRow)]
pub struct Deal {
    pub id: Uuid,
    pub title: String,
    /// Amount in the minor unit of the organization currency.
    pub value_cents: i64,
    pub stage_id: Uuid,
    pub stage_name: String,
    pub stage_kind: StageKind,
    pub probability: i16,
    pub board_position: f64,
    pub company_id: Option<Uuid>,
    pub company_name: Option<String>,
    pub contact_id: Option<Uuid>,
    pub contact_name: Option<String>,
    pub owner_id: Uuid,
    pub owner_name: String,
    pub expected_close_date: Option<NaiveDate>,
    pub closed_at: Option<DateTime<Utc>>,
    /// Optimistic concurrency token. Send it back on update or move.
    pub version: i32,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Serialize, FromRow)]
struct DealFields {
    title: String,
    value_cents: i64,
    stage_id: Uuid,
    company_id: Option<Uuid>,
    contact_id: Option<Uuid>,
    owner_id: Uuid,
    expected_close_date: Option<NaiveDate>,
}

#[derive(Debug, FromRow)]
struct LockedDeal {
    #[sqlx(flatten)]
    fields: DealFields,
    version: i32,
    stage_kind: StageKind,
    closed_at: Option<DateTime<Utc>>,
}

#[derive(Deserialize, ToSchema)]
pub struct CreateDeal {
    pub title: String,
    #[serde(default)]
    pub value_cents: i64,
    /// Defaults to the first open stage of the pipeline.
    pub stage_id: Option<Uuid>,
    pub company_id: Option<Uuid>,
    pub contact_id: Option<Uuid>,
    pub owner_id: Option<Uuid>,
    pub expected_close_date: Option<NaiveDate>,
}

#[derive(Deserialize, ToSchema)]
pub struct UpdateDeal {
    pub title: String,
    pub value_cents: i64,
    pub company_id: Option<Uuid>,
    pub contact_id: Option<Uuid>,
    pub owner_id: Option<Uuid>,
    pub expected_close_date: Option<NaiveDate>,
    pub version: i32,
}

#[derive(Deserialize, ToSchema)]
pub struct MoveDeal {
    pub stage_id: Uuid,
    /// Sort key inside the target column. Omit to place the deal at the top.
    pub board_position: Option<f64>,
    pub version: i32,
}

#[derive(Deserialize, IntoParams)]
#[into_params(parameter_in = Query)]
pub struct DealQuery {
    /// Case-insensitive substring match on the title.
    pub q: Option<String>,
    pub stage_id: Option<Uuid>,
    pub owner_id: Option<Uuid>,
    pub company_id: Option<Uuid>,
    pub contact_id: Option<Uuid>,
    pub status: Option<StageKind>,
    pub page: Option<u32>,
    pub per_page: Option<u32>,
}

#[derive(Deserialize, IntoParams)]
#[into_params(parameter_in = Query)]
pub struct BoardQuery {
    pub q: Option<String>,
    pub owner_id: Option<Uuid>,
}

#[derive(Serialize, ToSchema)]
pub struct BoardColumn {
    pub stage: Stage,
    pub deals: Vec<Deal>,
    pub count: usize,
    pub total_value_cents: i64,
}

#[derive(Serialize, ToSchema, FromRow)]
pub struct StageChange {
    pub from_stage_name: Option<String>,
    pub to_stage_name: String,
    pub changed_by_name: Option<String>,
    pub changed_at: DateTime<Utc>,
}

const SELECT_DEAL: &str = "
    SELECT d.id, d.title, d.value_cents, d.stage_id, s.name AS stage_name, s.kind AS stage_kind,
           s.probability, d.board_position, d.company_id, co.name AS company_name, d.contact_id,
           trim(ct.first_name || ' ' || ct.last_name) AS contact_name,
           d.owner_id, u.full_name AS owner_name, d.expected_close_date, d.closed_at, d.version,
           d.created_at, d.updated_at
    FROM deals d
    JOIN pipeline_stages s ON s.id = d.stage_id
    JOIN users u ON u.id = d.owner_id
    LEFT JOIN companies co ON co.id = d.company_id
    LEFT JOIN contacts ct ON ct.id = d.contact_id";

const FILTER: &str = "
    WHERE d.organization_id = $1
      AND ($2::text IS NULL OR d.title ILIKE $2)
      AND ($3::uuid IS NULL OR d.stage_id = $3)
      AND ($4::uuid IS NULL OR d.owner_id = $4)
      AND ($5::uuid IS NULL OR d.company_id = $5)
      AND ($6::uuid IS NULL OR d.contact_id = $6)
      AND ($7::stage_kind IS NULL OR s.kind = $7)";

const FIELD_COLUMNS: &str =
    "title, value_cents, stage_id, company_id, contact_id, owner_id, expected_close_date";

#[utoipa::path(
    get, path = "/api/v1/deals", tag = "deals",
    params(DealQuery),
    responses((status = 200, body = Page<Deal>)),
    security(("bearer" = []))
)]
pub async fn list_deals(
    State(state): State<AppState>,
    user: AuthUser,
    ApiQuery(query): ApiQuery<DealQuery>,
) -> AppResult<Json<Page<Deal>>> {
    let pagination = Pagination::new(query.page, query.per_page);
    let pattern = clean(query.q).map(|q| like_pattern(&q));

    let total: i64 = sqlx::query_scalar(&format!(
        "SELECT count(*) FROM deals d JOIN pipeline_stages s ON s.id = d.stage_id {FILTER}"
    ))
    .bind(user.org_id)
    .bind(&pattern)
    .bind(query.stage_id)
    .bind(query.owner_id)
    .bind(query.company_id)
    .bind(query.contact_id)
    .bind(query.status)
    .fetch_one(&state.pool)
    .await?;
    let deals = sqlx::query_as(&format!(
        "{SELECT_DEAL} {FILTER} ORDER BY d.updated_at DESC, d.id LIMIT $8 OFFSET $9"
    ))
    .bind(user.org_id)
    .bind(&pattern)
    .bind(query.stage_id)
    .bind(query.owner_id)
    .bind(query.company_id)
    .bind(query.contact_id)
    .bind(query.status)
    .bind(pagination.limit())
    .bind(pagination.offset())
    .fetch_all(&state.pool)
    .await?;
    Ok(Json(pagination.wrap(deals, total)))
}

#[utoipa::path(
    get, path = "/api/v1/deals/board", tag = "deals",
    params(BoardQuery),
    responses((status = 200, description = "One column per stage, in pipeline order", body = Vec<BoardColumn>)),
    security(("bearer" = []))
)]
pub async fn board(
    State(state): State<AppState>,
    user: AuthUser,
    ApiQuery(query): ApiQuery<BoardQuery>,
) -> AppResult<Json<Vec<BoardColumn>>> {
    let stages = fetch_stages(&state.pool, user.org_id).await?;
    let pattern = clean(query.q).map(|q| like_pattern(&q));
    let deals: Vec<Deal> = sqlx::query_as(&format!(
        "{SELECT_DEAL}
         WHERE d.organization_id = $1
           AND ($2::text IS NULL OR d.title ILIKE $2)
           AND ($3::uuid IS NULL OR d.owner_id = $3)
         ORDER BY d.board_position, d.id
         LIMIT $4"
    ))
    .bind(user.org_id)
    .bind(&pattern)
    .bind(query.owner_id)
    .bind(BOARD_LIMIT)
    .fetch_all(&state.pool)
    .await?;

    let columns = stages
        .into_iter()
        .map(|stage| {
            let deals: Vec<Deal> = deals
                .iter()
                .filter(|deal| deal.stage_id == stage.id)
                .cloned()
                .collect();
            BoardColumn {
                count: deals.len(),
                total_value_cents: deals.iter().map(|d| d.value_cents).sum(),
                stage,
                deals,
            }
        })
        .collect();
    Ok(Json(columns))
}

async fn fetch_deal(conn: &mut PgConnection, org_id: Uuid, id: Uuid) -> AppResult<Deal> {
    sqlx::query_as(&format!(
        "{SELECT_DEAL} WHERE d.organization_id = $1 AND d.id = $2"
    ))
    .bind(org_id)
    .bind(id)
    .fetch_optional(conn)
    .await?
    .ok_or(AppError::NotFound("deal"))
}

#[utoipa::path(
    get, path = "/api/v1/deals/{id}", tag = "deals",
    params(("id" = Uuid, Path)),
    responses((status = 200, body = Deal), (status = 404, body = crate::error::ErrorBody)),
    security(("bearer" = []))
)]
pub async fn get_deal(
    State(state): State<AppState>,
    user: AuthUser,
    ApiPath(id): ApiPath<Uuid>,
) -> AppResult<Json<Deal>> {
    let mut conn = state.pool.acquire().await?;
    Ok(Json(fetch_deal(&mut conn, user.org_id, id).await?))
}

fn validate_common(title: &str, value_cents: i64) -> AppResult<()> {
    Validator::new()
        .required("title", title, 200)
        .check(
            (0..=MAX_VALUE_CENTS).contains(&value_cents),
            "value_cents",
            "must be zero or a positive amount",
        )
        .finish()
}

async fn stage_kind(conn: &mut PgConnection, org_id: Uuid, stage_id: Uuid) -> AppResult<StageKind> {
    sqlx::query_scalar("SELECT kind FROM pipeline_stages WHERE id = $1 AND organization_id = $2")
        .bind(stage_id)
        .bind(org_id)
        .fetch_optional(conn)
        .await?
        .ok_or_else(|| AppError::validation("stage_id", "does not exist"))
}

async fn top_of_column(conn: &mut PgConnection, org_id: Uuid, stage_id: Uuid) -> AppResult<f64> {
    let min: Option<f64> = sqlx::query_scalar(
        "SELECT min(board_position) FROM deals WHERE organization_id = $1 AND stage_id = $2",
    )
    .bind(org_id)
    .bind(stage_id)
    .fetch_one(conn)
    .await?;
    Ok(min.map_or(0.0, |min| min - BOARD_GAP))
}

async fn record_stage_change(
    conn: &mut PgConnection,
    actor: &AuthUser,
    deal_id: Uuid,
    from: Option<Uuid>,
    to: Uuid,
) -> AppResult<()> {
    sqlx::query(
        "INSERT INTO deal_stage_changes (organization_id, deal_id, from_stage_id, to_stage_id, changed_by)
         VALUES ($1, $2, $3, $4, $5)",
    )
    .bind(actor.org_id)
    .bind(deal_id)
    .bind(from)
    .bind(to)
    .bind(actor.user_id)
    .execute(conn)
    .await?;
    Ok(())
}

#[utoipa::path(
    post, path = "/api/v1/deals", tag = "deals",
    request_body = CreateDeal,
    responses((status = 201, body = Deal), (status = 422, body = crate::error::ErrorBody)),
    security(("bearer" = []))
)]
pub async fn create_deal(
    State(state): State<AppState>,
    actor: AuthUser,
    ApiJson(req): ApiJson<CreateDeal>,
) -> AppResult<(StatusCode, Json<Deal>)> {
    let title = req.title.trim();
    validate_common(title, req.value_cents)?;
    let owner_id = actor.resolve_owner(req.owner_id)?;

    let mut tx = state.pool.begin().await?;
    let stage_id = match req.stage_id {
        Some(id) => id,
        None => sqlx::query_scalar(
            "SELECT id FROM pipeline_stages
             WHERE organization_id = $1 AND kind = 'open' ORDER BY position LIMIT 1",
        )
        .bind(actor.org_id)
        .fetch_one(&mut *tx)
        .await?,
    };
    let kind = stage_kind(&mut tx, actor.org_id, stage_id).await?;
    let closed_at = (kind != StageKind::Open).then(Utc::now);
    let position = top_of_column(&mut tx, actor.org_id, stage_id).await?;
    let id = Uuid::now_v7();

    let fields: DealFields = sqlx::query_as(&format!(
        "INSERT INTO deals (id, organization_id, title, value_cents, stage_id, board_position,
                            company_id, contact_id, owner_id, expected_close_date, closed_at)
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11)
         RETURNING {FIELD_COLUMNS}"
    ))
    .bind(id)
    .bind(actor.org_id)
    .bind(title)
    .bind(req.value_cents)
    .bind(stage_id)
    .bind(position)
    .bind(req.company_id)
    .bind(req.contact_id)
    .bind(owner_id)
    .bind(req.expected_close_date)
    .bind(closed_at)
    .fetch_one(&mut *tx)
    .await?;
    record_stage_change(&mut tx, &actor, id, None, stage_id).await?;
    audit::record(
        &mut tx,
        &actor,
        EntityType::Deal,
        id,
        AuditAction::Created,
        audit::snapshot(&fields),
    )
    .await?;
    let deal = fetch_deal(&mut tx, actor.org_id, id).await?;
    tx.commit().await?;
    Ok((StatusCode::CREATED, Json(deal)))
}

/// Locks the deal row and checks the caller's version, so two people editing the same
/// deal cannot silently overwrite each other.
async fn lock_for_write(
    conn: &mut PgConnection,
    actor: &AuthUser,
    id: Uuid,
    expected_version: i32,
) -> AppResult<LockedDeal> {
    let locked: LockedDeal = sqlx::query_as(
        "SELECT d.title, d.value_cents, d.stage_id, d.company_id, d.contact_id, d.owner_id,
                d.expected_close_date, d.version, s.kind AS stage_kind, d.closed_at
         FROM deals d JOIN pipeline_stages s ON s.id = d.stage_id
         WHERE d.id = $1 AND d.organization_id = $2
         FOR UPDATE OF d",
    )
    .bind(id)
    .bind(actor.org_id)
    .fetch_optional(conn)
    .await?
    .ok_or(AppError::NotFound("deal"))?;
    actor.require_can_modify(locked.fields.owner_id)?;
    if locked.version != expected_version {
        return Err(AppError::Conflict(
            "this deal was changed by someone else, reload it and try again".into(),
        ));
    }
    Ok(locked)
}

#[utoipa::path(
    put, path = "/api/v1/deals/{id}", tag = "deals",
    params(("id" = Uuid, Path)),
    request_body = UpdateDeal,
    responses(
        (status = 200, body = Deal),
        (status = 403, body = crate::error::ErrorBody),
        (status = 404, body = crate::error::ErrorBody),
        (status = 409, description = "Version mismatch", body = crate::error::ErrorBody),
    ),
    security(("bearer" = []))
)]
pub async fn update_deal(
    State(state): State<AppState>,
    actor: AuthUser,
    ApiPath(id): ApiPath<Uuid>,
    ApiJson(req): ApiJson<UpdateDeal>,
) -> AppResult<Json<Deal>> {
    let title = req.title.trim();
    validate_common(title, req.value_cents)?;

    let mut tx = state.pool.begin().await?;
    let before = lock_for_write(&mut tx, &actor, id, req.version).await?.fields;
    let owner_id = match req.owner_id {
        Some(owner) if owner != before.owner_id => actor.resolve_owner(Some(owner))?,
        _ => before.owner_id,
    };

    let after: DealFields = sqlx::query_as(&format!(
        "UPDATE deals SET title = $3, value_cents = $4, company_id = $5, contact_id = $6,
                owner_id = $7, expected_close_date = $8, version = version + 1, updated_at = now()
         WHERE id = $1 AND organization_id = $2
         RETURNING {FIELD_COLUMNS}"
    ))
    .bind(id)
    .bind(actor.org_id)
    .bind(title)
    .bind(req.value_cents)
    .bind(req.company_id)
    .bind(req.contact_id)
    .bind(owner_id)
    .bind(req.expected_close_date)
    .fetch_one(&mut *tx)
    .await?;
    let changes = audit::diff(&before, &after);
    if !audit::is_empty_diff(&changes) {
        audit::record(&mut tx, &actor, EntityType::Deal, id, AuditAction::Updated, changes).await?;
    }
    let deal = fetch_deal(&mut tx, actor.org_id, id).await?;
    tx.commit().await?;
    Ok(Json(deal))
}

#[utoipa::path(
    post, path = "/api/v1/deals/{id}/move", tag = "deals",
    params(("id" = Uuid, Path)),
    request_body = MoveDeal,
    responses(
        (status = 200, description = "Deal moved; closing stages stamp closed_at", body = Deal),
        (status = 409, description = "Version mismatch", body = crate::error::ErrorBody),
    ),
    security(("bearer" = []))
)]
pub async fn move_deal(
    State(state): State<AppState>,
    actor: AuthUser,
    ApiPath(id): ApiPath<Uuid>,
    ApiJson(req): ApiJson<MoveDeal>,
) -> AppResult<Json<Deal>> {
    if req.board_position.is_some_and(|p| !p.is_finite()) {
        return Err(AppError::validation("board_position", "must be a finite number"));
    }

    let mut tx = state.pool.begin().await?;
    let locked = lock_for_write(&mut tx, &actor, id, req.version).await?;
    let target_kind = stage_kind(&mut tx, actor.org_id, req.stage_id).await?;
    let stage_changed = locked.fields.stage_id != req.stage_id;

    let closed_at = match target_kind {
        StageKind::Open => None,
        _ if stage_changed || locked.stage_kind == StageKind::Open => Some(Utc::now()),
        _ => locked.closed_at,
    };
    let position = match req.board_position {
        Some(position) => position,
        None => top_of_column(&mut tx, actor.org_id, req.stage_id).await?,
    };

    let after: DealFields = sqlx::query_as(&format!(
        "UPDATE deals SET stage_id = $3, board_position = $4, closed_at = $5,
                version = version + 1, updated_at = now()
         WHERE id = $1 AND organization_id = $2
         RETURNING {FIELD_COLUMNS}"
    ))
    .bind(id)
    .bind(actor.org_id)
    .bind(req.stage_id)
    .bind(position)
    .bind(closed_at)
    .fetch_one(&mut *tx)
    .await?;

    if stage_changed {
        record_stage_change(&mut tx, &actor, id, Some(locked.fields.stage_id), req.stage_id).await?;
        let changes = audit::diff(&locked.fields, &after);
        audit::record(&mut tx, &actor, EntityType::Deal, id, AuditAction::Updated, changes).await?;
    }
    let deal = fetch_deal(&mut tx, actor.org_id, id).await?;
    tx.commit().await?;
    Ok(Json(deal))
}

#[utoipa::path(
    delete, path = "/api/v1/deals/{id}", tag = "deals",
    params(("id" = Uuid, Path)),
    responses((status = 204), (status = 403, description = "Managers and admins only", body = crate::error::ErrorBody)),
    security(("bearer" = []))
)]
pub async fn delete_deal(
    State(state): State<AppState>,
    actor: AuthUser,
    ApiPath(id): ApiPath<Uuid>,
) -> AppResult<StatusCode> {
    actor.require_manager()?;
    let mut tx = state.pool.begin().await?;
    let deleted: Option<DealFields> = sqlx::query_as(&format!(
        "DELETE FROM deals WHERE id = $1 AND organization_id = $2 RETURNING {FIELD_COLUMNS}"
    ))
    .bind(id)
    .bind(actor.org_id)
    .fetch_optional(&mut *tx)
    .await?;
    let deleted = deleted.ok_or(AppError::NotFound("deal"))?;
    audit::record(
        &mut tx,
        &actor,
        EntityType::Deal,
        id,
        AuditAction::Deleted,
        audit::snapshot(&deleted),
    )
    .await?;
    tx.commit().await?;
    Ok(StatusCode::NO_CONTENT)
}

#[utoipa::path(
    get, path = "/api/v1/deals/{id}/stage-history", tag = "deals",
    params(("id" = Uuid, Path)),
    responses((status = 200, description = "Oldest change first", body = Vec<StageChange>)),
    security(("bearer" = []))
)]
pub async fn stage_history(
    State(state): State<AppState>,
    user: AuthUser,
    ApiPath(id): ApiPath<Uuid>,
) -> AppResult<Json<Vec<StageChange>>> {
    let mut conn = state.pool.acquire().await?;
    fetch_deal(&mut conn, user.org_id, id).await?;
    let history = sqlx::query_as(
        "SELECT fs.name AS from_stage_name, ts.name AS to_stage_name,
                u.full_name AS changed_by_name, c.changed_at
         FROM deal_stage_changes c
         JOIN pipeline_stages ts ON ts.id = c.to_stage_id
         LEFT JOIN pipeline_stages fs ON fs.id = c.from_stage_id
         LEFT JOIN users u ON u.id = c.changed_by
         WHERE c.organization_id = $1 AND c.deal_id = $2
         ORDER BY c.changed_at, c.id",
    )
    .bind(user.org_id)
    .bind(id)
    .fetch_all(&mut *conn)
    .await?;
    Ok(Json(history))
}
