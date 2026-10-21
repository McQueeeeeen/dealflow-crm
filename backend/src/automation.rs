use chrono::{DateTime, NaiveDate, Utc};
use serde::{Deserialize, Serialize};
use sqlx::types::Json;
use sqlx::{Connection, FromRow, PgConnection};
use uuid::Uuid;

use crate::api::deals::{fetch_deal, Deal, SELECT_DEAL};
use crate::audit;
use crate::auth::AuthUser;
use crate::error::{AppError, AppResult};
use crate::models::EntityType;
use crate::pagination::Params;
use crate::{dql, live};

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Trigger {
    DealCreated,
    StageEntered { stage_id: Uuid },
    TaskCompleted,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Action {
    CreateTask {
        title: String,
        due_in_days: i32,
        assignee_id: Option<Uuid>,
    },
    AddNote {
        body: String,
    },
    AssignOwner {
        user_id: Uuid,
    },
    SetCloseDate {
        days_from_now: i32,
    },
}

#[derive(FromRow)]
struct Rule {
    id: Uuid,
    condition: String,
    actions: Json<Vec<Action>>,
}

#[derive(Serialize, FromRow)]
struct CreatedTask {
    title: String,
    due_at: Option<DateTime<Utc>>,
    assignee_id: Uuid,
    deal_id: Option<Uuid>,
}

#[derive(Serialize, FromRow)]
struct Ownership {
    owner_id: Uuid,
}

#[derive(Serialize, FromRow)]
struct CloseDate {
    expected_close_date: Option<NaiveDate>,
}

pub fn render(template: &str, deal: &Deal) -> String {
    let value = (deal.value_cents / 100).to_string();
    let grouped: String = value
        .chars()
        .rev()
        .collect::<Vec<_>>()
        .chunks(3)
        .map(|c| c.iter().collect::<String>())
        .collect::<Vec<_>>()
        .join(",")
        .chars()
        .rev()
        .collect();
    template
        .replace("{deal}", &deal.title)
        .replace(
            "{company}",
            deal.company_name.as_deref().unwrap_or("no company"),
        )
        .replace("{owner}", &deal.owner_name)
        .replace("{stage}", &deal.stage_name)
        .replace("{value}", &grouped)
}

pub async fn fire(
    conn: &mut PgConnection,
    actor: &AuthUser,
    trigger: Trigger,
    deal_id: Uuid,
) -> AppResult<()> {
    let rules: Vec<Rule> = sqlx::query_as(
        "SELECT id, condition, actions FROM automation_rules
         WHERE organization_id = $1 AND enabled AND trigger = $2
         ORDER BY created_at, id",
    )
    .bind(actor.org_id)
    .bind(Json(trigger))
    .fetch_all(&mut *conn)
    .await?;

    for rule in rules {
        let mut savepoint = conn.begin().await?;
        let outcome = apply(&mut savepoint, actor, &rule, deal_id).await;
        let (succeeded, detail) = match outcome {
            Ok(None) => continue,
            Ok(Some(done)) => {
                savepoint.commit().await?;
                (true, done.join("; "))
            }
            Err(err) => {
                savepoint.rollback().await?;
                (false, failure(&err))
            }
        };
        sqlx::query(
            "INSERT INTO automation_runs (organization_id, rule_id, deal_id, succeeded, detail)
             VALUES ($1, $2, $3, $4, $5)",
        )
        .bind(actor.org_id)
        .bind(rule.id)
        .bind(deal_id)
        .bind(succeeded)
        .bind(detail)
        .execute(&mut *conn)
        .await?;
    }
    Ok(())
}

fn failure(err: &AppError) -> String {
    match err {
        AppError::Database(_) | AppError::Internal(_) => "the database rejected the change".into(),
        AppError::Validation(fields) => fields
            .iter()
            .map(|f| format!("{} {}", f.field.replace('_', " "), f.message))
            .collect::<Vec<_>>()
            .join(", "),
        other => other.to_string(),
    }
}

pub async fn matches(
    conn: &mut PgConnection,
    actor: &AuthUser,
    condition: &str,
    deal_id: Uuid,
) -> AppResult<bool> {
    let filter = dql::parse(condition, Utc::now().date_naive())?;
    let params = Params::default().bind(actor.org_id)?.bind(deal_id)?;
    let (sql, params) = dql::compile(filter.as_ref(), params, actor.user_id)?;
    Ok(sqlx::query_scalar_with(
        &format!(
            "SELECT EXISTS (SELECT 1 FROM ({SELECT_DEAL}
                 WHERE d.organization_id = $1 AND d.id = $2) AS deal WHERE {sql})"
        ),
        params.arguments(),
    )
    .fetch_one(conn)
    .await?)
}

