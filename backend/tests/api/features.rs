use std::time::Duration;

use axum::body::Body;
use axum::http::StatusCode;
use chrono::{Datelike, Utc};
use http_body_util::BodyExt;
use serde_json::json;
use sqlx::PgPool;

use crate::common::TestApp;

async fn next_event(body: &mut Body, name: &str) -> String {
    let marker = format!("event: {name}\n");
    let mut buffer = String::new();
    tokio::time::timeout(Duration::from_secs(10), async {
        loop {
            let frame = body.frame().await.expect("stream open").expect("frame");
            if let Ok(data) = frame.into_data() {
                buffer.push_str(std::str::from_utf8(&data).expect("utf-8"));
            }
            if let Some(start) = buffer.find(&marker) {
                if let Some(end) = buffer[start..].find("\n\n") {
                    return buffer[start..start + end].to_owned();
                }
            }
        }
    })
    .await
    .unwrap_or_else(|_| panic!("no {name} event, got: {buffer}"))
}

#[sqlx::test(migrator = "dealflow_api::MIGRATOR")]
async fn live_changes_stay_inside_their_workspace(pool: PgPool) {
    let app = TestApp::new(pool);
    let acme = app.register("Acme", "owner@acme.test").await;
    let globex = app.register("Globex", "owner@globex.test").await;

    let stream = app.open("/api/v1/live", &acme).await;
    assert_eq!(stream.status(), StatusCode::OK);
    assert_eq!(
        stream.headers()["content-type"],
        "text/event-stream",
        "served as server-sent events"
    );
    let mut body = stream.into_body();
    next_event(&mut body, "hello").await;
    app.hub.listening().await;

    let foreign = app
        .create_deal(&globex, json!({ "title": "Not yours" }))
        .await;
    let own = app.create_deal(&acme, json!({ "title": "Fleet" })).await;

    let event = next_event(&mut body, "change").await;
    assert!(event.contains(&own.id().to_string()), "{event}");
    assert!(event.contains("\"entity\":\"deal\""), "{event}");
    assert!(!event.contains(&foreign.id().to_string()), "{event}");
}

#[sqlx::test(migrator = "dealflow_api::MIGRATOR")]
async fn presence_lists_who_is_viewing(pool: PgPool) {
    let app = TestApp::new(pool);
    let admin = app.register("Acme", "owner@acme.test").await;

    let mut body = app.open("/api/v1/live", &admin).await.into_body();
    let hello = next_event(&mut body, "hello").await;
    let connection = hello.rsplit("data: ").next().expect("connection id");

    app.put(
        "/api/v1/live/presence",
        &admin,
        json!({ "connection": connection, "view": "deal:42" }),
    )
    .await
    .assert_status(StatusCode::NO_CONTENT);
    let mut presence = next_event(&mut body, "presence").await;
    if !presence.contains("deal:42") {
        presence = next_event(&mut body, "presence").await;
    }
    assert!(presence.contains("\"view\":\"deal:42\""), "{presence}");
    assert!(presence.contains("Acme Admin"), "{presence}");
}

#[sqlx::test(migrator = "dealflow_api::MIGRATOR")]
async fn search_tolerates_typos_and_respects_tenancy(pool: PgPool) {
    let app = TestApp::new(pool);
    let acme = app.register("Acme", "owner@acme.test").await;
    let globex = app.register("Globex", "owner@globex.test").await;

    app.post(
        "/api/v1/companies",
        &acme,
        json!({ "name": "Harbor Logistics", "domain": "harbor.example" }),
    )
    .await
    .assert_status(StatusCode::CREATED);
    app.post(
        "/api/v1/contacts",
        &acme,
        json!({ "first_name": "Emma", "last_name": "Novak", "email": "emma@harbor.example" }),
    )
    .await
    .assert_status(StatusCode::CREATED);
    app.create_deal(&acme, json!({ "title": "Fleet renewal" }))
        .await;
    app.post(
        "/api/v1/companies",
        &globex,
        json!({ "name": "Harbor Freight" }),
    )
    .await
    .assert_status(StatusCode::CREATED);

    let typo = app.get("/api/v1/search?q=harbr", &acme).await;
    let companies = typo.body["companies"].as_array().unwrap();
    assert_eq!(companies.len(), 1, "{}", typo.body);
    assert_eq!(companies[0]["title"], "Harbor Logistics");

    let people = app.get("/api/v1/search?q=novak", &acme).await;
    assert_eq!(people.body["contacts"][0]["title"], "Emma Novak");
    assert_eq!(people.body["contacts"][0]["detail"], "emma@harbor.example");

    let deals = app.get("/api/v1/search?q=fleet", &acme).await;
    assert_eq!(deals.body["deals"][0]["title"], "Fleet renewal");

    let short = app.get("/api/v1/search?q=h", &acme).await;
    assert_eq!(
        short.body,
        json!({ "deals": [], "companies": [], "contacts": [] })
    );
}

