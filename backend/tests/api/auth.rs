use axum::http::{Method, StatusCode};
use serde_json::json;
use sqlx::PgPool;

use crate::common::{TestApp, PASSWORD};

#[sqlx::test(migrator = "dealflow_api::MIGRATOR")]
async fn register_creates_workspace_with_default_pipeline(pool: PgPool) {
    let app = TestApp::new(pool);
    let admin = app.register("Acme", "Owner@Acme.test").await;

    let me = app.get("/api/v1/auth/me", &admin).await;
    me.assert_status(StatusCode::OK);
    assert_eq!(me.body["email"], "owner@acme.test", "email is normalized");
    assert_eq!(me.body["role"], "admin");
    assert_eq!(me.body["currency"], "USD");

    let names: Vec<_> = app
        .stages(&admin)
        .await
        .iter()
        .map(|s| s["name"].as_str().unwrap().to_owned())
        .collect();
    assert_eq!(
        names,
        [
            "Lead",
            "Qualified",
            "Proposal",
            "Negotiation",
            "Won",
            "Lost"
        ]
    );
}

#[sqlx::test(migrator = "dealflow_api::MIGRATOR")]
async fn register_validates_every_field(pool: PgPool) {
    let app = TestApp::new(pool);
    let res = app
        .request(
            Method::POST,
            "/api/v1/auth/register",
            None,
            Some(json!({
                "organization_name": " ",
                "full_name": "",
                "email": "not-an-email",
                "password": "short",
                "currency": "dollars",
            })),
        )
        .await;
    res.assert_status(StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(
        res.error_fields(),
        [
            "organization_name",
            "full_name",
            "email",
            "password",
            "currency"
        ]
    );
}

#[sqlx::test(migrator = "dealflow_api::MIGRATOR")]
async fn email_is_unique_across_organizations(pool: PgPool) {
    let app = TestApp::new(pool);
    app.register("Acme", "taken@example.test").await;
    let res = app
        .request(
            Method::POST,
            "/api/v1/auth/register",
            None,
            Some(json!({
                "organization_name": "Other",
                "full_name": "Someone",
                "email": "TAKEN@example.test",
                "password": PASSWORD,
            })),
        )
        .await;
    res.assert_status(StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(res.error_fields(), ["email"]);
}

#[sqlx::test(migrator = "dealflow_api::MIGRATOR")]
async fn login_rejects_bad_credentials_with_one_message(pool: PgPool) {
    let app = TestApp::new(pool);
    app.register("Acme", "owner@acme.test").await;

    for (email, password) in [
        ("owner@acme.test", "wrong-password-123"),
        ("nobody@acme.test", PASSWORD),
    ] {
        let res = app
            .request(
                Method::POST,
                "/api/v1/auth/login",
                None,
                Some(json!({ "email": email, "password": password })),
            )
            .await;
        res.assert_status(StatusCode::UNAUTHORIZED);
        assert_eq!(res.error_code(), "invalid_credentials");
    }
}

#[sqlx::test(migrator = "dealflow_api::MIGRATOR")]
async fn missing_or_forged_token_is_rejected(pool: PgPool) {
    let app = TestApp::new(pool);
    let res = app.request(Method::GET, "/api/v1/deals", None, None).await;
    res.assert_status(StatusCode::UNAUTHORIZED);
    assert_eq!(res.error_code(), "unauthorized");

    let res = app
        .request(Method::GET, "/api/v1/deals", Some("not.a.jwt"), None)
        .await;
    res.assert_status(StatusCode::UNAUTHORIZED);
}

async fn refresh(app: &TestApp, token: &str) -> crate::common::Response {
    app.request(
        Method::POST,
        "/api/v1/auth/refresh",
        None,
        Some(json!({ "refresh_token": token })),
    )
    .await
}

#[sqlx::test(migrator = "dealflow_api::MIGRATOR")]
async fn refresh_rotates_the_token(pool: PgPool) {
    let app = TestApp::new(pool);
    let session = app.register("Acme", "owner@acme.test").await;

    let first = refresh(&app, &session.refresh).await;
    first.assert_status(StatusCode::OK);
    let rotated = first.body["refresh_token"].as_str().unwrap().to_owned();
    assert_ne!(rotated, session.refresh);
    refresh(&app, &session.refresh)
        .await
        .assert_status(StatusCode::UNAUTHORIZED);
    refresh(&app, &rotated).await.assert_status(StatusCode::OK);
}

#[sqlx::test(migrator = "dealflow_api::MIGRATOR")]
async fn replaying_a_spent_refresh_token_revokes_the_whole_session(pool: PgPool) {
    let app = TestApp::new(pool);
    let session = app.register("Acme", "owner@acme.test").await;
    let rotated = refresh(&app, &session.refresh).await.body["refresh_token"]
        .as_str()
        .unwrap()
        .to_owned();
    sqlx::query(
        "UPDATE refresh_tokens SET used_at = now() - interval '1 minute' WHERE used_at IS NOT NULL",
    )
    .execute(&app.pool)
    .await
    .unwrap();

    refresh(&app, &session.refresh)
        .await
        .assert_status(StatusCode::UNAUTHORIZED);
    refresh(&app, &rotated)
        .await
        .assert_status(StatusCode::UNAUTHORIZED);
}

#[sqlx::test(migrator = "dealflow_api::MIGRATOR")]
async fn logout_revokes_the_refresh_token(pool: PgPool) {
    let app = TestApp::new(pool);
    let session = app.register("Acme", "owner@acme.test").await;
    app.request(
        Method::POST,
        "/api/v1/auth/logout",
        None,
        Some(json!({ "refresh_token": session.refresh })),
    )
    .await
    .assert_status(StatusCode::NO_CONTENT);
    refresh(&app, &session.refresh)
        .await
        .assert_status(StatusCode::UNAUTHORIZED);
}

#[sqlx::test(migrator = "dealflow_api::MIGRATOR")]
async fn deactivated_user_loses_access_immediately(pool: PgPool) {
    let app = TestApp::new(pool);
    let admin = app.register("Acme", "owner@acme.test").await;
    let rep = app.add_user(&admin, "rep@acme.test", "sales_rep").await;
    app.get("/api/v1/deals", &rep)
        .await
        .assert_status(StatusCode::OK);

    app.put(
        &format!("/api/v1/users/{}", rep.user_id),
        &admin,
        json!({ "full_name": "Rep", "role": "sales_rep", "is_active": false }),
    )
    .await
    .assert_status(StatusCode::OK);

    app.get("/api/v1/deals", &rep)
        .await
        .assert_status(StatusCode::UNAUTHORIZED);
    refresh(&app, &rep.refresh)
        .await
        .assert_status(StatusCode::UNAUTHORIZED);
}
