use axum::http::StatusCode;
use serde_json::json;
use sqlx::PgPool;

use crate::common::TestApp;

#[sqlx::test(migrator = "dealflow_api::MIGRATOR")]
async fn reps_edit_only_their_own_deals(pool: PgPool) {
    let app = TestApp::new(pool);
    let admin = app.register("Acme", "owner@acme.test").await;
    let rep = app.add_user(&admin, "rep@acme.test", "sales_rep").await;
    let lead = app.stage_id(&admin, "Qualified").await;

    let admins_deal = app.create_deal(&admin, json!({ "title": "Admin's" })).await;
    let reps_deal = app.create_deal(&rep, json!({ "title": "Rep's" })).await;
    app.get(&format!("/api/v1/deals/{}", admins_deal.id()), &rep)
        .await
        .assert_status(StatusCode::OK);

    app.put(
        &format!("/api/v1/deals/{}", admins_deal.id()),
        &rep,
        json!({ "title": "Taken over", "value_cents": 0, "version": 1 }),
    )
    .await
    .assert_status(StatusCode::FORBIDDEN);
    app.move_deal(&rep, &admins_deal.body, lead)
        .await
        .assert_status(StatusCode::FORBIDDEN);

    app.move_deal(&rep, &reps_deal.body, lead)
        .await
        .assert_status(StatusCode::OK);
}

#[sqlx::test(migrator = "dealflow_api::MIGRATOR")]
async fn reps_cannot_hand_records_to_colleagues(pool: PgPool) {
    let app = TestApp::new(pool);
    let admin = app.register("Acme", "owner@acme.test").await;
    let rep = app.add_user(&admin, "rep@acme.test", "sales_rep").await;
    let manager = app.add_user(&admin, "manager@acme.test", "manager").await;

    app.post(
        "/api/v1/companies",
        &rep,
        json!({ "name": "Mine?", "owner_id": admin.user_id }),
    )
    .await
    .assert_status(StatusCode::FORBIDDEN);

    let assigned = app
        .post(
            "/api/v1/companies",
            &manager,
            json!({ "name": "Assigned", "owner_id": rep.user_id }),
        )
        .await;
    assigned.assert_status(StatusCode::CREATED);
    assert_eq!(assigned.body["owner_id"], rep.user_id.to_string());
}

#[sqlx::test(migrator = "dealflow_api::MIGRATOR")]
async fn only_managers_and_admins_delete_records(pool: PgPool) {
    let app = TestApp::new(pool);
    let admin = app.register("Acme", "owner@acme.test").await;
    let rep = app.add_user(&admin, "rep@acme.test", "sales_rep").await;
    let manager = app.add_user(&admin, "manager@acme.test", "manager").await;

    let deal = app.create_deal(&rep, json!({ "title": "Doomed" })).await;
    let url = format!("/api/v1/deals/{}", deal.id());
    app.delete(&url, &rep)
        .await
        .assert_status(StatusCode::FORBIDDEN);
    app.delete(&url, &manager)
        .await
        .assert_status(StatusCode::NO_CONTENT);
    app.get(&url, &admin)
        .await
        .assert_status(StatusCode::NOT_FOUND);
}

#[sqlx::test(migrator = "dealflow_api::MIGRATOR")]
async fn workspace_settings_are_admin_only(pool: PgPool) {
    let app = TestApp::new(pool);
    let admin = app.register("Acme", "owner@acme.test").await;
    let manager = app.add_user(&admin, "manager@acme.test", "manager").await;

    app.post(
        "/api/v1/users",
        &manager,
        json!({ "email": "x@acme.test", "full_name": "X", "role": "admin", "password": "long-enough-password" }),
    )
    .await
    .assert_status(StatusCode::FORBIDDEN);
    app.post(
        "/api/v1/stages",
        &manager,
        json!({ "name": "Discovery", "probability": 15, "kind": "open" }),
    )
    .await
    .assert_status(StatusCode::FORBIDDEN);
}

#[sqlx::test(migrator = "dealflow_api::MIGRATOR")]
async fn admins_cannot_lock_themselves_out(pool: PgPool) {
    let app = TestApp::new(pool);
    let admin = app.register("Acme", "owner@acme.test").await;
    let res = app
        .put(
            &format!("/api/v1/users/{}", admin.user_id),
            &admin,
            json!({ "full_name": "Me", "role": "sales_rep", "is_active": true }),
        )
        .await;
    res.assert_status(StatusCode::CONFLICT);
}

#[sqlx::test(migrator = "dealflow_api::MIGRATOR")]
async fn role_changes_apply_to_existing_tokens(pool: PgPool) {
    let app = TestApp::new(pool);
    let admin = app.register("Acme", "owner@acme.test").await;
    let manager = app.add_user(&admin, "manager@acme.test", "manager").await;
    app.get("/api/v1/analytics/leaderboard", &manager)
        .await
        .assert_status(StatusCode::OK);

    app.put(
        &format!("/api/v1/users/{}", manager.user_id),
        &admin,
        json!({ "full_name": "Demoted", "role": "sales_rep", "is_active": true }),
    )
    .await
    .assert_status(StatusCode::OK);

    app.get("/api/v1/analytics/leaderboard", &manager)
        .await
        .assert_status(StatusCode::FORBIDDEN);
}