#[sqlx::test(migrator = "dealflow_api::MIGRATOR")]
async fn targets_track_won_value_and_need_a_manager(pool: PgPool) {
    let app = TestApp::new(pool);
    let admin = app.register("Acme", "owner@acme.test").await;
    let rep = app.add_user(&admin, "rep@acme.test", "sales_rep").await;
    let month = Utc::now().date_naive().with_day(1).unwrap();

    app.put(
        &format!("/api/v1/targets/{}", rep.user_id),
        &rep,
        json!({ "month": month, "amount_cents": 1 }),
    )
    .await
    .assert_status(StatusCode::FORBIDDEN);
    app.put(
        &format!("/api/v1/targets/{}", rep.user_id),
        &admin,
        json!({ "month": month, "amount_cents": 500_000 }),
    )
    .await
    .assert_status(StatusCode::NO_CONTENT);

    let deal = app
        .create_deal(&rep, json!({ "title": "Fleet", "value_cents": 120_000 }))
        .await;
    let won = app.stage_id(&rep, "Won").await;
    app.move_deal(&rep, &deal.body, won)
        .await
        .assert_status(StatusCode::OK);

    let targets = app.get("/api/v1/targets", &rep).await;
    let row = targets.body["rows"]
        .as_array()
        .unwrap()
        .iter()
        .find(|r| r["user_id"] == rep.user_id.to_string())
        .cloned()
        .expect("rep row");
    assert_eq!(row["target_cents"], 500_000);
    assert_eq!(row["won_cents"], 120_000);

    let audit = app.get("/api/v1/audit?entity_type=target", &admin).await;
    assert_eq!(audit.body["total"], 1, "{}", audit.body);
}

#[sqlx::test(migrator = "dealflow_api::MIGRATOR")]
async fn contact_import_previews_then_dedupes(pool: PgPool) {
    let app = TestApp::new(pool);
    let admin = app.register("Acme", "owner@acme.test").await;
    let rows = json!([
        { "first_name": "Emma", "last_name": "Novak", "email": "Emma@Harbor.example", "company": "Harbor" },
        { "first_name": "", "email": "nobody@example.com" },
        { "first_name": "Liam", "email": "not-an-email" }
    ]);

    let preview = app
        .post(
            "/api/v1/import/contacts",
            &admin,
            json!({ "rows": rows, "dry_run": true }),
        )
        .await;
    preview.assert_status(StatusCode::OK);
    assert_eq!(preview.body["created"], 1);
    assert_eq!(preview.body["companies_created"], 1);
    assert_eq!(preview.body["errors"][0]["row"], 2);
    assert_eq!(
        preview.body["errors"][0]["message"],
        "first name is required"
    );
    assert_eq!(preview.body["errors"][1]["row"], 3);
    let empty = app.get("/api/v1/contacts", &admin).await;
    assert_eq!(empty.body["total"], 0, "a dry run writes nothing");

    app.post("/api/v1/import/contacts", &admin, json!({ "rows": rows }))
        .await
        .assert_status(StatusCode::OK);
    let again = app
        .post(
            "/api/v1/import/contacts",
            &admin,
            json!({ "rows": [{ "first_name": "Emma", "email": "emma@harbor.example", "job_title": "COO" }] }),
        )
        .await;
    assert_eq!(again.body["created"], 0);
    assert_eq!(again.body["updated"], 1);
    let repeat = app
        .post(
            "/api/v1/import/contacts",
            &admin,
            json!({ "rows": [{ "first_name": "Emma", "email": "emma@harbor.example", "job_title": "COO" }] }),
        )
        .await;
    assert_eq!(repeat.body["unchanged"], 1);

    let contacts = app.get("/api/v1/contacts", &admin).await;
    assert_eq!(contacts.body["total"], 1);
    let emma = &contacts.body["data"][0];
    assert_eq!(
        emma["last_name"], "Novak",
        "blank cells keep existing values"
    );
    assert_eq!(emma["job_title"], "COO");
    assert_eq!(emma["company_name"], "Harbor");
}

