use axum::extract::State;
use axum::http::StatusCode;
use axum::routing::{get, post};
use axum::{Json, Router};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::{FromRow, PgConnection};
use utoipa::{IntoParams, ToSchema};
use uuid::Uuid;

use crate::audit;
use crate::auth::AuthUser;
use crate::error::{AppError, AppResult};
use crate::extract::{ApiJson, ApiPath, ApiQuery};
use crate::models::{AuditAction, EntityType};
use crate::pagination::{Page, Pagination};
use crate::state::AppState;
use crate::validation::{clean, Validator};

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/tasks", get(list_tasks).post(create_task))
        .route(
            "/tasks/{id}",
            get(get_task).put(update_task).delete(delete_task),
        )
        .route("/tasks/{id}/complete", post(complete_task))
        .route("/tasks/{id}/reopen", post(reopen_task))
}

#[derive(Debug, Serialize, ToSchema, FromRow)]
pub struct Task {
    pub id: Uuid,
    pub title: String,
    pub details: Option<String>,
    pub due_at: Option<DateTime<Utc>>,
    pub completed_at: Option<DateTime<Utc>>,
    pub assignee_id: Uuid,
    pub assignee_name: String,
    pub created_by: Uuid,
    pub deal_id: Option<Uuid>,
    pub deal_title: Option<String>,
    pub contact_id: Option<Uuid>,
    pub contact_name: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Serialize, FromRow)]
struct TaskFields {
    title: String,
    details: Option<String>,
    due_at: Option<DateTime<Utc>>,
    completed_at: Option<DateTime<Utc>>,
    assignee_id: Uuid,
    created_by: Uuid,
    deal_id: Option<Uuid>,
    contact_id: Option<Uuid>,
}

#[derive(Deserialize, ToSchema)]
pub struct TaskInput {
    pub title: String,
    pub details: Option<String>,
    pub due_at: Option<DateTime<Utc>>,
    /// Defaults to the caller. Anyone can hand a task to a teammate.
    pub assignee_id: Option<Uuid>,
    pub deal_id: Option<Uuid>,
    pub contact_id: Option<Uuid>,
}

#[derive(Debug, Clone, Copy, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum TaskStatus {
    Open,
    Completed,
    /// Open and past its due time.
    Overdue,
}

#[derive(Deserialize, IntoParams)]
#[into_params(parameter_in = Query)]
pub struct TaskQuery {
    pub assignee_id: Option<Uuid>,
    pub deal_id: Option<Uuid>,
    pub contact_id: Option<Uuid>,
    pub status: Option<TaskStatus>,
    /// Only tasks due before this instant.
    pub due_before: Option<DateTime<Utc>>,
    pub page: Option<u32>,
    pub per_page: Option<u32>,
}

const SELECT_TASK: &str = "
    SELECT t.id, t.title, t.details, t.due_at, t.completed_at, t.assignee_id,
           u.full_name AS assignee_name, t.created_by, t.deal_id, d.title AS deal_title,
           t.contact_id, trim(ct.first_name || ' ' || ct.last_name) AS contact_name,
           t.created_at, t.updated_at
    FROM tasks t
    JOIN users u ON u.id = t.assignee_id
    LEFT JOIN deals d ON d.id = t.deal_id
    LEFT JOIN contacts ct ON ct.id = t.contact_id";

const FILTER: &str = "
    WHERE t.organization_id = $1
      AND ($2::uuid IS NULL OR t.assignee_id = $2)
      AND ($3::uuid IS NULL OR t.deal_id = $3)
      AND ($4::uuid IS NULL OR t.contact_id = $4)
      AND ($5::text IS NULL
           OR ($5 = 'open' AND t.completed_at IS NULL)
           OR ($5 = 'completed' AND t.completed_at IS NOT NULL)
           OR ($5 = 'overdue' AND t.completed_at IS NULL AND t.due_at < now()))
      AND ($6::timestamptz IS NULL OR t.due_at < $6)";

const FIELD_COLUMNS: &str =
    "title, details, due_at, completed_at, assignee_id, created_by, deal_id, contact_id";

fn status_filter(status: Option<TaskStatus>) -> Option<&'static str> {
    status.map(|status| match status {
        TaskStatus::Open => "open",
        TaskStatus::Completed => "completed",
        TaskStatus::Overdue => "overdue",
    })
}

