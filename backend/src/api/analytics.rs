use axum::extract::State;
use axum::routing::get;
use axum::{Json, Router};
use chrono::{DateTime, Days, NaiveDate, NaiveTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use uuid::Uuid;

use crate::auth::AuthUser;
use crate::error::{AppError, AppResult};
use crate::extract::ApiQuery;
use crate::models::StageKind;
use crate::state::AppState;

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/analytics/summary", get(summary))
        .route("/analytics/funnel", get(funnel))
        .route("/analytics/forecast", get(forecast))
        .route("/analytics/leaderboard", get(leaderboard))
}

#[derive(Deserialize)]
pub struct OwnerQuery {
    pub owner_id: Option<Uuid>,
}

#[derive(Deserialize)]
pub struct PeriodQuery {
    pub from: Option<NaiveDate>,
    pub owner_id: Option<Uuid>,
}

#[derive(Deserialize)]
pub struct ForecastQuery {
    pub months: Option<i32>,
    pub owner_id: Option<Uuid>,
}

#[derive(Serialize, FromRow)]
pub struct Summary {
    pub open_deals_count: i64,
    pub open_pipeline_cents: i64,
    pub weighted_pipeline_cents: i64,
    pub won_this_month_cents: i64,
    pub won_this_month_count: i64,
    pub win_rate_90d: Option<f64>,
    pub avg_days_to_win: Option<f64>,
}

#[derive(Serialize)]
pub struct Funnel {
    pub from: NaiveDate,
    pub cohort_size: i64,
    pub stages: Vec<FunnelStage>,
}

#[derive(Serialize)]
pub struct FunnelStage {
    pub stage_id: Uuid,
    pub name: String,
    pub kind: StageKind,
    pub reached: i64,
    pub conversion_from_previous: Option<f64>,
}

#[derive(FromRow)]
struct FunnelRow {
    stage_id: Uuid,
    name: String,
    kind: StageKind,
    reached: i64,
}

#[derive(Serialize)]
pub struct Forecast {
    pub months: Vec<ForecastMonth>,
    pub overdue_cents: i64,
    pub undated_cents: i64,
}

#[derive(Serialize, FromRow)]
pub struct ForecastMonth {
    pub month: NaiveDate,
    pub deals: i64,
    pub total_cents: i64,
    pub weighted_cents: i64,
}

#[derive(Serialize, FromRow)]
pub struct LeaderboardRow {
    pub user_id: Uuid,
    pub full_name: String,
    pub won_count: i64,
    pub won_cents: i64,
    pub lost_count: i64,
    pub open_pipeline_cents: i64,
    pub win_rate: Option<f64>,
}

pub async fn summary(
    State(state): State<AppState>,
    user: AuthUser,
    ApiQuery(query): ApiQuery<OwnerQuery>,
) -> AppResult<Json<Summary>> {
    let summary = sqlx::query_as(
        "SELECT
            count(*) FILTER (WHERE s.kind = 'open') AS open_deals_count,
            coalesce(sum(d.value_cents) FILTER (WHERE s.kind = 'open'), 0)::bigint
                AS open_pipeline_cents,
            coalesce(sum(d.value_cents * s.probability) FILTER (WHERE s.kind = 'open') / 100, 0)::bigint
                AS weighted_pipeline_cents,
            coalesce(sum(d.value_cents) FILTER (
                WHERE s.kind = 'won' AND d.closed_at >= date_trunc('month', now())), 0)::bigint
                AS won_this_month_cents,
            count(*) FILTER (WHERE s.kind = 'won' AND d.closed_at >= date_trunc('month', now()))
                AS won_this_month_count,
            (count(*) FILTER (WHERE s.kind = 'won' AND d.closed_at >= now() - interval '90 days'))::float8
                / nullif(count(*) FILTER (
                    WHERE s.kind IN ('won', 'lost') AND d.closed_at >= now() - interval '90 days'), 0)
                AS win_rate_90d,
            (avg(extract(epoch FROM d.closed_at - d.created_at) / 86400) FILTER (
                WHERE s.kind = 'won' AND d.closed_at >= now() - interval '180 days'))::float8
                AS avg_days_to_win
         FROM deals d
         JOIN pipeline_stages s ON s.id = d.stage_id
         WHERE d.organization_id = $1 AND ($2::uuid IS NULL OR d.owner_id = $2)",
    )
    .bind(user.org_id)
    .bind(query.owner_id)
    .fetch_one(&state.pool)
    .await?;
    Ok(Json(summary))
}

fn start_of_day(date: NaiveDate) -> DateTime<Utc> {
    date.and_time(NaiveTime::MIN).and_utc()
}

fn default_from() -> NaiveDate {
    Utc::now().date_naive() - Days::new(180)
}