#[sqlx::test(migrator = "dealflow_api::MIGRATOR")]
async fn company_import_matches_by_domain(pool: PgPool) {
    let app = TestApp::new(pool);
    let admin = app.register("Acme", "owner@acme.test").await;
    app.post(
        "/api/v1/companies",
        &admin,
        json!({ "name": "Harbor Logistics", "domain": "harbor.example" }),
    )
    .await
    .assert_status(StatusCode::CREATED);

    let report = app
        .post(
            "/api/v1/import/companies",
            &admin,
            json!({ "rows": [
                { "name": "Harbor Logistics Inc", "domain": "https://www.harbor.example/", "industry": "Freight" },
                { "name": "Granite Foods" }
            ] }),
        )
        .await;
    assert_eq!(report.body["updated"], 1, "{}", report.body);
    assert_eq!(report.body["created"], 1);
    let too_many = app
        .post("/api/v1/import/companies", &admin, json!({ "rows": [] }))
        .await;
    too_many.assert_status(StatusCode::UNPROCESSABLE_ENTITY);
}

#[sqlx::test(migrator = "dealflow_api::MIGRATOR")]
async fn export_is_csv_with_formulas_neutralised(pool: PgPool) {
    let app = TestApp::new(pool);
    let admin = app.register("Acme", "owner@acme.test").await;
    app.post(
        "/api/v1/companies",
        &admin,
        json!({ "name": "=HYPERLINK(\"evil\")" }),
    )
    .await
    .assert_status(StatusCode::CREATED);

    let res = app.open("/api/v1/export/companies", &admin).await;
    assert_eq!(res.status(), StatusCode::OK);
    assert!(res.headers()["content-disposition"]
        .to_str()
        .unwrap()
        .starts_with("attachment; filename=\"dealflow-companies-"));
    let bytes = res.into_body().collect().await.unwrap().to_bytes();
    let csv = String::from_utf8(bytes.to_vec()).unwrap();
    assert!(csv.starts_with("\u{feff}Name,Domain,Industry,Owner,Created\r\n"));
    assert!(csv.contains("\"'=HYPERLINK(\"\"evil\"\")\""), "{csv}");

    let missing = app.get("/api/v1/export/passwords", &admin).await;
    missing.assert_status(StatusCode::NOT_FOUND);
}

#[sqlx::test(migrator = "dealflow_api::MIGRATOR")]
async fn cold_threshold_and_last_activity(pool: PgPool) {
    let app = TestApp::new(pool);
    let admin = app.register("Acme", "owner@acme.test").await;
    let rep = app.add_user(&admin, "rep@acme.test", "sales_rep").await;

    let me = app.get("/api/v1/auth/me", &admin).await;
    assert_eq!(me.body["stale_after_days"], 14);
    app.put(
        "/api/v1/organization",
        &rep,
        json!({ "stale_after_days": 7 }),
    )
    .await
    .assert_status(StatusCode::FORBIDDEN);
    app.put(
        "/api/v1/organization",
        &admin,
        json!({ "stale_after_days": 0 }),
    )
    .await
    .assert_status(StatusCode::UNPROCESSABLE_ENTITY);
    app.put(
        "/api/v1/organization",
        &admin,
        json!({ "stale_after_days": 7 }),
    )
    .await
    .assert_status(StatusCode::NO_CONTENT);
    let me = app.get("/api/v1/auth/me", &rep).await;
    assert_eq!(me.body["stale_after_days"], 7);

    let deal = app.create_deal(&admin, json!({ "title": "Fleet" })).await;
    sqlx::query("UPDATE deals SET updated_at = now() - interval '30 days' WHERE id = $1")
        .bind(deal.id())
        .execute(&app.pool)
        .await
        .unwrap();
    let quiet = app
        .get(&format!("/api/v1/deals/{}", deal.id()), &admin)
        .await;
    let before = quiet.body["last_activity_at"].as_str().unwrap().to_owned();
    app.post(
        "/api/v1/notes",
        &admin,
        json!({ "body": "Called them", "deal_id": deal.id() }),
    )
    .await
    .assert_status(StatusCode::CREATED);
    let active = app
        .get(&format!("/api/v1/deals/{}", deal.id()), &admin)
        .await;
    let after = active.body["last_activity_at"].as_str().unwrap();
    assert!(after > before.as_str(), "{before} -> {after}");
}