#[utoipa::path(
    get, path = "/api/v1/tasks", tag = "tasks",
    params(TaskQuery),
    responses((status = 200, description = "Open tasks first, soonest due first", body = Page<Task>)),
    security(("bearer" = []))
)]
pub async fn list_tasks(
    State(state): State<AppState>,
    user: AuthUser,
    ApiQuery(query): ApiQuery<TaskQuery>,
) -> AppResult<Json<Page<Task>>> {
    let pagination = Pagination::new(query.page, query.per_page);
    let status = status_filter(query.status);

    let total: i64 = sqlx::query_scalar(&format!("SELECT count(*) FROM tasks t {FILTER}"))
        .bind(user.org_id)
        .bind(query.assignee_id)
        .bind(query.deal_id)
        .bind(query.contact_id)
        .bind(status)
        .bind(query.due_before)
        .fetch_one(&state.pool)
        .await?;
    let tasks = sqlx::query_as(&format!(
        "{SELECT_TASK} {FILTER}
         ORDER BY t.completed_at IS NOT NULL, t.due_at NULLS LAST, t.created_at DESC, t.id
         LIMIT $7 OFFSET $8"
    ))
    .bind(user.org_id)
    .bind(query.assignee_id)
    .bind(query.deal_id)
    .bind(query.contact_id)
    .bind(status)
    .bind(query.due_before)
    .bind(pagination.limit())
    .bind(pagination.offset())
    .fetch_all(&state.pool)
    .await?;
    Ok(Json(pagination.wrap(tasks, total)))
}

async fn fetch_task(conn: &mut PgConnection, org_id: Uuid, id: Uuid) -> AppResult<Task> {
    sqlx::query_as(&format!(
        "{SELECT_TASK} WHERE t.organization_id = $1 AND t.id = $2"
    ))
    .bind(org_id)
    .bind(id)
    .fetch_optional(conn)
    .await?
    .ok_or(AppError::NotFound("task"))
}

#[utoipa::path(
    get, path = "/api/v1/tasks/{id}", tag = "tasks",
    params(("id" = Uuid, Path)),
    responses((status = 200, body = Task), (status = 404, body = crate::error::ErrorBody)),
    security(("bearer" = []))
)]
pub async fn get_task(
    State(state): State<AppState>,
    user: AuthUser,
    ApiPath(id): ApiPath<Uuid>,
) -> AppResult<Json<Task>> {
    let mut conn = state.pool.acquire().await?;
    Ok(Json(fetch_task(&mut conn, user.org_id, id).await?))
}

fn validate(req: &TaskInput) -> AppResult<(String, Option<String>)> {
    let title = req.title.trim().to_owned();
    let details = clean(req.details.clone());
    Validator::new()
        .required("title", &title, 200)
        .optional("details", details.as_deref(), 5000)
        .finish()?;
    Ok((title, details))
}

/// Reps may change a task when it is assigned to them or they created it.
fn require_task_access(actor: &AuthUser, task: &TaskFields) -> AppResult<()> {
    if actor.can_modify(task.assignee_id) || task.created_by == actor.user_id {
        Ok(())
    } else {
        Err(AppError::Forbidden)
    }
}

async fn lock_task(conn: &mut PgConnection, actor: &AuthUser, id: Uuid) -> AppResult<TaskFields> {
    let task: TaskFields = sqlx::query_as(&format!(
        "SELECT {FIELD_COLUMNS} FROM tasks WHERE id = $1 AND organization_id = $2 FOR UPDATE"
    ))
    .bind(id)
    .bind(actor.org_id)
    .fetch_optional(conn)
    .await?
    .ok_or(AppError::NotFound("task"))?;
    require_task_access(actor, &task)?;
    Ok(task)
}

#[utoipa::path(
    post, path = "/api/v1/tasks", tag = "tasks",
    request_body = TaskInput,
    responses((status = 201, body = Task), (status = 422, body = crate::error::ErrorBody)),
    security(("bearer" = []))
)]
pub async fn create_task(
    State(state): State<AppState>,
    actor: AuthUser,
    ApiJson(req): ApiJson<TaskInput>,
) -> AppResult<(StatusCode, Json<Task>)> {
    let (title, details) = validate(&req)?;
    let id = Uuid::now_v7();

    let mut tx = state.pool.begin().await?;
    let fields: TaskFields = sqlx::query_as(&format!(
        "INSERT INTO tasks (id, organization_id, title, details, due_at, assignee_id, created_by,
                            deal_id, contact_id)
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)
         RETURNING {FIELD_COLUMNS}"
    ))
    .bind(id)
    .bind(actor.org_id)
    .bind(&title)
    .bind(&details)
    .bind(req.due_at)
    .bind(req.assignee_id.unwrap_or(actor.user_id))
    .bind(actor.user_id)
    .bind(req.deal_id)
    .bind(req.contact_id)
    .fetch_one(&mut *tx)
    .await?;
    audit::record(
        &mut tx,
        &actor,
        EntityType::Task,
        id,
        AuditAction::Created,
        audit::snapshot(&fields),
    )
    .await?;
    let task = fetch_task(&mut tx, actor.org_id, id).await?;
    tx.commit().await?;
    Ok((StatusCode::CREATED, Json(task)))
}

