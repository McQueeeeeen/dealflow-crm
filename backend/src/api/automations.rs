use axum::extract::State;
use axum::http::StatusCode;
use axum::routing::{get, put};
use axum::{Json, Router};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::types::Json as Jsonb;
use sqlx::{FromRow, PgConnection};
use uuid::Uuid;

use crate::audit;
use crate::auth::AuthUser;
use crate::automation::{Action, Trigger};
use crate::dql;
use crate::error::{AppError, AppResult};
use crate::extract::{ApiJson, ApiPath, ApiQuery};
use crate::models::EntityType;
use crate::state::AppState;
use crate::validation::Validator;

const MAX_ACTIONS: usize = 5;

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/automations", get(list_rules).post(create_rule))
        .route("/automations/runs", get(list_runs))
        .route("/automations/{id}", put(update_rule).delete(delete_rule))
}

#[derive(Debug, Serialize, FromRow)]
pub struct RuleView {
    pub id: Uuid,
    pub name: String,
    pub enabled: bool,
    pub trigger: Jsonb<Trigger>,
    pub condition: String,
    pub actions: Jsonb<Vec<Action>>,
    pub created_at: DateTime<Utc>,
    pub runs_7d: i64,
    pub failures_7d: i64,
    pub last_run_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Serialize, FromRow)]
pub struct RunView {
    pub id: i64,
    pub rule_id: Uuid,
    pub rule_name: String,
    pub deal_id: Uuid,
    pub deal_title: String,
    pub succeeded: bool,
    pub detail: String,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Serialize, Deserialize, FromRow)]
pub struct RuleInput {
    pub name: String,
    #[serde(default = "enabled_by_default")]
    pub enabled: bool,
    pub trigger: Jsonb<Trigger>,
    #[serde(default)]
    pub condition: String,
    pub actions: Jsonb<Vec<Action>>,
}

#[derive(Deserialize)]
pub struct RunQuery {
    pub rule_id: Option<Uuid>,
}

fn enabled_by_default() -> bool {
    true
}

const SELECT_RULE: &str = "
    SELECT r.id, r.name, r.enabled, r.trigger, r.condition, r.actions, r.created_at,
           count(x.id) FILTER (WHERE x.created_at > now() - interval '7 days') AS runs_7d,
           count(x.id) FILTER (WHERE x.created_at > now() - interval '7 days' AND NOT x.succeeded)
               AS failures_7d,
           max(x.created_at) AS last_run_at
    FROM automation_rules r
    LEFT JOIN automation_runs x ON x.organization_id = r.organization_id AND x.rule_id = r.id";

const RULE_COLUMNS: &str = "name, enabled, trigger, condition, actions";

async fn fetch_rule(conn: &mut PgConnection, org_id: Uuid, id: Uuid) -> AppResult<RuleView> {
    sqlx::query_as(&format!(
        "{SELECT_RULE} WHERE r.organization_id = $1 AND r.id = $2 GROUP BY r.id"
    ))
    .bind(org_id)
    .bind(id)
    .fetch_optional(conn)
    .await?
    .ok_or(AppError::NotFound("automation"))
}

async fn active_user(conn: &mut PgConnection, org_id: Uuid, id: Uuid) -> AppResult<bool> {
    Ok(sqlx::query_scalar(
        "SELECT EXISTS (SELECT 1 FROM users WHERE id = $1 AND organization_id = $2 AND is_active)",
    )
    .bind(id)
    .bind(org_id)
    .fetch_one(conn)
    .await?)
}

async fn validate(
    conn: &mut PgConnection,
    actor: &AuthUser,
    input: &mut RuleInput,
) -> AppResult<()> {
    input.name = input.name.trim().to_owned();
    input.condition = input.condition.trim().to_owned();
    let actions = &input.actions.0;
    let mut v = Validator::new();
    v.required("name", &input.name, 80)
        .optional("condition", Some(&input.condition), 500)
        .check(
            (1..=MAX_ACTIONS).contains(&actions.len()),
            "actions",
            "add between one and five actions",
        );
    for action in actions {
        match action {
            Action::CreateTask {
                title,
                due_in_days,
                assignee_id,
            } => {
                v.required("actions", title.trim(), 200).check(
                    (0..=365).contains(due_in_days),
                    "actions",
                    "follow-ups are due within a year",
                );
                if let Some(id) = assignee_id {
                    let ok = active_user(conn, actor.org_id, *id).await?;
                    v.check(
                        ok,
                        "actions",
                        "the follow-up goes to someone who is not on the team",
                    );
                }
            }
            Action::AddNote { body } => {
                v.required("actions", body.trim(), 2000);
            }
            Action::AssignOwner { user_id } => {
                let ok = active_user(conn, actor.org_id, *user_id).await?;
                v.check(ok, "actions", "the new owner is not on the team");
            }
            Action::SetCloseDate { days_from_now } => {
                v.check(
                    (0..=730).contains(days_from_now),
                    "actions",
                    "the close date is within two years",
                );
            }
        }
    }
    if let Trigger::StageEntered { stage_id } = input.trigger.0 {
        let ok: bool = sqlx::query_scalar(
            "SELECT EXISTS (SELECT 1 FROM pipeline_stages WHERE id = $1 AND organization_id = $2)",
        )
        .bind(stage_id)
        .bind(actor.org_id)
        .fetch_one(&mut *conn)
        .await?;
        v.check(ok, "trigger", "that stage does not exist");
    }
    v.finish()?;
    dql::parse(&input.condition, Utc::now().date_naive())?;
    Ok(())
}

