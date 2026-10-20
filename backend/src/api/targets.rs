use axum::extract::State;
use axum::http::StatusCode;
use axum::routing::{get, put};
use axum::{Json, Router};
use chrono::{Datelike, NaiveDate, Utc};
use serde::{Deserialize, Serialize};
use serde_json::json;
use sqlx::FromRow;
use uuid::Uuid;

use crate::audit;
use crate::auth::AuthUser;
use crate::error::{AppError, AppResult};
use crate::extract::{ApiJson, ApiPath, ApiQuery};
use crate::models::{AuditAction, EntityType};
use crate::state::AppState;

const MAX_TARGET_CENTS: i64 = 1_000_000_000_000_000;

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/targets", get(list_targets))
        .route("/targets/{user_id}", put(set_target))
}

#[derive(Deserialize)]
pub struct MonthQuery {
    pub month: Option<NaiveDate>,
}

#[derive(Debug, Serialize, FromRow)]
pub struct TargetRow {
    pub user_id: Uuid,
    pub full_name: String,
    pub target_cents: Option<i64>,
    pub won_cents: i64,
    pub weighted_cents: i64,
}

#[derive(Serialize)]
pub struct Targets {
    pub month: NaiveDate,
    pub rows: Vec<TargetRow>,
}

#[derive(Deserialize)]
pub struct SetTarget {
    pub month: NaiveDate,
    pub amount_cents: Option<i64>,
}

fn month_start(date: NaiveDate) -> NaiveDate {
    date.with_day(1).unwrap_or(date)
}

pub async fn list_targets(
    State(state): State<AppState>,
    user: AuthUser,
    ApiQuery(query): ApiQuery<MonthQuery>,
) -> AppResult<Json<Targets>> {
    let month = month_start(query.month.unwrap_or_else(|| Utc::now().date_naive()));
    let rows = sqlx::query_as(
        "SELECT u.id AS user_id, u.full_name, t.amount_cents AS target_cents,
                coalesce((SELECT sum(d.value_cents) FROM deals d
                          JOIN pipeline_stages s ON s.id = d.stage_id
                          WHERE d.organization_id = $1 AND d.owner_id = u.id AND s.kind = 'won'
                            AND d.closed_at >= $2 AND d.closed_at < $2 + interval '1 month'), 0)::bigint
                    AS won_cents,
                coalesce((SELECT sum(d.value_cents * s.probability / 100) FROM deals d
                          JOIN pipeline_stages s ON s.id = d.stage_id
                          WHERE d.organization_id = $1 AND d.owner_id = u.id AND s.kind = 'open'
                            AND d.expected_close_date >= $2
                            AND d.expected_close_date < $2 + interval '1 month'), 0)::bigint
                    AS weighted_cents
         FROM users u
         LEFT JOIN sales_targets t
                ON t.organization_id = u.organization_id AND t.user_id = u.id AND t.month = $2
         WHERE u.organization_id = $1 AND u.is_active
         ORDER BY u.full_name, u.id",
    )
    .bind(user.org_id)
    .bind(month)
    .fetch_all(&state.pool)
    .await?;
    Ok(Json(Targets { month, rows }))
}

pub async fn set_target(
    State(state): State<AppState>,
    actor: AuthUser,
    ApiPath(user_id): ApiPath<Uuid>,
    ApiJson(req): ApiJson<SetTarget>,
) -> AppResult<StatusCode> {
    actor.require_manager()?;
    if req
        .amount_cents
        .is_some_and(|a| !(0..=MAX_TARGET_CENTS).contains(&a))
    {
        return Err(AppError::validation(
            "amount_cents",
            "must be zero or a positive amount",
        ));
    }
    let month = month_start(req.month);

    let mut tx = state.pool.begin().await?;
    let exists: bool = sqlx::query_scalar(
        "SELECT EXISTS (SELECT 1 FROM users WHERE id = $1 AND organization_id = $2)",
    )
    .bind(user_id)
    .bind(actor.org_id)
    .fetch_one(&mut *tx)
    .await?;
    if !exists {
        return Err(AppError::NotFound("user"));
    }
    let before: Option<i64> = sqlx::query_scalar(
        "SELECT amount_cents FROM sales_targets
         WHERE organization_id = $1 AND user_id = $2 AND month = $3 FOR UPDATE",
    )
    .bind(actor.org_id)
    .bind(user_id)
    .bind(month)
    .fetch_optional(&mut *tx)
    .await?;
    if before == req.amount_cents {
        return Ok(StatusCode::NO_CONTENT);
    }
    match req.amount_cents {
        Some(amount) => {
            sqlx::query(
                "INSERT INTO sales_targets (organization_id, user_id, month, amount_cents)
                 VALUES ($1, $2, $3, $4)
                 ON CONFLICT (organization_id, user_id, month)
                 DO UPDATE SET amount_cents = EXCLUDED.amount_cents, updated_at = now()",
            )
            .bind(actor.org_id)
            .bind(user_id)
            .bind(month)
            .bind(amount)
            .execute(&mut *tx)
            .await?;
        }
        None => {
            sqlx::query(
                "DELETE FROM sales_targets WHERE organization_id = $1 AND user_id = $2 AND month = $3",
            )
            .bind(actor.org_id)
            .bind(user_id)
            .bind(month)
            .execute(&mut *tx)
            .await?;
        }
    }
    let action = match (before, req.amount_cents) {
        (None, _) => AuditAction::Created,
        (_, None) => AuditAction::Deleted,
        _ => AuditAction::Updated,
    };
    audit::record(
        &mut tx,
        &actor,
        EntityType::Target,
        user_id,
        action,
        json!({ "month": month, "amount_cents": { "from": before, "to": req.amount_cents } }),
    )
    .await?;
    tx.commit().await?;
    Ok(StatusCode::NO_CONTENT)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn months_start_on_the_first() {
        let date = NaiveDate::from_ymd_opt(2026, 10, 20).unwrap();
        assert_eq!(month_start(date), NaiveDate::from_ymd_opt(2026, 10, 1).unwrap());
    }
}