#[utoipa::path(
    put, path = "/api/v1/tasks/{id}", tag = "tasks",
    params(("id" = Uuid, Path)),
    request_body = TaskInput,
    responses(
        (status = 200, body = Task),
        (status = 403, body = crate::error::ErrorBody),
        (status = 404, body = crate::error::ErrorBody),
    ),
    security(("bearer" = []))
)]
pub async fn update_task(
    State(state): State<AppState>,
    actor: AuthUser,
    ApiPath(id): ApiPath<Uuid>,
    ApiJson(req): ApiJson<TaskInput>,
) -> AppResult<Json<Task>> {
    let (title, details) = validate(&req)?;
    let mut tx = state.pool.begin().await?;
    let before = lock_task(&mut tx, &actor, id).await?;

    let after: TaskFields = sqlx::query_as(&format!(
        "UPDATE tasks SET title = $3, details = $4, due_at = $5, assignee_id = $6,
                deal_id = $7, contact_id = $8, updated_at = now()
         WHERE id = $1 AND organization_id = $2
         RETURNING {FIELD_COLUMNS}"
    ))
    .bind(id)
    .bind(actor.org_id)
    .bind(&title)
    .bind(&details)
    .bind(req.due_at)
    .bind(req.assignee_id.unwrap_or(before.assignee_id))
    .bind(req.deal_id)
    .bind(req.contact_id)
    .fetch_one(&mut *tx)
    .await?;
    let changes = audit::diff(&before, &after);
    if !audit::is_empty_diff(&changes) {
        audit::record(
            &mut tx,
            &actor,
            EntityType::Task,
            id,
            AuditAction::Updated,
            changes,
        )
        .await?;
    }
    let task = fetch_task(&mut tx, actor.org_id, id).await?;
    tx.commit().await?;
    Ok(Json(task))
}

async fn set_completion(
    state: &AppState,
    actor: &AuthUser,
    id: Uuid,
    completed: bool,
) -> AppResult<Task> {
    let mut tx = state.pool.begin().await?;
    let before = lock_task(&mut tx, actor, id).await?;
    if before.completed_at.is_some() != completed {
        let after: TaskFields = sqlx::query_as(&format!(
            "UPDATE tasks
             SET completed_at = CASE WHEN $3 THEN now() END, updated_at = now()
             WHERE id = $1 AND organization_id = $2
             RETURNING {FIELD_COLUMNS}"
        ))
        .bind(id)
        .bind(actor.org_id)
        .bind(completed)
        .fetch_one(&mut *tx)
        .await?;
        let changes = audit::diff(&before, &after);
        audit::record(
            &mut tx,
            actor,
            EntityType::Task,
            id,
            AuditAction::Updated,
            changes,
        )
        .await?;
    }
    let task = fetch_task(&mut tx, actor.org_id, id).await?;
    tx.commit().await?;
    Ok(task)
}

#[utoipa::path(
    post, path = "/api/v1/tasks/{id}/complete", tag = "tasks",
    params(("id" = Uuid, Path)),
    responses((status = 200, description = "Idempotent", body = Task)),
    security(("bearer" = []))
)]
pub async fn complete_task(
    State(state): State<AppState>,
    actor: AuthUser,
    ApiPath(id): ApiPath<Uuid>,
) -> AppResult<Json<Task>> {
    Ok(Json(set_completion(&state, &actor, id, true).await?))
}

#[utoipa::path(
    post, path = "/api/v1/tasks/{id}/reopen", tag = "tasks",
    params(("id" = Uuid, Path)),
    responses((status = 200, description = "Idempotent", body = Task)),
    security(("bearer" = []))
)]
pub async fn reopen_task(
    State(state): State<AppState>,
    actor: AuthUser,
    ApiPath(id): ApiPath<Uuid>,
) -> AppResult<Json<Task>> {
    Ok(Json(set_completion(&state, &actor, id, false).await?))
}

#[utoipa::path(
    delete, path = "/api/v1/tasks/{id}", tag = "tasks",
    params(("id" = Uuid, Path)),
    responses((status = 204), (status = 403, body = crate::error::ErrorBody)),
    security(("bearer" = []))
)]
pub async fn delete_task(
    State(state): State<AppState>,
    actor: AuthUser,
    ApiPath(id): ApiPath<Uuid>,
) -> AppResult<StatusCode> {
    let mut tx = state.pool.begin().await?;
    let task = lock_task(&mut tx, &actor, id).await?;
    sqlx::query("DELETE FROM tasks WHERE id = $1 AND organization_id = $2")
        .bind(id)
        .bind(actor.org_id)
        .execute(&mut *tx)
        .await?;
    audit::record(
        &mut tx,
        &actor,
        EntityType::Task,
        id,
        AuditAction::Deleted,
        audit::snapshot(&task),
    )
    .await?;
    tx.commit().await?;
    Ok(StatusCode::NO_CONTENT)
}