#[sqlx::test(migrator = "dealflow_api::MIGRATOR")]
async fn simulation_counts_won_revenue_and_spreads_the_rest(pool: PgPool) {
    let app = TestApp::new(pool);
    let admin = app.register("Acme", "owner@acme.test").await;
    let won = app.stage_id(&admin, "Won").await;
    let closed = app
        .create_deal(&admin, json!({ "title": "Signed", "value_cents": 50_000 }))
        .await;
    app.move_deal(&admin, &closed.body, won)
        .await
        .assert_status(StatusCode::OK);
    for i in 0..6 {
        app.create_deal(
            &admin,
            json!({ "title": format!("Open {i}"), "value_cents": 100_000 }),
        )
        .await;
    }
    let month = Utc::now().date_naive().with_day(1).unwrap();
    app.put(
        &format!("/api/v1/targets/{}", admin.user_id),
        &admin,
        json!({ "month": month, "amount_cents": 200_000 }),
    )
    .await
    .assert_status(StatusCode::NO_CONTENT);

    let sim = app
        .get("/api/v1/analytics/simulation?runs=50", &admin)
        .await;
    sim.assert_status(StatusCode::OK);
    assert_eq!(sim.body["runs"], 1000, "runs are clamped");
    assert_eq!(sim.body["open_deals"], 6);
    let month = &sim.body["periods"][0];
    assert_eq!(month["key"], "month");
    assert_eq!(month["won_cents"], 50_000);
    assert_eq!(month["target_cents"], 200_000);
    let chance = month["chance_to_hit_target"].as_f64().unwrap();
    assert!((0.0..=1.0).contains(&chance));
    let d = &month["distribution"];
    let (p10, p50, p90) = (
        d["p10_cents"].as_i64().unwrap(),
        d["p50_cents"].as_i64().unwrap(),
        d["p90_cents"].as_i64().unwrap(),
    );
    assert!(
        50_000 <= p10 && p10 <= p50 && p50 <= p90 && p90 <= 650_000,
        "{d}"
    );
    let binned: u64 = d["histogram"]
        .as_array()
        .unwrap()
        .iter()
        .map(|b| b["runs"].as_u64().unwrap())
        .sum();
    assert_eq!(binned, 1000);
    let quarter = &sim.body["periods"][1];
    assert!(quarter["distribution"]["p50_cents"].as_i64().unwrap() >= p50);
    assert_eq!(sim.body["stages"].as_array().unwrap().len(), 4);

    let again = app
        .get("/api/v1/analytics/simulation?runs=50", &admin)
        .await;
    assert_eq!(
        again.body["periods"], sim.body["periods"],
        "seeded runs repeat"
    );
}

