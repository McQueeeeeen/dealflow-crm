use axum::extract::State;
use axum::http::StatusCode;
use axum::routing::get;
use axum::{Json, Router};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use uuid::Uuid;

use crate::audit;
use crate::auth::AuthUser;
use crate::error::{AppError, AppResult};
use crate::extract::{ApiJson, ApiPath, ApiQuery};
use crate::models::{AuditAction, EntityType};
use crate::pagination::{Page, Pagination};
use crate::state::AppState;
use crate::validation::{clean, like_pattern, normalize_email, Validator};

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/contacts", get(list_contacts).post(create_contact))
        .route(
            "/contacts/{id}",
            get(get_contact).put(update_contact).delete(delete_contact),
        )
}

#[derive(Debug, Serialize, FromRow)]
pub struct Contact {
    pub id: Uuid,
    pub first_name: String,
    pub last_name: String,
    pub email: Option<String>,
    pub phone: Option<String>,
    pub job_title: Option<String>,
    pub company_id: Option<Uuid>,
    pub company_name: Option<String>,
    pub owner_id: Uuid,
    pub owner_name: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Serialize, FromRow)]
struct ContactFields {
    first_name: String,
    last_name: String,
    email: Option<String>,
    phone: Option<String>,
    job_title: Option<String>,
    company_id: Option<Uuid>,
    owner_id: Uuid,
}

#[derive(Deserialize)]
pub struct ContactInput {
    pub first_name: String,
    #[serde(default)]
    pub last_name: String,
    pub email: Option<String>,
    pub phone: Option<String>,
    pub job_title: Option<String>,
    pub company_id: Option<Uuid>,
    pub owner_id: Option<Uuid>,
}

#[derive(Deserialize)]
pub struct ContactQuery {
    pub q: Option<String>,
    pub company_id: Option<Uuid>,
    pub owner_id: Option<Uuid>,
    pub page: Option<u32>,
    pub per_page: Option<u32>,
}

const SELECT_CONTACT: &str = "
    SELECT ct.id, ct.first_name, ct.last_name, ct.email, ct.phone, ct.job_title,
           ct.company_id, co.name AS company_name, ct.owner_id, u.full_name AS owner_name,
           ct.created_at, ct.updated_at
    FROM contacts ct
    JOIN users u ON u.id = ct.owner_id
    LEFT JOIN companies co ON co.id = ct.company_id";

const FILTER: &str = "
    WHERE ct.organization_id = $1
      AND ($2::text IS NULL
           OR (ct.first_name || ' ' || ct.last_name || ' ' || coalesce(ct.email, '')) ILIKE $2)
      AND ($3::uuid IS NULL OR ct.company_id = $3)
      AND ($4::uuid IS NULL OR ct.owner_id = $4)";

const RETURNING_FIELDS: &str =
    "first_name, last_name, email, phone, job_title, company_id, owner_id";

pub async fn list_contacts(
    State(state): State<AppState>,
    user: AuthUser,
    ApiQuery(query): ApiQuery<ContactQuery>,
) -> AppResult<Json<Page<Contact>>> {
    let pagination = Pagination::new(query.page, query.per_page);
    let pattern = clean(query.q).map(|q| like_pattern(&q));

    let total: i64 = sqlx::query_scalar(&format!("SELECT count(*) FROM contacts ct {FILTER}"))
        .bind(user.org_id)
        .bind(&pattern)
        .bind(query.company_id)
        .bind(query.owner_id)
        .fetch_one(&state.pool)
        .await?;
    let contacts = sqlx::query_as(&format!(
        "{SELECT_CONTACT} {FILTER}
         ORDER BY ct.first_name, ct.last_name, ct.id LIMIT $5 OFFSET $6"
    ))
    .bind(user.org_id)
    .bind(&pattern)
    .bind(query.company_id)
    .bind(query.owner_id)
    .bind(pagination.limit())
    .bind(pagination.offset())
    .fetch_all(&state.pool)
    .await?;
    Ok(Json(pagination.wrap(contacts, total)))
}

async fn fetch_contact(pool: &sqlx::PgPool, org_id: Uuid, id: Uuid) -> AppResult<Contact> {
    sqlx::query_as(&format!(
        "{SELECT_CONTACT} WHERE ct.organization_id = $1 AND ct.id = $2"
    ))
    .bind(org_id)
    .bind(id)
    .fetch_optional(pool)
    .await?
    .ok_or(AppError::NotFound("contact"))
}

pub async fn get_contact(
    State(state): State<AppState>,
    user: AuthUser,
    ApiPath(id): ApiPath<Uuid>,
) -> AppResult<Json<Contact>> {
    Ok(Json(fetch_contact(&state.pool, user.org_id, id).await?))
}

struct CleanInput {
    first_name: String,
    last_name: String,
    email: Option<String>,
    phone: Option<String>,
    job_title: Option<String>,
    company_id: Option<Uuid>,
}

