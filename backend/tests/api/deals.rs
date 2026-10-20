use axum::http::StatusCode;
use serde_json::json;
use sqlx::PgPool;

use crate::common::TestApp;

#[sqlx::test(migrator = "dealflow_api::MIGRATOR")]
async fn new_deal_lands_in_first_open_stage_and_records_history(pool: PgPool) {
    let app = TestApp::new(pool);
    let admin = app.register("Acme", "owner@acme.test").await;

    let deal = app
        .create_deal(
            &admin,
            json!({ "title": "  Renewal  ", "value_cents": 120_000 }),
        )
        .await;
    assert_eq!(deal.body["title"], "Renewal");
    assert_eq!(deal.body["stage_name"], "Lead");
    assert_eq!(deal.body["owner_id"], admin.user_id.to_string());
    assert_eq!(deal.body["version"], 1);
    assert!(deal.body["closed_at"].is_null());

    let history = app
        .get(
            &format!("/api/v1/deals/{}/stage-history", deal.id()),
            &admin,
        )
        .await;
    let history = history.body.as_array().unwrap();
    assert_eq!(history.len(), 1);
    assert!(history[0]["from_stage_name"].is_null());
    assert_eq!(history[0]["to_stage_name"], "Lead");
}

#[sqlx::test(migrator = "dealflow_api::MIGRATOR")]
async fn moving_into_a_closing_stage_stamps_closed_at(pool: PgPool) {
    let app = TestApp::new(pool);
    let admin = app.register("Acme", "owner@acme.test").await;
    let won = app.stage_id(&admin, "Won").await;
    let lead = app.stage_id(&admin, "Lead").await;

    let deal = app
        .create_deal(&admin, json!({ "title": "Expansion" }))
        .await;
    let moved = app.move_deal(&admin, &deal.body, won).await;
    moved.assert_status(StatusCode::OK);
    assert_eq!(moved.body["stage_kind"], "won");
    assert!(moved.body["closed_at"].is_string());
    assert_eq!(moved.body["version"], 2);

    let reopened = app.move_deal(&admin, &moved.body, lead).await;
    reopened.assert_status(StatusCode::OK);
    assert!(reopened.body["closed_at"].is_null());

    let history = app
        .get(
            &format!("/api/v1/deals/{}/stage-history", deal.id()),
            &admin,
        )
        .await;
    let steps: Vec<_> = history
        .body
        .as_array()
        .unwrap()
        .iter()
        .map(|h| h["to_stage_name"].as_str().unwrap().to_owned())
        .collect();
    assert_eq!(steps, ["Lead", "Won", "Lead"]);
}

#[sqlx::test(migrator = "dealflow_api::MIGRATOR")]
async fn stale_version_is_rejected_with_conflict(pool: PgPool) {
    let app = TestApp::new(pool);
    let admin = app.register("Acme", "owner@acme.test").await;
    let deal = app
        .create_deal(&admin, json!({ "title": "Contested", "value_cents": 500 }))
        .await;
    let url = format!("/api/v1/deals/{}", deal.id());

    app.put(
        &url,
        &admin,
        json!({ "title": "First edit", "value_cents": 600, "version": 1 }),
    )
    .await
    .assert_status(StatusCode::OK);

    let stale = app
        .put(
            &url,
            &admin,
            json!({ "title": "Second edit", "value_cents": 700, "version": 1 }),
        )
        .await;
    stale.assert_status(StatusCode::CONFLICT);
    assert_eq!(stale.error_code(), "conflict");

    let current = app.get(&url, &admin).await;
    assert_eq!(current.body["title"], "First edit");
    assert_eq!(current.body["version"], 2);
}

#[sqlx::test(migrator = "dealflow_api::MIGRATOR")]
async fn board_groups_deals_by_stage_in_pipeline_order(pool: PgPool) {
    let app = TestApp::new(pool);
    let admin = app.register("Acme", "owner@acme.test").await;
    let proposal = app.stage_id(&admin, "Proposal").await;

    app.create_deal(&admin, json!({ "title": "A", "value_cents": 100 }))
        .await;
    app.create_deal(&admin, json!({ "title": "B", "value_cents": 250 }))
        .await;
    app.create_deal(
        &admin,
        json!({ "title": "C", "value_cents": 1000, "stage_id": proposal }),
    )
    .await;

    let board = app.get("/api/v1/deals/board", &admin).await;
    let columns = board.body.as_array().unwrap();
    assert_eq!(columns.len(), 6);
    assert_eq!(columns[0]["stage"]["name"], "Lead");
    assert_eq!(columns[0]["count"], 2);
    assert_eq!(columns[0]["total_value_cents"], 350);
    // Newest deals go to the top of a column.
    assert_eq!(columns[0]["deals"][0]["title"], "B");
    assert_eq!(columns[2]["count"], 1);
    assert_eq!(columns[2]["total_value_cents"], 1000);
}

#[sqlx::test(migrator = "dealflow_api::MIGRATOR")]
async fn deal_input_is_validated(pool: PgPool) {
    let app = TestApp::new(pool);
    let admin = app.register("Acme", "owner@acme.test").await;
    let res = app
        .post(
            "/api/v1/deals",
            &admin,
            json!({ "title": "", "value_cents": -5 }),
        )
        .await;
    res.assert_status(StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(res.error_fields(), ["title", "value_cents"]);

    let res = app
        .post("/api/v1/deals", &admin, json!({ "value_cents": "lots" }))
        .await;
    res.assert_status(StatusCode::UNPROCESSABLE_ENTITY);
}

#[sqlx::test(migrator = "dealflow_api::MIGRATOR")]
async fn every_change_is_written_to_the_audit_log(pool: PgPool) {
    let app = TestApp::new(pool);
    let admin = app.register("Acme", "owner@acme.test").await;
    let deal = app
        .create_deal(&admin, json!({ "title": "Audit me", "value_cents": 100 }))
        .await;
    app.put(
        &format!("/api/v1/deals/{}", deal.id()),
        &admin,
        json!({ "title": "Audit me", "value_cents": 900, "version": 1 }),
    )
    .await
    .assert_status(StatusCode::OK);

    let log = app
        .get(
            &format!("/api/v1/audit?entity_type=deal&entity_id={}", deal.id()),
            &admin,
        )
        .await;
    let entries = log.body["data"].as_array().unwrap();
    assert_eq!(entries.len(), 2);
    assert_eq!(entries[0]["action"], "updated");
    assert_eq!(
        entries[0]["changes"],
        json!({ "value_cents": { "from": 100, "to": 900 } })
    );
    assert_eq!(entries[1]["action"], "created");
    assert_eq!(entries[0]["actor_name"], "Acme Admin");
}

#[sqlx::test(migrator = "dealflow_api::MIGRATOR")]
async fn audit_log_rejects_tampering(pool: PgPool) {
    let app = TestApp::new(pool);
    let admin = app.register("Acme", "owner@acme.test").await;
    app.create_deal(&admin, json!({ "title": "Immutable" }))
        .await;
    let tampered = sqlx::query("UPDATE audit_log SET action = 'deleted'")
        .execute(&app.pool)
        .await;
    assert!(tampered.is_err(), "audit rows must not be editable");
}