async fn apply(
    conn: &mut PgConnection,
    actor: &AuthUser,
    rule: &Rule,
    deal_id: Uuid,
) -> AppResult<Option<Vec<String>>> {
    if !matches(conn, actor, &rule.condition, deal_id).await? {
        return Ok(None);
    }
    let mut done = Vec::new();
    for action in &rule.actions.0 {
        let deal = fetch_deal(conn, actor.org_id, deal_id).await?;
        done.push(run(conn, actor, action, &deal).await?);
    }
    Ok(Some(done))
}

async fn run(
    conn: &mut PgConnection,
    actor: &AuthUser,
    action: &Action,
    deal: &Deal,
) -> AppResult<String> {
    match action {
        Action::CreateTask {
            title,
            due_in_days,
            assignee_id,
        } => {
            let id = Uuid::now_v7();
            let task: CreatedTask = sqlx::query_as(
                "INSERT INTO tasks (id, organization_id, title, due_at, assignee_id, created_by, deal_id)
                 VALUES ($1, $2, $3, now() + make_interval(days => $4), $5, $6, $7)
                 RETURNING title, due_at, assignee_id, deal_id",
            )
            .bind(id)
            .bind(actor.org_id)
            .bind(render(title, deal))
            .bind(due_in_days)
            .bind(assignee_id.unwrap_or(deal.owner_id))
            .bind(actor.user_id)
            .bind(deal.id)
            .fetch_one(&mut *conn)
            .await?;
            audit::created(conn, actor, EntityType::Task, id, &task).await?;
            Ok(format!("created the follow-up “{}”", task.title))
        }
        Action::AddNote { body } => {
            let id = Uuid::now_v7();
            sqlx::query(
                "INSERT INTO notes (id, organization_id, author_id, body, deal_id)
                 VALUES ($1, $2, $3, $4, $5)",
            )
            .bind(id)
            .bind(actor.org_id)
            .bind(actor.user_id)
            .bind(render(body, deal))
            .bind(deal.id)
            .execute(&mut *conn)
            .await?;
            let change = live::Change {
                org: actor.org_id,
                entity: "note".into(),
                id,
                action: "created".into(),
                actor: Some(actor.user_id),
            };
            live::notify(conn, &change).await?;
            Ok("added a note".into())
        }
        Action::AssignOwner { user_id } => {
            let after: Ownership = sqlx::query_as(
                "UPDATE deals SET owner_id = $3, version = version + 1, updated_at = now()
                 WHERE id = $1 AND organization_id = $2
                   AND EXISTS (SELECT 1 FROM users u
                               WHERE u.id = $3 AND u.organization_id = $2 AND u.is_active)
                 RETURNING owner_id",
            )
            .bind(deal.id)
            .bind(actor.org_id)
            .bind(user_id)
            .fetch_optional(&mut *conn)
            .await?
            .ok_or(AppError::NotFound("active user"))?;
            let before = Ownership {
                owner_id: deal.owner_id,
            };
            audit::updated(conn, actor, EntityType::Deal, deal.id, &before, &after).await?;
            Ok("reassigned the deal".into())
        }
        Action::SetCloseDate { days_from_now } => {
            let after: CloseDate = sqlx::query_as(
                "UPDATE deals SET expected_close_date = current_date + $3::int,
                        version = version + 1, updated_at = now()
                 WHERE id = $1 AND organization_id = $2
                 RETURNING expected_close_date",
            )
            .bind(deal.id)
            .bind(actor.org_id)
            .bind(days_from_now)
            .fetch_one(&mut *conn)
            .await?;
            let before = CloseDate {
                expected_close_date: deal.expected_close_date,
            };
            audit::updated(conn, actor, EntityType::Deal, deal.id, &before, &after).await?;
            Ok(format!(
                "set the expected close to {}",
                after
                    .expected_close_date
                    .map_or_else(String::new, |d| d.to_string())
            ))
        }
    }
}
