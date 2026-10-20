use axum::extract::State;
use axum::http::StatusCode;
use axum::routing::get;
use axum::{Json, Router};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use utoipa::{IntoParams, ToSchema};
use uuid::Uuid;

use crate::audit;
use crate::auth::AuthUser;
use crate::error::{AppError, AppResult};
use crate::extract::{ApiJson, ApiPath, ApiQuery};
use crate::models::{AuditAction, EntityType};
use crate::pagination::{Page, Pagination};
use crate::state::AppState;
use crate::validation::{clean, like_pattern, Validator};

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/companies", get(list_companies).post(create_company))
        .route(
            "/companies/{id}",
            get(get_company).put(update_company).delete(delete_company),
        )
}

#[derive(Debug, Serialize, ToSchema, FromRow)]
pub struct Company {
    pub id: Uuid,
    pub name: String,
    pub domain: Option<String>,
    pub industry: Option<String>,
    pub owner_id: Uuid,
    pub owner_name: String,
    pub contacts_count: i64,
    pub open_deals_count: i64,
    pub open_deals_value_cents: i64,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// The columns an update can change; used for audit diffs.
#[derive(Debug, Serialize, FromRow)]
struct CompanyFields {
    name: String,
    domain: Option<String>,
    industry: Option<String>,
    owner_id: Uuid,
}

#[derive(Deserialize, ToSchema)]
pub struct CompanyInput {
    pub name: String,
    pub domain: Option<String>,
    pub industry: Option<String>,
    /// Defaults to the caller. Sales reps can only assign themselves.
    pub owner_id: Option<Uuid>,
}

#[derive(Deserialize, IntoParams)]
#[into_params(parameter_in = Query)]
pub struct CompanyQuery {
    /// Case-insensitive substring match on the name or domain.
    pub q: Option<String>,
    pub owner_id: Option<Uuid>,
    pub page: Option<u32>,
    pub per_page: Option<u32>,
}

const SELECT_COMPANY: &str = "
    SELECT c.id, c.name, c.domain, c.industry, c.owner_id, u.full_name AS owner_name,
           (SELECT count(*) FROM contacts ct
             WHERE ct.organization_id = c.organization_id AND ct.company_id = c.id) AS contacts_count,
           (SELECT count(*) FROM deals d JOIN pipeline_stages s ON s.id = d.stage_id
             WHERE d.organization_id = c.organization_id AND d.company_id = c.id
               AND s.kind = 'open') AS open_deals_count,
           (SELECT coalesce(sum(d.value_cents), 0)::bigint FROM deals d JOIN pipeline_stages s ON s.id = d.stage_id
             WHERE d.organization_id = c.organization_id AND d.company_id = c.id
               AND s.kind = 'open') AS open_deals_value_cents,
           c.created_at, c.updated_at
    FROM companies c
    JOIN users u ON u.id = c.owner_id";

const FILTER: &str = "
    WHERE c.organization_id = $1
      AND ($2::text IS NULL OR c.name ILIKE $2 OR c.domain ILIKE $2)
      AND ($3::uuid IS NULL OR c.owner_id = $3)";

#[utoipa::path(
    get, path = "/api/v1/companies", tag = "companies",
    params(CompanyQuery),
    responses((status = 200, body = Page<Company>)),
    security(("bearer" = []))
)]
pub async fn list_companies(
    State(state): State<AppState>,
    user: AuthUser,
    ApiQuery(query): ApiQuery<CompanyQuery>,
) -> AppResult<Json<Page<Company>>> {
    let pagination = Pagination::new(query.page, query.per_page);
    let pattern = clean(query.q).map(|q| like_pattern(&q));

    let total: i64 = sqlx::query_scalar(&format!("SELECT count(*) FROM companies c {FILTER}"))
        .bind(user.org_id)
        .bind(&pattern)
        .bind(query.owner_id)
        .fetch_one(&state.pool)
        .await?;
    let companies = sqlx::query_as(&format!(
        "{SELECT_COMPANY} {FILTER} ORDER BY c.name, c.id LIMIT $4 OFFSET $5"
    ))
    .bind(user.org_id)
    .bind(&pattern)
    .bind(query.owner_id)
    .bind(pagination.limit())
    .bind(pagination.offset())
    .fetch_all(&state.pool)
    .await?;
    Ok(Json(pagination.wrap(companies, total)))
}

pub async fn fetch_company(
    pool: &sqlx::PgPool,
    org_id: Uuid,
    id: Uuid,
) -> AppResult<Company> {
    sqlx::query_as(&format!(
        "{SELECT_COMPANY} WHERE c.organization_id = $1 AND c.id = $2"
    ))
    .bind(org_id)
    .bind(id)
    .fetch_optional(pool)
    .await?
    .ok_or(AppError::NotFound("company"))
}

#[utoipa::path(
    get, path = "/api/v1/companies/{id}", tag = "companies",
    params(("id" = Uuid, Path)),
    responses((status = 200, body = Company), (status = 404, body = crate::error::ErrorBody)),
    security(("bearer" = []))
)]
pub async fn get_company(
    State(state): State<AppState>,
    user: AuthUser,
    ApiPath(id): ApiPath<Uuid>,
) -> AppResult<Json<Company>> {
    Ok(Json(fetch_company(&state.pool, user.org_id, id).await?))
}

