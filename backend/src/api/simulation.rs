use std::collections::HashMap;

use axum::extract::State;
use axum::routing::get;
use axum::{Json, Router};
use chrono::{DateTime, Datelike, Months, NaiveDate, Utc};
use rand::rngs::StdRng;
use rand::SeedableRng;
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use uuid::Uuid;

use crate::api::stages::{fetch_stages, Stage};
use crate::auth::AuthUser;
use crate::error::AppResult;
use crate::extract::ApiQuery;
use crate::models::StageKind;
use crate::simulation::{self, Distribution, OpenDeal, StageModel, Transition};
use crate::state::AppState;

const DEFAULT_RUNS: usize = 10_000;
const BINS: usize = 24;

pub fn router() -> Router<AppState> {
    Router::new().route("/analytics/simulation", get(simulate))
}

#[derive(Deserialize)]
pub struct SimulationQuery {
    pub owner_id: Option<Uuid>,
    pub runs: Option<usize>,
    pub seed: Option<u64>,
}

#[derive(Serialize)]
pub struct Simulation {
    pub runs: usize,
    pub open_deals: usize,
    pub periods: Vec<Period>,
    pub stages: Vec<StageFit>,
}

#[derive(Serialize)]
pub struct Period {
    pub key: &'static str,
    pub starts_on: NaiveDate,
    pub ends_on: NaiveDate,
    pub won_cents: i64,
    pub target_cents: Option<i64>,
    pub chance_to_hit_target: Option<f64>,
    pub distribution: Distribution,
}

#[derive(Serialize)]
pub struct StageFit {
    pub stage_id: Uuid,
    pub name: String,
    pub advance_rate: f64,
    pub median_days: f64,
    pub samples: usize,
}