fn validate(req: ContactInput) -> AppResult<(CleanInput, Option<Uuid>)> {
    let input = CleanInput {
        first_name: req.first_name.trim().to_owned(),
        last_name: req.last_name.trim().to_owned(),
        email: clean(req.email).map(|e| normalize_email(&e)),
        phone: clean(req.phone),
        job_title: clean(req.job_title),
        company_id: req.company_id,
    };
    let mut validator = Validator::new();
    validator
        .required("first_name", &input.first_name, 100)
        .optional("last_name", Some(&input.last_name), 100)
        .optional("phone", input.phone.as_deref(), 40)
        .optional("job_title", input.job_title.as_deref(), 120);
    if let Some(email) = &input.email {
        validator.email("email", email);
    }
    validator.finish()?;
    Ok((input, req.owner_id))
}

pub async fn create_contact(
    State(state): State<AppState>,
    actor: AuthUser,
    ApiJson(req): ApiJson<ContactInput>,
) -> AppResult<(StatusCode, Json<Contact>)> {
    let (input, owner) = validate(req)?;
    let owner_id = actor.resolve_owner(owner)?;
    let id = Uuid::now_v7();

    let mut tx = state.pool.begin().await?;
    let fields: ContactFields = sqlx::query_as(&format!(
        "INSERT INTO contacts
            (id, organization_id, first_name, last_name, email, phone, job_title, company_id, owner_id)
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)
         RETURNING {RETURNING_FIELDS}"
    ))
    .bind(id)
    .bind(actor.org_id)
    .bind(&input.first_name)
    .bind(&input.last_name)
    .bind(&input.email)
    .bind(&input.phone)
    .bind(&input.job_title)
    .bind(input.company_id)
    .bind(owner_id)
    .fetch_one(&mut *tx)
    .await?;
    audit::record(
        &mut tx,
        &actor,
        EntityType::Contact,
        id,
        AuditAction::Created,
        audit::snapshot(&fields),
    )
    .await?;
    tx.commit().await?;
    let contact = fetch_contact(&state.pool, actor.org_id, id).await?;
    Ok((StatusCode::CREATED, Json(contact)))
}

pub async fn update_contact(
    State(state): State<AppState>,
    actor: AuthUser,
    ApiPath(id): ApiPath<Uuid>,
    ApiJson(req): ApiJson<ContactInput>,
) -> AppResult<Json<Contact>> {
    let (input, owner) = validate(req)?;

    let mut tx = state.pool.begin().await?;
    let before: ContactFields = sqlx::query_as(&format!(
        "SELECT {RETURNING_FIELDS} FROM contacts
         WHERE id = $1 AND organization_id = $2 FOR UPDATE"
    ))
    .bind(id)
    .bind(actor.org_id)
    .fetch_optional(&mut *tx)
    .await?
    .ok_or(AppError::NotFound("contact"))?;
    actor.require_can_modify(before.owner_id)?;
    let owner_id = match owner {
        Some(owner) if owner != before.owner_id => actor.resolve_owner(Some(owner))?,
        _ => before.owner_id,
    };

    let after: ContactFields = sqlx::query_as(&format!(
        "UPDATE contacts SET first_name = $3, last_name = $4, email = $5, phone = $6,
                job_title = $7, company_id = $8, owner_id = $9, updated_at = now()
         WHERE id = $1 AND organization_id = $2
         RETURNING {RETURNING_FIELDS}"
    ))
    .bind(id)
    .bind(actor.org_id)
    .bind(&input.first_name)
    .bind(&input.last_name)
    .bind(&input.email)
    .bind(&input.phone)
    .bind(&input.job_title)
    .bind(input.company_id)
    .bind(owner_id)
    .fetch_one(&mut *tx)
    .await?;
    let changes = audit::diff(&before, &after);
    if !audit::is_empty_diff(&changes) {
        audit::record(
            &mut tx,
            &actor,
            EntityType::Contact,
            id,
            AuditAction::Updated,
            changes,
        )
        .await?;
    }
    tx.commit().await?;
    Ok(Json(fetch_contact(&state.pool, actor.org_id, id).await?))
}

pub async fn delete_contact(
    State(state): State<AppState>,
    actor: AuthUser,
    ApiPath(id): ApiPath<Uuid>,
) -> AppResult<StatusCode> {
    actor.require_manager()?;
    let mut tx = state.pool.begin().await?;
    let deleted: Option<ContactFields> = sqlx::query_as(&format!(
        "DELETE FROM contacts WHERE id = $1 AND organization_id = $2 RETURNING {RETURNING_FIELDS}"
    ))
    .bind(id)
    .bind(actor.org_id)
    .fetch_optional(&mut *tx)
    .await?;
    let deleted = deleted.ok_or(AppError::NotFound("contact"))?;
    audit::record(
        &mut tx,
        &actor,
        EntityType::Contact,
        id,
        AuditAction::Deleted,
        audit::snapshot(&deleted),
    )
    .await?;
    tx.commit().await?;
    Ok(StatusCode::NO_CONTENT)
}