pub async fn funnel(
    State(state): State<AppState>,
    user: AuthUser,
    ApiQuery(query): ApiQuery<PeriodQuery>,
) -> AppResult<Json<Funnel>> {
    let from = query.from.unwrap_or_else(default_from);
    let since = start_of_day(from);

    let cohort_size: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM deals
         WHERE organization_id = $1 AND created_at >= $2 AND ($3::uuid IS NULL OR owner_id = $3)",
    )
    .bind(user.org_id)
    .bind(since)
    .bind(query.owner_id)
    .fetch_one(&state.pool)
    .await?;
    let rows: Vec<FunnelRow> = sqlx::query_as(
        "WITH cohort AS (
            SELECT id FROM deals
            WHERE organization_id = $1 AND created_at >= $2 AND ($3::uuid IS NULL OR owner_id = $3)
         ),
         progress AS (
            SELECT c.deal_id,
                   max(s.position) FILTER (WHERE s.kind = 'open') AS furthest_open,
                   bool_or(s.kind = 'won') AS won
            FROM deal_stage_changes c
            JOIN pipeline_stages s ON s.id = c.to_stage_id
            WHERE c.organization_id = $1 AND c.deal_id IN (SELECT id FROM cohort)
            GROUP BY c.deal_id
         )
         SELECT s.id AS stage_id, s.name, s.kind,
                (SELECT count(*) FROM progress p
                 WHERE p.won OR (s.kind = 'open' AND p.furthest_open >= s.position)) AS reached
         FROM pipeline_stages s
         WHERE s.organization_id = $1 AND s.kind <> 'lost'
         ORDER BY s.kind = 'won', s.position",
    )
    .bind(user.org_id)
    .bind(since)
    .bind(query.owner_id)
    .fetch_all(&state.pool)
    .await?;

    let mut previous: Option<i64> = None;
    let stages = rows
        .into_iter()
        .map(|row| {
            let conversion = previous.map(|prev| ratio(row.reached, prev));
            previous = Some(row.reached);
            FunnelStage {
                stage_id: row.stage_id,
                name: row.name,
                kind: row.kind,
                reached: row.reached,
                conversion_from_previous: conversion.flatten(),
            }
        })
        .collect();

    Ok(Json(Funnel {
        from,
        cohort_size,
        stages,
    }))
}

fn ratio(numerator: i64, denominator: i64) -> Option<f64> {
    (denominator > 0).then(|| numerator as f64 / denominator as f64)
}

pub async fn forecast(
    State(state): State<AppState>,
    user: AuthUser,
    ApiQuery(query): ApiQuery<ForecastQuery>,
) -> AppResult<Json<Forecast>> {
    let months = query.months.unwrap_or(6);
    if !(1..=12).contains(&months) {
        return Err(AppError::validation("months", "must be between 1 and 12"));
    }

    let rows: Vec<ForecastMonth> = sqlx::query_as(
        "WITH months AS (
            SELECT (date_trunc('month', current_date) + make_interval(months => g))::date AS month
            FROM generate_series(0, $2 - 1) AS g
         )
         SELECT m.month,
                count(d.id) AS deals,
                coalesce(sum(d.value_cents), 0)::bigint AS total_cents,
                coalesce(sum(d.value_cents * s.probability) / 100, 0)::bigint AS weighted_cents
         FROM months m
         LEFT JOIN (deals d JOIN pipeline_stages s ON s.id = d.stage_id AND s.kind = 'open')
            ON d.organization_id = $1
           AND ($3::uuid IS NULL OR d.owner_id = $3)
           AND d.expected_close_date >= m.month
           AND d.expected_close_date < m.month + interval '1 month'
         GROUP BY m.month
         ORDER BY m.month",
    )
    .bind(user.org_id)
    .bind(months)
    .bind(query.owner_id)
    .fetch_all(&state.pool)
    .await?;

    let (overdue_cents, undated_cents): (i64, i64) = sqlx::query_as(
        "SELECT
            coalesce(sum(d.value_cents) FILTER (
                WHERE d.expected_close_date < date_trunc('month', current_date)), 0)::bigint,
            coalesce(sum(d.value_cents) FILTER (WHERE d.expected_close_date IS NULL), 0)::bigint
         FROM deals d
         JOIN pipeline_stages s ON s.id = d.stage_id AND s.kind = 'open'
         WHERE d.organization_id = $1 AND ($2::uuid IS NULL OR d.owner_id = $2)",
    )
    .bind(user.org_id)
    .bind(query.owner_id)
    .fetch_one(&state.pool)
    .await?;

    Ok(Json(Forecast {
        months: rows,
        overdue_cents,
        undated_cents,
    }))
}

pub async fn leaderboard(
    State(state): State<AppState>,
    user: AuthUser,
    ApiQuery(query): ApiQuery<PeriodQuery>,
) -> AppResult<Json<Vec<LeaderboardRow>>> {
    user.require_manager()?;
    let since = start_of_day(query.from.unwrap_or_else(default_from));

    let mut rows: Vec<LeaderboardRow> = sqlx::query_as(
        "SELECT u.id AS user_id, u.full_name,
                count(d.id) FILTER (WHERE s.kind = 'won' AND d.closed_at >= $2) AS won_count,
                coalesce(sum(d.value_cents) FILTER (WHERE s.kind = 'won' AND d.closed_at >= $2), 0)::bigint
                    AS won_cents,
                count(d.id) FILTER (WHERE s.kind = 'lost' AND d.closed_at >= $2) AS lost_count,
                coalesce(sum(d.value_cents) FILTER (WHERE s.kind = 'open'), 0)::bigint
                    AS open_pipeline_cents,
                NULL::float8 AS win_rate
         FROM users u
         LEFT JOIN (deals d JOIN pipeline_stages s ON s.id = d.stage_id)
            ON d.owner_id = u.id AND d.organization_id = u.organization_id
         WHERE u.organization_id = $1 AND u.is_active
           AND ($3::uuid IS NULL OR u.id = $3)
         GROUP BY u.id, u.full_name
         ORDER BY won_cents DESC, u.full_name",
    )
    .bind(user.org_id)
    .bind(since)
    .bind(query.owner_id)
    .fetch_all(&state.pool)
    .await?;
    for row in &mut rows {
        row.win_rate = ratio(row.won_count, row.won_count + row.lost_count);
    }
    Ok(Json(rows))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ratio_handles_empty_denominator() {
        assert_eq!(ratio(3, 0), None);
        assert_eq!(ratio(1, 4), Some(0.25));
    }
}