fn periods(today: NaiveDate) -> [(&'static str, NaiveDate, NaiveDate); 2] {
    let month = today.with_day(1).unwrap_or(today);
    let quarter = month - Months::new(month.month0() % 3);
    [
        ("month", month, month + Months::new(1)),
        ("quarter", quarter, quarter + Months::new(3)),
    ]
}

async fn learn(pool: &PgPool, org_id: Uuid, stages: &[Stage]) -> AppResult<Vec<Vec<Transition>>> {
    let history: Vec<(Uuid, Uuid, DateTime<Utc>)> = sqlx::query_as(
        "SELECT deal_id, to_stage_id, changed_at FROM deal_stage_changes
         WHERE organization_id = $1 AND changed_at >= now() - interval '365 days'
         ORDER BY deal_id, changed_at, id",
    )
    .bind(org_id)
    .fetch_all(pool)
    .await?;
    let by_id: HashMap<Uuid, &Stage> = stages.iter().map(|s| (s.id, s)).collect();
    let open: Vec<&Stage> = stages
        .iter()
        .filter(|s| s.kind == StageKind::Open)
        .collect();
    let index: HashMap<Uuid, usize> = open.iter().enumerate().map(|(i, s)| (s.id, i)).collect();

    let mut transitions = vec![Vec::new(); open.len()];
    for pair in history.windows(2) {
        let ((deal, from, entered), (next_deal, to, left)) = (&pair[0], &pair[1]);
        let (Some(&i), Some(next)) = (index.get(from), by_id.get(to)) else {
            continue;
        };
        let advanced = match next.kind {
            _ if deal != next_deal => continue,
            StageKind::Won => true,
            StageKind::Lost => false,
            StageKind::Open if next.position > open[i].position => true,
            StageKind::Open => continue,
        };
        transitions[i].push(Transition {
            days: (*left - *entered).num_seconds() as f64 / 86_400.0,
            advanced,
        });
    }
    Ok(transitions)
}

pub async fn simulate(
    State(state): State<AppState>,
    user: AuthUser,
    ApiQuery(query): ApiQuery<SimulationQuery>,
) -> AppResult<Json<Simulation>> {
    let runs = query.runs.unwrap_or(DEFAULT_RUNS).clamp(1_000, 20_000);
    let stages = fetch_stages(&state.pool, user.org_id).await?;
    let open: Vec<&Stage> = stages
        .iter()
        .filter(|s| s.kind == StageKind::Open)
        .collect();
    let transitions = learn(&state.pool, user.org_id, &stages).await?;
    let models: Vec<StageModel> = open
        .iter()
        .enumerate()
        .map(|(i, stage)| {
            let next = open.get(i + 1).map_or(100.0, |s| f64::from(s.probability));
            let prior = if next > 0.0 {
                f64::from(stage.probability) / next
            } else {
                0.5
            };
            StageModel::learn(&transitions[i], prior)
        })
        .collect();

    let index: HashMap<Uuid, usize> = open.iter().enumerate().map(|(i, s)| (s.id, i)).collect();
    let rows: Vec<(i64, Uuid, f64)> = sqlx::query_as(
        "SELECT d.value_cents, d.stage_id,
                extract(epoch FROM now() - coalesce(
                    (SELECT max(c.changed_at) FROM deal_stage_changes c
                     WHERE c.organization_id = d.organization_id AND c.deal_id = d.id
                       AND c.to_stage_id = d.stage_id),
                    d.created_at))::float8 / 86400
         FROM deals d JOIN pipeline_stages s ON s.id = d.stage_id
         WHERE d.organization_id = $1 AND s.kind = 'open' AND ($2::uuid IS NULL OR d.owner_id = $2)",
    )
    .bind(user.org_id)
    .bind(query.owner_id)
    .fetch_all(&state.pool)
    .await?;
    let deals: Vec<OpenDeal> = rows
        .into_iter()
        .filter_map(|(value_cents, stage_id, days_in_stage)| {
            index.get(&stage_id).map(|&stage| OpenDeal {
                value_cents,
                stage,
                days_in_stage,
            })
        })
        .collect();

    let now = Utc::now();
    let windows = periods(now.date_naive());
    let horizons: Vec<f64> = windows
        .iter()
        .map(|(_, _, end)| {
            (end.and_time(chrono::NaiveTime::MIN).and_utc() - now).num_seconds() as f64 / 86_400.0
        })
        .collect();
    let (seed, sim_models, sim_deals) = (query.seed.unwrap_or(2026), models.clone(), deals.clone());
    let totals = tokio::task::spawn_blocking(move || {
        let mut rng = StdRng::seed_from_u64(seed);
        simulation::simulate(&sim_models, &sim_deals, &horizons, runs, &mut rng)
    })
    .await
    .map_err(anyhow::Error::from)?;

    let mut periods = Vec::new();
    for ((key, start, end), mut totals) in windows.into_iter().zip(totals) {
        let won_cents: i64 = sqlx::query_scalar(
            "SELECT coalesce(sum(d.value_cents), 0)::bigint
             FROM deals d JOIN pipeline_stages s ON s.id = d.stage_id
             WHERE d.organization_id = $1 AND s.kind = 'won' AND d.closed_at >= $2
               AND ($3::uuid IS NULL OR d.owner_id = $3)",
        )
        .bind(user.org_id)
        .bind(start)
        .bind(query.owner_id)
        .fetch_one(&state.pool)
        .await?;
        let target_cents: Option<i64> = sqlx::query_scalar(
            "SELECT sum(amount_cents)::bigint FROM sales_targets
             WHERE organization_id = $1 AND month >= $2 AND month < $3
               AND ($4::uuid IS NULL OR user_id = $4)",
        )
        .bind(user.org_id)
        .bind(start)
        .bind(end)
        .bind(query.owner_id)
        .fetch_one(&state.pool)
        .await?;
        for value in &mut totals {
            *value += won_cents;
        }
        let distribution = simulation::summarize(&mut totals, BINS);
        periods.push(Period {
            key,
            starts_on: start,
            ends_on: end.pred_opt().unwrap_or(end),
            won_cents,
            chance_to_hit_target: target_cents.map(|t| simulation::share_at_least(&totals, t)),
            target_cents,
            distribution,
        });
    }

    let stages = open
        .iter()
        .zip(&models)
        .zip(&transitions)
        .map(|((stage, model), seen)| StageFit {
            stage_id: stage.id,
            name: stage.name.clone(),
            advance_rate: model.advance,
            median_days: model.median_days(),
            samples: seen.len(),
        })
        .collect();
    Ok(Json(Simulation {
        runs,
        open_deals: deals.len(),
        periods,
        stages,
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn periods_cover_the_month_and_its_quarter() {
        let day = NaiveDate::from_ymd_opt(2026, 11, 20).unwrap();
        let [(_, month, month_end), (_, quarter, quarter_end)] = periods(day);
        assert_eq!(month, NaiveDate::from_ymd_opt(2026, 11, 1).unwrap());
        assert_eq!(month_end, NaiveDate::from_ymd_opt(2026, 12, 1).unwrap());
        assert_eq!(quarter, NaiveDate::from_ymd_opt(2026, 10, 1).unwrap());
        assert_eq!(quarter_end, NaiveDate::from_ymd_opt(2027, 1, 1).unwrap());
    }
}
