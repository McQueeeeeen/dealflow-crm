use axum::extract::State;
use axum::http::StatusCode;
use axum::routing::{delete, get};
use axum::{Json, Router};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use uuid::Uuid;

use crate::auth::AuthUser;
use crate::error::{AppError, AppResult};
use crate::extract::{ApiJson, ApiPath, ApiQuery};
use crate::live;
use crate::pagination::{Page, Pagination};
use crate::state::AppState;
use crate::validation::Validator;

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/notes", get(list_notes).post(create_note))
        .route("/notes/{id}", delete(delete_note))
}

#[derive(Debug, Serialize, FromRow)]
pub struct Note {
    pub id: Uuid,
    pub body: String,
    pub author_id: Uuid,
    pub author_name: String,
    pub deal_id: Option<Uuid>,
    pub contact_id: Option<Uuid>,
    pub company_id: Option<Uuid>,
    pub created_at: DateTime<Utc>,
}

#[derive(Deserialize)]
pub struct NoteParent {
    pub deal_id: Option<Uuid>,
    pub contact_id: Option<Uuid>,
    pub company_id: Option<Uuid>,
}

impl NoteParent {
    fn require_single(&self) -> AppResult<()> {
        let set = [self.deal_id, self.contact_id, self.company_id]
            .iter()
            .filter(|id| id.is_some())
            .count();
        if set == 1 {
            Ok(())
        } else {
            Err(AppError::validation(
                "parent",
                "set exactly one of deal_id, contact_id or company_id",
            ))
        }
    }
}

#[derive(Deserialize)]
pub struct CreateNote {
    pub body: String,
    #[serde(flatten)]
    pub parent: NoteParent,
}

#[derive(Deserialize)]
pub struct NoteQuery {
    pub deal_id: Option<Uuid>,
    pub contact_id: Option<Uuid>,
    pub company_id: Option<Uuid>,
    pub page: Option<u32>,
    pub per_page: Option<u32>,
}

const SELECT_NOTE: &str = "
    SELECT n.id, n.body, n.author_id, u.full_name AS author_name,
           n.deal_id, n.contact_id, n.company_id, n.created_at
    FROM notes n JOIN users u ON u.id = n.author_id";

const FILTER: &str = "
    WHERE n.organization_id = $1
      AND n.deal_id IS NOT DISTINCT FROM $2
      AND n.contact_id IS NOT DISTINCT FROM $3
      AND n.company_id IS NOT DISTINCT FROM $4";

pub async fn list_notes(
    State(state): State<AppState>,
    user: AuthUser,
    ApiQuery(query): ApiQuery<NoteQuery>,
) -> AppResult<Json<Page<Note>>> {
    let parent = NoteParent {
        deal_id: query.deal_id,
        contact_id: query.contact_id,
        company_id: query.company_id,
    };
    parent.require_single()?;
    let pagination = Pagination::new(query.page, query.per_page);

    let total: i64 = sqlx::query_scalar(&format!("SELECT count(*) FROM notes n {FILTER}"))
        .bind(user.org_id)
        .bind(parent.deal_id)
        .bind(parent.contact_id)
        .bind(parent.company_id)
        .fetch_one(&state.pool)
        .await?;
    let notes = sqlx::query_as(&format!(
        "{SELECT_NOTE} {FILTER} ORDER BY n.created_at DESC, n.id DESC LIMIT $5 OFFSET $6"
    ))
    .bind(user.org_id)
    .bind(parent.deal_id)
    .bind(parent.contact_id)
    .bind(parent.company_id)
    .bind(pagination.limit())
    .bind(pagination.offset())
    .fetch_all(&state.pool)
    .await?;
    Ok(Json(pagination.wrap(notes, total)))
}

pub async fn create_note(
    State(state): State<AppState>,
    actor: AuthUser,
    ApiJson(req): ApiJson<CreateNote>,
) -> AppResult<(StatusCode, Json<Note>)> {
    req.parent.require_single()?;
    let body = req.body.trim();
    Validator::new().required("body", body, 10_000).finish()?;

    let id = Uuid::now_v7();
    sqlx::query(
        "INSERT INTO notes (id, organization_id, author_id, body, deal_id, contact_id, company_id)
         VALUES ($1, $2, $3, $4, $5, $6, $7)",
    )
    .bind(id)
    .bind(actor.org_id)
    .bind(actor.user_id)
    .bind(body)
    .bind(req.parent.deal_id)
    .bind(req.parent.contact_id)
    .bind(req.parent.company_id)
    .execute(&state.pool)
    .await?;
    let note = sqlx::query_as(&format!("{SELECT_NOTE} WHERE n.id = $1"))
        .bind(id)
        .fetch_one(&state.pool)
        .await?;
    announce(&state, &actor, id, "created").await?;
    Ok((StatusCode::CREATED, Json(note)))
}

async fn announce(state: &AppState, actor: &AuthUser, id: Uuid, action: &str) -> AppResult<()> {
    let change = live::Change {
        org: actor.org_id,
        entity: "note".into(),
        id,
        action: action.into(),
        actor: Some(actor.user_id),
    };
    live::notify(&mut *state.pool.acquire().await?, &change).await
}

pub async fn delete_note(
    State(state): State<AppState>,
    actor: AuthUser,
    ApiPath(id): ApiPath<Uuid>,
) -> AppResult<StatusCode> {
    let author: Uuid =
        sqlx::query_scalar("SELECT author_id FROM notes WHERE id = $1 AND organization_id = $2")
            .bind(id)
            .bind(actor.org_id)
            .fetch_optional(&state.pool)
            .await?
            .ok_or(AppError::NotFound("note"))?;
    actor.require_can_modify(author)?;
    sqlx::query("DELETE FROM notes WHERE id = $1 AND organization_id = $2")
        .bind(id)
        .bind(actor.org_id)
        .execute(&state.pool)
        .await?;
    announce(&state, &actor, id, "deleted").await?;
    Ok(StatusCode::NO_CONTENT)
}
