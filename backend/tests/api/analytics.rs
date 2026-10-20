use axum::http::StatusCode;
use chrono::Utc;
use serde_json::json;
use sqlx::PgPool;

use crate::common::TestApp;

#[sqlx::test(migrator = "dealflow_api::MIGRATOR")]
async fn summary_weights_open_pipeline_by_stage_probability(pool: PgPool) {
    let app = TestApp::new(pool);
    let admin = app.register("Acme", "owner@acme.test").await;
    let proposal = app.stage_id(&admin, "Proposal").await;
    let won = app.stage_id(&admin, "Won").await;
    let lost = app.stage_id(&admin, "Lost").await;
    app.create_deal(&admin, json!({ "title": "A", "value_cents": 1000 }))
        .await;
    app.create_deal(
        &admin,
        json!({ "title": "B", "value_cents": 2000, "stage_id": proposal }),
    )
    .await;
    let c = app
        .create_deal(&admin, json!({ "title": "C", "value_cents": 5000 }))
        .await;
    app.move_deal(&admin, &c.body, won)
        .await
        .assert_status(StatusCode::OK);
    let d = app
        .create_deal(&admin, json!({ "title": "D", "value_cents": 700 }))
        .await;
    app.move_deal(&admin, &d.body, lost)
        .await
        .assert_status(StatusCode::OK);

    let summary = app.get("/api/v1/analytics/summary", &admin).await;
    summary.assert_status(StatusCode::OK);
    let s = &summary.body;
    assert_eq!(s["open_deals_count"], 2);
    assert_eq!(s["open_pipeline_cents"], 3000);
    assert_eq!(s["weighted_pipeline_cents"], 1000 / 10 + 2000 / 2);
    assert_eq!(s["won_this_month_cents"], 5000);
    assert_eq!(s["won_this_month_count"], 1);
    assert_eq!(s["win_rate_90d"], 0.5);

    let filtered = app
        .get(
            &format!(
                "/api/v1/analytics/summary?owner_id={}",
                uuid::Uuid::now_v7()
            ),
            &admin,
        )
        .await;
    assert_eq!(filtered.body["open_deals_count"], 0);
}

#[sqlx::test(migrator = "dealflow_api::MIGRATOR")]
async fn funnel_counts_skipped_stages_as_passed(pool: PgPool) {
    let app = TestApp::new(pool);
    let admin = app.register("Acme", "owner@acme.test").await;
    let qualified = app.stage_id(&admin, "Qualified").await;
    let won = app.stage_id(&admin, "Won").await;

    app.create_deal(&admin, json!({ "title": "Stays in lead" }))
        .await;
    for title in ["Qualifies 1", "Qualifies 2"] {
        let deal = app.create_deal(&admin, json!({ "title": title })).await;
        app.move_deal(&admin, &deal.body, qualified)
            .await
            .assert_status(StatusCode::OK);
    }
    let fast = app
        .create_deal(&admin, json!({ "title": "Straight to won" }))
        .await;
    app.move_deal(&admin, &fast.body, won)
        .await
        .assert_status(StatusCode::OK);

    let funnel = app.get("/api/v1/analytics/funnel", &admin).await;
    funnel.assert_status(StatusCode::OK);
    assert_eq!(funnel.body["cohort_size"], 4);
    let stages = funnel.body["stages"].as_array().unwrap();
    let reached: Vec<_> = stages
        .iter()
        .map(|s| s["reached"].as_i64().unwrap())
        .collect();
    assert_eq!(
        reached,
        [4, 3, 1, 1, 1],
        "Lead, Qualified, Proposal, Negotiation, Won"
    );
    assert!(stages[0]["conversion_from_previous"].is_null());
    assert_eq!(stages[1]["conversion_from_previous"], 0.75);
    assert!(stages.iter().all(|s| s["kind"] != "lost"));
}

#[sqlx::test(migrator = "dealflow_api::MIGRATOR")]
async fn forecast_buckets_open_deals_by_expected_close_month(pool: PgPool) {
    let app = TestApp::new(pool);
    let admin = app.register("Acme", "owner@acme.test").await;
    let today = Utc::now().date_naive();

    app.create_deal(
        &admin,
        json!({ "title": "This month", "value_cents": 1000, "expected_close_date": today }),
    )
    .await;
    app.create_deal(
        &admin,
        json!({ "title": "Late", "value_cents": 300, "expected_close_date": "2020-03-15" }),
    )
    .await;
    app.create_deal(&admin, json!({ "title": "Undated", "value_cents": 50 }))
        .await;

    let forecast = app.get("/api/v1/analytics/forecast?months=3", &admin).await;
    forecast.assert_status(StatusCode::OK);
    let months = forecast.body["months"].as_array().unwrap();
    assert_eq!(months.len(), 3);
    assert_eq!(months[0]["total_cents"], 1000);
    assert_eq!(months[0]["weighted_cents"], 100);
    assert_eq!(months[1]["deals"], 0);
    assert_eq!(forecast.body["overdue_cents"], 300);
    assert_eq!(forecast.body["undated_cents"], 50);

    app.get("/api/v1/analytics/forecast?months=40", &admin)
        .await
        .assert_status(StatusCode::UNPROCESSABLE_ENTITY);
}

#[sqlx::test(migrator = "dealflow_api::MIGRATOR")]
async fn leaderboard_ranks_by_won_value(pool: PgPool) {
    let app = TestApp::new(pool);
    let admin = app.register("Acme", "owner@acme.test").await;
    let rep = app.add_user(&admin, "rep@acme.test", "sales_rep").await;
    let won = app.stage_id(&admin, "Won").await;

    let deal = app
        .create_deal(&rep, json!({ "title": "Rep win", "value_cents": 9000 }))
        .await;
    app.move_deal(&rep, &deal.body, won)
        .await
        .assert_status(StatusCode::OK);

    app.get("/api/v1/analytics/leaderboard", &rep)
        .await
        .assert_status(StatusCode::FORBIDDEN);
    let board = app.get("/api/v1/analytics/leaderboard", &admin).await;
    board.assert_status(StatusCode::OK);
    let rows = board.body.as_array().unwrap();
    assert_eq!(rows.len(), 2);
    assert_eq!(rows[0]["user_id"], rep.user_id.to_string());
    assert_eq!(rows[0]["won_cents"], 9000);
    assert_eq!(rows[0]["win_rate"], 1.0);
    assert!(rows[1]["win_rate"].is_null());
}