#[sqlx::test(migrator = "dealflow_api::MIGRATOR")]
async fn deal_filters_compile_safely_and_report_errors(pool: PgPool) {
    let app = TestApp::new(pool);
    let admin = app.register("Acme", "owner@acme.test").await;
    let rep = app.add_user(&admin, "rep@acme.test", "sales_rep").await;
    app.create_deal(
        &admin,
        json!({ "title": "Fleet renewal", "value_cents": 9_000_000 }),
    )
    .await;
    app.create_deal(
        &admin,
        json!({ "title": "Small pilot", "value_cents": 200_000 }),
    )
    .await;
    app.create_deal(
        &rep,
        json!({ "title": "Fleet pilot", "value_cents": 6_000_000 }),
    )
    .await;

    let count = |body: &serde_json::Value| body["total"].as_i64().unwrap();
    let big = app.get("/api/v1/deals?filter=value%3E%3D50k", &admin).await;
    assert_eq!(count(&big.body), 2);
    let mine = app
        .get("/api/v1/deals?filter=fleet%20owner%3Ame", &rep)
        .await;
    assert_eq!(count(&mine.body), 1);
    assert_eq!(mine.body["data"][0]["title"], "Fleet pilot");
    let either = app
        .get(
            "/api/v1/deals?filter=small%20OR%20-fleet%20OR%20value%3E80k",
            &admin,
        )
        .await;
    assert_eq!(count(&either.body), 2);
    let injection = app
        .get(
            "/api/v1/deals?filter=%22%27%3B%20DROP%20TABLE%20deals%3B--%22",
            &admin,
        )
        .await;
    injection.assert_status(StatusCode::OK);
    assert_eq!(count(&injection.body), 0);

    let board = app.get("/api/v1/deals/board?filter=pilot", &admin).await;
    let on_board: i64 = board
        .body
        .as_array()
        .unwrap()
        .iter()
        .map(|c| c["count"].as_i64().unwrap())
        .sum();
    assert_eq!(on_board, 2);

    let bad = app
        .get("/api/v1/deals?filter=fleet%20valu%3E5", &admin)
        .await;
    bad.assert_status(StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(bad.error_code(), "invalid_filter");
    assert_eq!(bad.body["error"]["span"], json!([6, 10]));
}

#[sqlx::test(migrator = "dealflow_api::MIGRATOR")]
async fn saved_views_are_private_unless_shared(pool: PgPool) {
    let app = TestApp::new(pool);
    let admin = app.register("Acme", "owner@acme.test").await;
    let rep = app.add_user(&admin, "rep@acme.test", "sales_rep").await;

    let private = app
        .post(
            "/api/v1/views",
            &rep,
            json!({ "name": "Big", "filter": "value>50k" }),
        )
        .await;
    private.assert_status(StatusCode::CREATED);
    app.post(
        "/api/v1/views",
        &rep,
        json!({ "name": "Big", "filter": "value>10k" }),
    )
    .await
    .assert_status(StatusCode::UNPROCESSABLE_ENTITY);
    app.post(
        "/api/v1/views",
        &rep,
        json!({ "name": "Broken", "filter": "valu>5" }),
    )
    .await
    .assert_status(StatusCode::UNPROCESSABLE_ENTITY);
    let shared = app
        .post(
            "/api/v1/views",
            &admin,
            json!({ "name": "Team closing", "filter": "closes<30d", "shared": true }),
        )
        .await;

    let seen_by_admin = app.get("/api/v1/views", &admin).await;
    assert_eq!(seen_by_admin.body.as_array().unwrap().len(), 1);
    let seen_by_rep = app.get("/api/v1/views", &rep).await;
    let names: Vec<&str> = seen_by_rep
        .body
        .as_array()
        .unwrap()
        .iter()
        .map(|v| v["name"].as_str().unwrap())
        .collect();
    assert_eq!(names, ["Big", "Team closing"], "own views first");

    app.delete(&format!("/api/v1/views/{}", shared.id()), &rep)
        .await
        .assert_status(StatusCode::FORBIDDEN);
    app.delete(&format!("/api/v1/views/{}", private.id()), &admin)
        .await
        .assert_status(StatusCode::NOT_FOUND);
    app.delete(&format!("/api/v1/views/{}", private.id()), &rep)
        .await
        .assert_status(StatusCode::NO_CONTENT);
}

#[sqlx::test(migrator = "dealflow_api::MIGRATOR")]
async fn automations_act_inside_the_change_that_triggers_them(pool: PgPool) {
    let app = TestApp::new(pool);
    let admin = app.register("Acme", "owner@acme.test").await;
    let rep = app.add_user(&admin, "rep@acme.test", "sales_rep").await;
    let proposal = app.stage_id(&admin, "Proposal").await;

    let rule = json!({
        "name": "Big proposals get a follow-up",
        "trigger": { "type": "stage_entered", "stage_id": proposal },
        "condition": "value>=50k",
        "actions": [
            { "type": "create_task", "title": "Chase {deal} at {company}", "due_in_days": 2, "assignee_id": null },
            { "type": "add_note", "body": "{deal} reached {stage}" },
            { "type": "set_close_date", "days_from_now": 30 }
        ]
    });
    app.post("/api/v1/automations", &rep, rule.clone())
        .await
        .assert_status(StatusCode::FORBIDDEN);
    app.post("/api/v1/automations", &admin, rule)
        .await
        .assert_status(StatusCode::CREATED);

    let big = app
        .create_deal(&rep, json!({ "title": "Fleet", "value_cents": 9_000_000 }))
        .await;
    let small = app
        .create_deal(&rep, json!({ "title": "Pilot", "value_cents": 100_000 }))
        .await;
    let moved = app.move_deal(&rep, &big.body, proposal).await;
    moved.assert_status(StatusCode::OK);
    assert!(
        moved.body["expected_close_date"].is_string(),
        "{}",
        moved.body
    );
    app.move_deal(&rep, &small.body, proposal)
        .await
        .assert_status(StatusCode::OK);

    let tasks = app
        .get(&format!("/api/v1/tasks?deal_id={}", big.id()), &rep)
        .await;
    assert_eq!(tasks.body["total"], 1);
    assert_eq!(tasks.body["data"][0]["title"], "Chase Fleet at no company");
    assert_eq!(
        tasks.body["data"][0]["assignee_id"],
        rep.user_id.to_string()
    );
    let notes = app
        .get(&format!("/api/v1/notes?deal_id={}", big.id()), &rep)
        .await;
    assert_eq!(notes.body["data"][0]["body"], "Fleet reached Proposal");
    let untouched = app
        .get(&format!("/api/v1/tasks?deal_id={}", small.id()), &rep)
        .await;
    assert_eq!(untouched.body["total"], 0, "the condition filters deals");

    let runs = app.get("/api/v1/automations/runs", &admin).await;
    let runs = runs.body.as_array().unwrap();
    assert_eq!(runs.len(), 1);
    assert_eq!(runs[0]["succeeded"], true);
    assert!(runs[0]["detail"]
        .as_str()
        .unwrap()
        .starts_with("created the follow-up"));
    let rules = app.get("/api/v1/automations", &admin).await;
    assert_eq!(rules.body[0]["runs_7d"], 1);
}

#[sqlx::test(migrator = "dealflow_api::MIGRATOR")]
async fn a_failing_automation_never_blocks_the_change(pool: PgPool) {
    let app = TestApp::new(pool);
    let admin = app.register("Acme", "owner@acme.test").await;
    let helper = app.add_user(&admin, "helper@acme.test", "sales_rep").await;

    app.post(
        "/api/v1/automations",
        &admin,
        json!({
            "name": "Hand new deals to the helper",
            "trigger": { "type": "deal_created" },
            "actions": [{ "type": "assign_owner", "user_id": helper.user_id }]
        }),
    )
    .await
    .assert_status(StatusCode::CREATED);
    app.post(
        "/api/v1/automations",
        &admin,
        json!({
            "name": "Welcome note",
            "trigger": { "type": "deal_created" },
            "actions": [{ "type": "add_note", "body": "New deal" }]
        }),
    )
    .await
    .assert_status(StatusCode::CREATED);
    app.put(
        &format!("/api/v1/users/{}", helper.user_id),
        &admin,
        json!({ "full_name": "Helper", "role": "sales_rep", "is_active": false }),
    )
    .await
    .assert_status(StatusCode::OK);

    let deal = app.create_deal(&admin, json!({ "title": "Fleet" })).await;
    assert_eq!(deal.body["owner_id"], admin.user_id.to_string());
    let notes = app
        .get(&format!("/api/v1/notes?deal_id={}", deal.id()), &admin)
        .await;
    assert_eq!(notes.body["total"], 1, "the next rule still ran");

    let runs = app.get("/api/v1/automations/runs", &admin).await;
    let failed: Vec<_> = runs
        .body
        .as_array()
        .unwrap()
        .iter()
        .filter(|r| r["succeeded"] == false)
        .collect();
    assert_eq!(failed.len(), 1);
    assert_eq!(failed[0]["detail"], "active user not found");
}

#[sqlx::test(migrator = "dealflow_api::MIGRATOR")]
async fn automations_validate_their_parts(pool: PgPool) {
    let app = TestApp::new(pool);
    let admin = app.register("Acme", "owner@acme.test").await;
    let other = app.register("Globex", "owner@globex.test").await;
    let foreign_stage = app.stage_id(&other, "Proposal").await;
    let note = json!([{ "type": "add_note", "body": "x" }]);

    let bad_filter = app
        .post(
            "/api/v1/automations",
            &admin,
            json!({ "name": "x", "trigger": { "type": "deal_created" }, "condition": "valu>1", "actions": note }),
        )
        .await;
    assert_eq!(bad_filter.error_code(), "invalid_filter");
    let foreign = app
        .post(
            "/api/v1/automations",
            &admin,
            json!({ "name": "x", "trigger": { "type": "stage_entered", "stage_id": foreign_stage }, "actions": note }),
        )
        .await;
    assert_eq!(foreign.error_fields(), ["trigger"]);
    let late = app
        .post(
            "/api/v1/automations",
            &admin,
            json!({ "name": "x", "trigger": { "type": "deal_created" },
                    "actions": [{ "type": "create_task", "title": "t", "due_in_days": 999, "assignee_id": null }] }),
        )
        .await;
    assert_eq!(late.error_fields(), ["actions"]);
    let none = app
        .post(
            "/api/v1/automations",
            &admin,
            json!({ "name": "x", "trigger": { "type": "deal_created" }, "actions": [] }),
        )
        .await;
    assert_eq!(none.error_fields(), ["actions"]);
}

#[sqlx::test(migrator = "dealflow_api::MIGRATOR")]
async fn completing_a_follow_up_can_trigger_a_rule(pool: PgPool) {
    let app = TestApp::new(pool);
    let admin = app.register("Acme", "owner@acme.test").await;
    let rule = app
        .post(
            "/api/v1/automations",
            &admin,
            json!({
                "name": "Plan the next step",
                "trigger": { "type": "task_completed" },
                "actions": [{ "type": "create_task", "title": "Next step for {deal}", "due_in_days": 1, "assignee_id": null }]
            }),
        )
        .await;
    let deal = app.create_deal(&admin, json!({ "title": "Fleet" })).await;
    let task = app
        .post(
            "/api/v1/tasks",
            &admin,
            json!({ "title": "Call", "deal_id": deal.id() }),
        )
        .await;
    app.post(
        &format!("/api/v1/tasks/{}/complete", task.id()),
        &admin,
        json!({}),
    )
    .await
    .assert_status(StatusCode::OK);
    let open = app
        .get(
            &format!("/api/v1/tasks?deal_id={}&status=open", deal.id()),
            &admin,
        )
        .await;
    assert_eq!(open.body["data"][0]["title"], "Next step for Fleet");

    let mut disabled = rule.body.clone();
    disabled["enabled"] = json!(false);
    app.put(
        &format!("/api/v1/automations/{}", rule.id()),
        &admin,
        disabled,
    )
    .await
    .assert_status(StatusCode::OK);
    let audit = app.get("/api/v1/audit?entity_type=rule", &admin).await;
    assert_eq!(audit.body["total"], 2);
}

#[sqlx::test(migrator = "dealflow_api::MIGRATOR")]
async fn duplicate_contacts_merge_with_everything_attached(pool: PgPool) {
    let app = TestApp::new(pool);
    let admin = app.register("Acme", "owner@acme.test").await;
    let keep = app
        .post(
            "/api/v1/contacts",
            &admin,
            json!({ "first_name": "Emma", "last_name": "Novak", "email": "emma@harbor.example" }),
        )
        .await;
    let twin = app
        .post(
            "/api/v1/contacts",
            &admin,
            json!({ "first_name": "Emma", "last_name": "Novack", "email": "Emma@Harbor.example",
                    "phone": "+1 555 0100" }),
        )
        .await;
    app.post(
        "/api/v1/contacts",
        &admin,
        json!({ "first_name": "Liam", "last_name": "Stone" }),
    )
    .await;
    let deal = app
        .create_deal(&admin, json!({ "title": "Fleet", "contact_id": twin.id() }))
        .await;
    app.post(
        "/api/v1/tasks",
        &admin,
        json!({ "title": "Call Emma", "contact_id": twin.id() }),
    )
    .await
    .assert_status(StatusCode::CREATED);
    app.post(
        "/api/v1/notes",
        &admin,
        json!({ "body": "Prefers email", "contact_id": twin.id() }),
    )
    .await
    .assert_status(StatusCode::CREATED);

    let pairs = app.get("/api/v1/duplicates/contacts", &admin).await;
    let pairs = pairs.body.as_array().unwrap();
    assert_eq!(pairs.len(), 1, "{pairs:?}");
    assert_eq!(pairs[0]["same_key"], true);

    app.post(
        "/api/v1/duplicates/contacts/merge",
        &admin,
        json!({ "keep": keep.id(), "remove": twin.id(), "take": ["password_hash"] }),
    )
    .await
    .assert_status(StatusCode::UNPROCESSABLE_ENTITY);
    let merged = app
        .post(
            "/api/v1/duplicates/contacts/merge",
            &admin,
            json!({ "keep": keep.id(), "remove": twin.id(), "take": ["last_name"] }),
        )
        .await;
    merged.assert_status(StatusCode::OK);
    assert_eq!(merged.body["kept"]["last_name"], "Novack");
    assert_eq!(
        merged.body["kept"]["phone"], "+1 555 0100",
        "blanks are filled"
    );
    assert_eq!(
        merged.body["moved"],
        json!({ "deals": 1, "tasks": 1, "notes": 1 })
    );

    app.get(&format!("/api/v1/contacts/{}", twin.id()), &admin)
        .await
        .assert_status(StatusCode::NOT_FOUND);
    let deal = app
        .get(&format!("/api/v1/deals/{}", deal.id()), &admin)
        .await;
    assert_eq!(deal.body["contact_id"], keep.id().to_string());
    let notes = app
        .get(&format!("/api/v1/notes?contact_id={}", keep.id()), &admin)
        .await;
    assert_eq!(notes.body["total"], 1);
    let history = app
        .get(
            &format!("/api/v1/audit?entity_type=contact&entity_id={}", keep.id()),
            &admin,
        )
        .await;
    assert_eq!(history.body["data"][0]["action"], "updated");
    let empty = app.get("/api/v1/duplicates/contacts", &admin).await;
    assert_eq!(empty.body, json!([]));
}

#[sqlx::test(migrator = "dealflow_api::MIGRATOR")]
async fn similar_company_names_pair_up_and_merges_respect_ownership(pool: PgPool) {
    let app = TestApp::new(pool);
    let admin = app.register("Acme", "owner@acme.test").await;
    let rep = app.add_user(&admin, "rep@acme.test", "sales_rep").await;
    let first = app
        .post(
            "/api/v1/companies",
            &admin,
            json!({ "name": "Harbor Logistics" }),
        )
        .await;
    let second = app
        .post(
            "/api/v1/companies",
            &rep,
            json!({ "name": "Harbor Logistic" }),
        )
        .await;
    app.post(
        "/api/v1/companies",
        &admin,
        json!({ "name": "Granite Foods" }),
    )
    .await;

    let pairs = app.get("/api/v1/duplicates/companies", &rep).await;
    let pairs = pairs.body.as_array().unwrap();
    assert_eq!(pairs.len(), 1);
    assert_eq!(pairs[0]["same_key"], false);
    assert!(pairs[0]["similarity"].as_f64().unwrap() > 0.55);

    let body = json!({ "keep": second.id(), "remove": first.id() });
    app.post("/api/v1/duplicates/companies/merge", &rep, body.clone())
        .await
        .assert_status(StatusCode::FORBIDDEN);
    app.post(
        "/api/v1/duplicates/companies/merge",
        &admin,
        json!({ "keep": first.id(), "remove": first.id() }),
    )
    .await
    .assert_status(StatusCode::UNPROCESSABLE_ENTITY);
    app.post("/api/v1/duplicates/companies/merge", &admin, body)
        .await
        .assert_status(StatusCode::OK);
    app.get("/api/v1/duplicates/users", &admin)
        .await
        .assert_status(StatusCode::NOT_FOUND);
}