struct CleanInput {
    name: String,
    domain: Option<String>,
    industry: Option<String>,
}

fn validate(req: CompanyInput) -> AppResult<(CleanInput, Option<Uuid>)> {
    let input = CleanInput {
        name: req.name.trim().to_owned(),
        domain: clean(req.domain).map(|d| d.to_lowercase()),
        industry: clean(req.industry),
    };
    Validator::new()
        .required("name", &input.name, 200)
        .optional("domain", input.domain.as_deref(), 253)
        .optional("industry", input.industry.as_deref(), 80)
        .finish()?;
    Ok((input, req.owner_id))
}

#[utoipa::path(
    post, path = "/api/v1/companies", tag = "companies",
    request_body = CompanyInput,
    responses((status = 201, body = Company), (status = 422, body = crate::error::ErrorBody)),
    security(("bearer" = []))
)]
pub async fn create_company(
    State(state): State<AppState>,
    actor: AuthUser,
    ApiJson(req): ApiJson<CompanyInput>,
) -> AppResult<(StatusCode, Json<Company>)> {
    let (input, owner) = validate(req)?;
    let owner_id = actor.resolve_owner(owner)?;
    let id = Uuid::now_v7();

    let mut tx = state.pool.begin().await?;
    let fields: CompanyFields = sqlx::query_as(
        "INSERT INTO companies (id, organization_id, name, domain, industry, owner_id)
         VALUES ($1, $2, $3, $4, $5, $6)
         RETURNING name, domain, industry, owner_id",
    )
    .bind(id)
    .bind(actor.org_id)
    .bind(&input.name)
    .bind(&input.domain)
    .bind(&input.industry)
    .bind(owner_id)
    .fetch_one(&mut *tx)
    .await?;
    audit::record(
        &mut tx,
        &actor,
        EntityType::Company,
        id,
        AuditAction::Created,
        audit::snapshot(&fields),
    )
    .await?;
    tx.commit().await?;

    let company = fetch_company(&state.pool, actor.org_id, id).await?;
    Ok((StatusCode::CREATED, Json(company)))
}

#[utoipa::path(
    put, path = "/api/v1/companies/{id}", tag = "companies",
    params(("id" = Uuid, Path)),
    request_body = CompanyInput,
    responses(
        (status = 200, body = Company),
        (status = 403, body = crate::error::ErrorBody),
        (status = 404, body = crate::error::ErrorBody),
    ),
    security(("bearer" = []))
)]
pub async fn update_company(
    State(state): State<AppState>,
    actor: AuthUser,
    ApiPath(id): ApiPath<Uuid>,
    ApiJson(req): ApiJson<CompanyInput>,
) -> AppResult<Json<Company>> {
    let (input, owner) = validate(req)?;

    let mut tx = state.pool.begin().await?;
    let before: CompanyFields = sqlx::query_as(
        "SELECT name, domain, industry, owner_id FROM companies
         WHERE id = $1 AND organization_id = $2 FOR UPDATE",
    )
    .bind(id)
    .bind(actor.org_id)
    .fetch_optional(&mut *tx)
    .await?
    .ok_or(AppError::NotFound("company"))?;
    actor.require_can_modify(before.owner_id)?;
    let owner_id = match owner {
        Some(owner) if owner != before.owner_id => actor.resolve_owner(Some(owner))?,
        _ => before.owner_id,
    };

    let after: CompanyFields = sqlx::query_as(
        "UPDATE companies SET name = $3, domain = $4, industry = $5, owner_id = $6, updated_at = now()
         WHERE id = $1 AND organization_id = $2
         RETURNING name, domain, industry, owner_id",
    )
    .bind(id)
    .bind(actor.org_id)
    .bind(&input.name)
    .bind(&input.domain)
    .bind(&input.industry)
    .bind(owner_id)
    .fetch_one(&mut *tx)
    .await?;
    let changes = audit::diff(&before, &after);
    if !audit::is_empty_diff(&changes) {
        audit::record(&mut tx, &actor, EntityType::Company, id, AuditAction::Updated, changes)
            .await?;
    }
    tx.commit().await?;
    Ok(Json(fetch_company(&state.pool, actor.org_id, id).await?))
}

#[utoipa::path(
    delete, path = "/api/v1/companies/{id}", tag = "companies",
    params(("id" = Uuid, Path)),
    responses((status = 204), (status = 403, description = "Managers and admins only", body = crate::error::ErrorBody)),
    security(("bearer" = []))
)]
pub async fn delete_company(
    State(state): State<AppState>,
    actor: AuthUser,
    ApiPath(id): ApiPath<Uuid>,
) -> AppResult<StatusCode> {
    actor.require_manager()?;
    let mut tx = state.pool.begin().await?;
    let deleted: Option<CompanyFields> = sqlx::query_as(
        "DELETE FROM companies WHERE id = $1 AND organization_id = $2
         RETURNING name, domain, industry, owner_id",
    )
    .bind(id)
    .bind(actor.org_id)
    .fetch_optional(&mut *tx)
    .await?;
    let deleted = deleted.ok_or(AppError::NotFound("company"))?;
    audit::record(
        &mut tx,
        &actor,
        EntityType::Company,
        id,
        AuditAction::Deleted,
        audit::snapshot(&deleted),
    )
    .await?;
    tx.commit().await?;
    Ok(StatusCode::NO_CONTENT)
}