pub async fn list_rules(
    State(state): State<AppState>,
    user: AuthUser,
) -> AppResult<Json<Vec<RuleView>>> {
    let rules = sqlx::query_as(&format!(
        "{SELECT_RULE} WHERE r.organization_id = $1 GROUP BY r.id ORDER BY r.created_at, r.id"
    ))
    .bind(user.org_id)
    .fetch_all(&state.pool)
    .await?;
    Ok(Json(rules))
}

pub async fn create_rule(
    State(state): State<AppState>,
    actor: AuthUser,
    ApiJson(mut input): ApiJson<RuleInput>,
) -> AppResult<(StatusCode, Json<RuleView>)> {
    actor.require_manager()?;
    let mut tx = state.pool.begin().await?;
    validate(&mut tx, &actor, &mut input).await?;
    let id = Uuid::now_v7();
    let saved: RuleInput = sqlx::query_as(&format!(
        "INSERT INTO automation_rules (id, organization_id, name, enabled, trigger, condition, actions, created_by)
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8) RETURNING {RULE_COLUMNS}"
    ))
    .bind(id)
    .bind(actor.org_id)
    .bind(&input.name)
    .bind(input.enabled)
    .bind(input.trigger)
    .bind(&input.condition)
    .bind(&input.actions)
    .bind(actor.user_id)
    .fetch_one(&mut *tx)
    .await?;
    audit::created(&mut tx, &actor, EntityType::Rule, id, &saved).await?;
    let rule = fetch_rule(&mut tx, actor.org_id, id).await?;
    tx.commit().await?;
    Ok((StatusCode::CREATED, Json(rule)))
}

pub async fn update_rule(
    State(state): State<AppState>,
    actor: AuthUser,
    ApiPath(id): ApiPath<Uuid>,
    ApiJson(mut input): ApiJson<RuleInput>,
) -> AppResult<Json<RuleView>> {
    actor.require_manager()?;
    let mut tx = state.pool.begin().await?;
    validate(&mut tx, &actor, &mut input).await?;
    let before: RuleInput = sqlx::query_as(&format!(
        "SELECT {RULE_COLUMNS} FROM automation_rules
         WHERE id = $1 AND organization_id = $2 FOR UPDATE"
    ))
    .bind(id)
    .bind(actor.org_id)
    .fetch_optional(&mut *tx)
    .await?
    .ok_or(AppError::NotFound("automation"))?;
    let after: RuleInput = sqlx::query_as(&format!(
        "UPDATE automation_rules
         SET name = $3, enabled = $4, trigger = $5, condition = $6, actions = $7, updated_at = now()
         WHERE id = $1 AND organization_id = $2 RETURNING {RULE_COLUMNS}"
    ))
    .bind(id)
    .bind(actor.org_id)
    .bind(&input.name)
    .bind(input.enabled)
    .bind(input.trigger)
    .bind(&input.condition)
    .bind(&input.actions)
    .fetch_one(&mut *tx)
    .await?;
    audit::updated(&mut tx, &actor, EntityType::Rule, id, &before, &after).await?;
    let rule = fetch_rule(&mut tx, actor.org_id, id).await?;
    tx.commit().await?;
    Ok(Json(rule))
}

pub async fn delete_rule(
    State(state): State<AppState>,
    actor: AuthUser,
    ApiPath(id): ApiPath<Uuid>,
) -> AppResult<StatusCode> {
    actor.require_manager()?;
    let mut tx = state.pool.begin().await?;
    let deleted: RuleInput = sqlx::query_as(&format!(
        "DELETE FROM automation_rules WHERE id = $1 AND organization_id = $2 RETURNING {RULE_COLUMNS}"
    ))
    .bind(id)
    .bind(actor.org_id)
    .fetch_optional(&mut *tx)
    .await?
    .ok_or(AppError::NotFound("automation"))?;
    audit::deleted(&mut tx, &actor, EntityType::Rule, id, &deleted).await?;
    tx.commit().await?;
    Ok(StatusCode::NO_CONTENT)
}

pub async fn list_runs(
    State(state): State<AppState>,
    user: AuthUser,
    ApiQuery(query): ApiQuery<RunQuery>,
) -> AppResult<Json<Vec<RunView>>> {
    let runs = sqlx::query_as(
        "SELECT x.id, x.rule_id, r.name AS rule_name, x.deal_id, d.title AS deal_title,
                x.succeeded, x.detail, x.created_at
         FROM automation_runs x
         JOIN automation_rules r ON r.id = x.rule_id
         JOIN deals d ON d.id = x.deal_id
         WHERE x.organization_id = $1 AND ($2::uuid IS NULL OR x.rule_id = $2)
         ORDER BY x.created_at DESC, x.id DESC
         LIMIT 50",
    )
    .bind(user.org_id)
    .bind(query.rule_id)
    .fetch_all(&state.pool)
    .await?;
    Ok(Json(runs))
}
