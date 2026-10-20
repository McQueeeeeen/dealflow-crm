use axum::http::StatusCode;
use serde_json::json;
use sqlx::PgPool;

use crate::common::TestApp;

#[sqlx::test(migrator = "dealflow_api::MIGRATOR")]
async fn company_search_and_rollups(pool: PgPool) {
    let app = TestApp::new(pool);
    let admin = app.register("Acme", "owner@acme.test").await;

    let harbor = app
        .post(
            "/api/v1/companies",
            &admin,
            json!({ "name": "Harbor Logistics", "domain": "Harbor.Example", "industry": " " }),
        )
        .await;
    harbor.assert_status(StatusCode::CREATED);
    assert_eq!(harbor.body["domain"], "harbor.example");
    assert!(
        harbor.body["industry"].is_null(),
        "blank optional text is stored as null"
    );

    app.post(
        "/api/v1/companies",
        &admin,
        json!({ "name": "Granite Foods" }),
    )
    .await
    .assert_status(StatusCode::CREATED);
    app.post(
        "/api/v1/contacts",
        &admin,
        json!({ "first_name": "Emma", "last_name": "Novak", "company_id": harbor.id() }),
    )
    .await
    .assert_status(StatusCode::CREATED);
    app.create_deal(
        &admin,
        json!({ "title": "Fleet", "value_cents": 4200, "company_id": harbor.id() }),
    )
    .await;

    let found = app.get("/api/v1/companies?q=harb", &admin).await;
    assert_eq!(found.body["total"], 1);
    let row = &found.body["data"][0];
    assert_eq!(row["contacts_count"], 1);
    assert_eq!(row["open_deals_count"], 1);
    assert_eq!(row["open_deals_value_cents"], 4200);

    // Wildcards in the search term are matched literally.
    let none = app.get("/api/v1/companies?q=%25", &admin).await;
    assert_eq!(none.body["total"], 0);
}

#[sqlx::test(migrator = "dealflow_api::MIGRATOR")]
async fn deleting_a_company_keeps_its_deals_and_contacts(pool: PgPool) {
    let app = TestApp::new(pool);
    let admin = app.register("Acme", "owner@acme.test").await;
    let company = app
        .post(
            "/api/v1/companies",
            &admin,
            json!({ "name": "Short-lived" }),
        )
        .await
        .id();
    let deal = app
        .create_deal(
            &admin,
            json!({ "title": "Survivor", "company_id": company }),
        )
        .await;

    app.delete(&format!("/api/v1/companies/{company}"), &admin)
        .await
        .assert_status(StatusCode::NO_CONTENT);
    let after = app
        .get(&format!("/api/v1/deals/{}", deal.id()), &admin)
        .await;
    after.assert_status(StatusCode::OK);
    assert!(after.body["company_id"].is_null());
}

#[sqlx::test(migrator = "dealflow_api::MIGRATOR")]
async fn contacts_paginate(pool: PgPool) {
    let app = TestApp::new(pool);
    let admin = app.register("Acme", "owner@acme.test").await;
    for name in ["Ana", "Ben", "Cleo", "Dev", "Eli"] {
        app.post("/api/v1/contacts", &admin, json!({ "first_name": name }))
            .await
            .assert_status(StatusCode::CREATED);
    }
    let page = app.get("/api/v1/contacts?page=2&per_page=2", &admin).await;
    assert_eq!(page.body["total"], 5);
    assert_eq!(page.body["page"], 2);
    let names: Vec<_> = page.body["data"]
        .as_array()
        .unwrap()
        .iter()
        .map(|c| c["first_name"].as_str().unwrap().to_owned())
        .collect();
    assert_eq!(names, ["Cleo", "Dev"]);
}

#[sqlx::test(migrator = "dealflow_api::MIGRATOR")]
async fn task_lifecycle_and_filters(pool: PgPool) {
    let app = TestApp::new(pool);
    let admin = app.register("Acme", "owner@acme.test").await;
    let rep = app.add_user(&admin, "rep@acme.test", "sales_rep").await;

    let overdue = app
        .post(
            "/api/v1/tasks",
            &admin,
            json!({ "title": "Call back", "due_at": "2020-01-01T09:00:00Z", "assignee_id": rep.user_id }),
        )
        .await;
    overdue.assert_status(StatusCode::CREATED);
    app.post("/api/v1/tasks", &rep, json!({ "title": "Someday" }))
        .await
        .assert_status(StatusCode::CREATED);

    let mine = app
        .get(
            &format!("/api/v1/tasks?assignee_id={}&status=overdue", rep.user_id),
            &rep,
        )
        .await;
    assert_eq!(mine.body["total"], 1);

    let url = format!("/api/v1/tasks/{}/complete", overdue.id());
    let done = app.post(&url, &rep, json!({})).await;
    done.assert_status(StatusCode::OK);
    let completed_at = done.body["completed_at"].clone();
    assert!(completed_at.is_string());
    // Completing twice is a no-op, not a new timestamp.
    let again = app.post(&url, &rep, json!({})).await;
    assert_eq!(again.body["completed_at"], completed_at);

    let open = app.get("/api/v1/tasks?status=open", &rep).await;
    assert_eq!(open.body["total"], 1);
}

#[sqlx::test(migrator = "dealflow_api::MIGRATOR")]
async fn notes_need_exactly_one_parent(pool: PgPool) {
    let app = TestApp::new(pool);
    let admin = app.register("Acme", "owner@acme.test").await;
    let deal = app
        .create_deal(&admin, json!({ "title": "Noted" }))
        .await
        .id();

    let res = app
        .post("/api/v1/notes", &admin, json!({ "body": "Orphan" }))
        .await;
    res.assert_status(StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(res.error_fields(), ["parent"]);

    app.post(
        "/api/v1/notes",
        &admin,
        json!({ "body": "Budget approved", "deal_id": deal }),
    )
    .await
    .assert_status(StatusCode::CREATED);
    let notes = app
        .get(&format!("/api/v1/notes?deal_id={deal}"), &admin)
        .await;
    assert_eq!(notes.body["total"], 1);
    assert_eq!(notes.body["data"][0]["author_name"], "Acme Admin");
}

#[sqlx::test(migrator = "dealflow_api::MIGRATOR")]
async fn pipeline_stages_can_be_reordered_but_not_deleted_while_used(pool: PgPool) {
    let app = TestApp::new(pool);
    let admin = app.register("Acme", "owner@acme.test").await;
    let mut ids: Vec<String> = app
        .stages(&admin)
        .await
        .iter()
        .map(|s| s["id"].as_str().unwrap().to_owned())
        .collect();

    ids.swap(0, 1);
    let reordered = app
        .put("/api/v1/stages/order", &admin, json!({ "stage_ids": ids }))
        .await;
    reordered.assert_status(StatusCode::OK);
    assert_eq!(reordered.body[0]["name"], "Qualified");

    let partial = app
        .put(
            "/api/v1/stages/order",
            &admin,
            json!({ "stage_ids": &ids[..2] }),
        )
        .await;
    partial.assert_status(StatusCode::UNPROCESSABLE_ENTITY);

    let deal = app.create_deal(&admin, json!({ "title": "Blocker" })).await;
    let stage = deal.body["stage_id"].as_str().unwrap();
    app.delete(&format!("/api/v1/stages/{stage}"), &admin)
        .await
        .assert_status(StatusCode::CONFLICT);

    let duplicate = app
        .post(
            "/api/v1/stages",
            &admin,
            json!({ "name": "Won", "probability": 100, "kind": "won" }),
        )
        .await;
    duplicate.assert_status(StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(duplicate.error_fields(), ["name"]);
}
