use axum::http::StatusCode;
use serde_json::json;
use sqlx::PgPool;

use crate::common::TestApp;

#[sqlx::test(migrator = "dealflow_api::MIGRATOR")]
async fn records_of_another_organization_behave_as_missing(pool: PgPool) {
    let app = TestApp::new(pool);
    let acme = app.register("Acme", "owner@acme.test").await;
    let globex = app.register("Globex", "owner@globex.test").await;

    let company = app
        .post("/api/v1/companies", &acme, json!({ "name": "Acme Client" }))
        .await;
    let deal = app
        .create_deal(
            &acme,
            json!({ "title": "Acme secret deal", "value_cents": 100 }),
        )
        .await;
    let deal_url = format!("/api/v1/deals/{}", deal.id());
    let company_url = format!("/api/v1/companies/{}", company.id());

    app.get(&deal_url, &globex)
        .await
        .assert_status(StatusCode::NOT_FOUND);
    app.get(&company_url, &globex)
        .await
        .assert_status(StatusCode::NOT_FOUND);
    app.put(
        &deal_url,
        &globex,
        json!({ "title": "Hijacked", "value_cents": 1, "version": 1 }),
    )
    .await
    .assert_status(StatusCode::NOT_FOUND);
    app.delete(&deal_url, &globex)
        .await
        .assert_status(StatusCode::NOT_FOUND);

    let listed = app.get("/api/v1/deals", &globex).await;
    assert_eq!(listed.body["total"], 0);
    let audit = app.get("/api/v1/audit", &globex).await;
    assert_eq!(audit.body["total"], 0);

    // The original is untouched.
    let original = app.get(&deal_url, &acme).await;
    assert_eq!(original.body["title"], "Acme secret deal");
}

#[sqlx::test(migrator = "dealflow_api::MIGRATOR")]
async fn cannot_link_to_another_organizations_records(pool: PgPool) {
    let app = TestApp::new(pool);
    let acme = app.register("Acme", "owner@acme.test").await;
    let globex = app.register("Globex", "owner@globex.test").await;

    let foreign_company = app
        .post("/api/v1/companies", &acme, json!({ "name": "Acme Client" }))
        .await
        .id();
    let foreign_stage = app.stage_id(&acme, "Lead").await;

    let res = app
        .post(
            "/api/v1/deals",
            &globex,
            json!({ "title": "Sneaky", "company_id": foreign_company }),
        )
        .await;
    res.assert_status(StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(res.error_fields(), ["company_id"]);

    let res = app
        .post(
            "/api/v1/deals",
            &globex,
            json!({ "title": "Sneaky", "stage_id": foreign_stage }),
        )
        .await;
    res.assert_status(StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(res.error_fields(), ["stage_id"]);

    let res = app
        .post(
            "/api/v1/contacts",
            &globex,
            json!({ "first_name": "Eve", "company_id": foreign_company }),
        )
        .await;
    res.assert_status(StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(res.error_fields(), ["company_id"]);
}

#[sqlx::test(migrator = "dealflow_api::MIGRATOR")]
async fn managers_cannot_assign_records_to_users_of_another_organization(pool: PgPool) {
    let app = TestApp::new(pool);
    let acme = app.register("Acme", "owner@acme.test").await;
    let globex = app.register("Globex", "owner@globex.test").await;

    let res = app
        .post(
            "/api/v1/companies",
            &globex,
            json!({ "name": "Target", "owner_id": acme.user_id }),
        )
        .await;
    res.assert_status(StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(res.error_fields(), ["owner_id"]);
}
