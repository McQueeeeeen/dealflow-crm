use anyhow::Context;
use chrono::{DateTime, Datelike, Days, Duration, NaiveDate, Timelike, Utc};
use dealflow_api::auth::password::hash_password;
use dealflow_api::models::{Role, StageKind};
use dealflow_api::MIGRATOR;
use rand::rngs::StdRng;
use rand::seq::SliceRandom;
use rand::{Rng, SeedableRng};
use sqlx::postgres::PgPoolOptions;
use sqlx::{PgConnection, Postgres, Transaction};
use uuid::Uuid;

const DEMO_EMAIL: &str = "demo@dealflow.dev";
const DEMO_PASSWORD: &str = "dealflow-demo";

const TEAM: [(&str, &str, Role); 5] = [
    ("Alex Morgan", DEMO_EMAIL, Role::Admin),
    ("Priya Raman", "priya@dealflow.dev", Role::Manager),
    ("Jordan Lee", "jordan@dealflow.dev", Role::SalesRep),
    ("Sofia Martins", "sofia@dealflow.dev", Role::SalesRep),
    ("Daniel Okafor", "daniel@dealflow.dev", Role::SalesRep),
];

const STAGES: [(&str, i16, StageKind); 6] = [
    ("Lead", 10, StageKind::Open),
    ("Qualified", 25, StageKind::Open),
    ("Proposal", 50, StageKind::Open),
    ("Negotiation", 75, StageKind::Open),
    ("Won", 100, StageKind::Won),
    ("Lost", 0, StageKind::Lost),
];

const COMPANY_PREFIXES: [&str; 16] = [
    "Harbor",
    "Granite",
    "Cedar",
    "Northwind",
    "Bluebell",
    "Ironwood",
    "Silverline",
    "Maple",
    "Riverstone",
    "Summit",
    "Lighthouse",
    "Copperleaf",
    "Westfield",
    "Oakridge",
    "Brightwater",
    "Fieldstone",
];
const COMPANY_SUFFIXES: [&str; 10] = [
    "Logistics",
    "Foods",
    "Clinics",
    "Robotics",
    "Hotels",
    "Builders",
    "Analytics",
    "Textiles",
    "Energy",
    "Media",
];
const INDUSTRIES: [&str; 8] = [
    "Logistics",
    "Food & beverage",
    "Healthcare",
    "Manufacturing",
    "Hospitality",
    "Construction",
    "Software",
    "Retail",
];
const FIRST_NAMES: [&str; 20] = [
    "Emma", "Liam", "Olivia", "Noah", "Ava", "Mateo", "Mia", "Lucas", "Amara", "Ethan", "Leila",
    "Kenji", "Nora", "Omar", "Ingrid", "Tomas", "Zara", "Felix", "Hana", "Rafael",
];
const LAST_NAMES: [&str; 20] = [
    "Novak",
    "Fischer",
    "Costa",
    "Haddad",
    "Kowalski",
    "Nakamura",
    "Silva",
    "Brennan",
    "Dubois",
    "Moreau",
    "Lindqvist",
    "Petrov",
    "Rossi",
    "Andersen",
    "Varga",
    "Kim",
    "Mendes",
    "Walsh",
    "Ibrahim",
    "Sato",
];
const JOB_TITLES: [&str; 7] = [
    "Head of Operations",
    "Procurement Manager",
    "CFO",
    "IT Director",
    "COO",
    "Office Manager",
    "VP of Sales",
];
const DEAL_KINDS: [&str; 8] = [
    "Annual license",
    "Pilot rollout",
    "Fleet expansion",
    "Support renewal",
    "Multi-site upgrade",
    "Onboarding package",
    "Data migration",
    "Seat expansion",
];
const TASK_TITLES: [&str; 8] = [
    "Send revised proposal",
    "Schedule demo with the operations team",
    "Follow up on pricing questions",
    "Prepare contract draft",
    "Call to confirm budget",
    "Share case study",
    "Book onsite visit",
    "Check in after the trial",
];
const NOTE_BODIES: [&str; 6] = [
    "Budget is approved for this quarter. Legal review is the remaining blocker.",
    "They compared us with two other vendors. Price is fine, onboarding time matters most.",
    "Decision maker is the COO. Our champion will set up a call next week.",
    "Asked for a reference customer in the same industry.",
    "Trial went well, three teams are using it daily.",
    "Procurement wants net-60 payment terms.",
];

struct Ids {
    org: Uuid,
    users: Vec<Uuid>,
    stages: Vec<(Uuid, StageKind)>,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let database_url = std::env::var("DATABASE_URL").context("DATABASE_URL must be set")?;
    let pool = PgPoolOptions::new()
        .max_connections(2)
        .connect(&database_url)
        .await?;
    MIGRATOR.run(&pool).await?;

    let mut rng = StdRng::seed_from_u64(2026);
    let mut tx = pool.begin().await?;
    for table in ["deal_stage_changes", "audit_log", "organizations"] {
        let column = if table == "organizations" {
            "id"
        } else {
            "organization_id"
        };
        sqlx::query(&format!(
            "DELETE FROM {table} WHERE {column} IN (SELECT organization_id FROM users WHERE email = $1)"
        ))
        .bind(DEMO_EMAIL)
        .execute(&mut *tx)
        .await?;
    }

    let ids = seed_workspace(&mut tx).await?;
    let companies = seed_companies(&mut tx, &ids, &mut rng).await?;
    let contacts = seed_contacts(&mut tx, &ids, &companies, &mut rng).await?;
    let deals = seed_deals(&mut tx, &ids, &contacts, &mut rng).await?;
    seed_tasks_and_notes(&mut tx, &ids, &deals, &mut rng).await?;
    seed_targets(&mut tx, &ids, &mut rng).await?;
    seed_automations(&mut tx, &ids, &deals).await?;
    seed_duplicates(&mut tx, &ids).await?;
    tx.commit().await?;

    println!(
        "Seeded {} companies, {} contacts, {} deals. Sign in as {DEMO_EMAIL} / {DEMO_PASSWORD}",
        companies.len(),
        contacts.len(),
        deals.len()
    );
    Ok(())
}

async fn seed_workspace(tx: &mut Transaction<'_, Postgres>) -> anyhow::Result<Ids> {
    let org = Uuid::now_v7();
    sqlx::query(
        "INSERT INTO organizations (id, name, currency) VALUES ($1, 'Demo workspace', 'USD')",
    )
    .bind(org)
    .execute(&mut **tx)
    .await?;

    let hash = hash_password(DEMO_PASSWORD)?;
    let mut users = Vec::new();
    for (name, email, role) in TEAM {
        let id = Uuid::now_v7();
        sqlx::query(
            "INSERT INTO users (id, organization_id, email, full_name, password_hash, role)
             VALUES ($1, $2, $3, $4, $5, $6)",
        )
        .bind(id)
        .bind(org)
        .bind(email)
        .bind(name)
        .bind(&hash)
        .bind(role)
        .execute(&mut **tx)
        .await?;
        users.push(id);
    }

    let mut stages = Vec::new();
    for (position, (name, probability, kind)) in STAGES.into_iter().enumerate() {
        let id = Uuid::now_v7();
        sqlx::query(
            "INSERT INTO pipeline_stages (id, organization_id, name, position, probability, kind)
             VALUES ($1, $2, $3, $4, $5, $6)",
        )
        .bind(id)
        .bind(org)
        .bind(name)
        .bind(position as i32)
        .bind(probability)
        .bind(kind)
        .execute(&mut **tx)
        .await?;
        stages.push((id, kind));
    }
    Ok(Ids { org, users, stages })
}

async fn seed_duplicates(tx: &mut Transaction<'_, Postgres>, ids: &Ids) -> anyhow::Result<()> {
    for sql in [
        "INSERT INTO contacts (id, organization_id, first_name, last_name, email, phone, company_id,
                               owner_id, created_at, updated_at)
         SELECT $2, organization_id, first_name, last_name || 's', email, '+1 555 0142', company_id,
                owner_id, now() - interval '20 days', now() - interval '20 days'
         FROM contacts WHERE organization_id = $1 AND email IS NOT NULL ORDER BY id LIMIT 1",
        "INSERT INTO contacts (id, organization_id, first_name, last_name, job_title, owner_id,
                               created_at, updated_at)
         SELECT $2, organization_id, first_name, last_name, 'Procurement lead', owner_id,
                now() - interval '9 days', now() - interval '9 days'
         FROM contacts WHERE organization_id = $1 ORDER BY id OFFSET 3 LIMIT 1",
        "INSERT INTO companies (id, organization_id, name, domain, owner_id, created_at, updated_at)
         SELECT $2, organization_id, name || ' Inc', domain, owner_id,
                now() - interval '15 days', now() - interval '15 days'
         FROM companies WHERE organization_id = $1 AND domain IS NOT NULL ORDER BY id LIMIT 1",
    ] {
        sqlx::query(sql)
            .bind(ids.org)
            .bind(Uuid::now_v7())
            .execute(&mut **tx)
            .await?;
    }
    Ok(())
}

async fn seed_automations(
    tx: &mut Transaction<'_, Postgres>,
    ids: &Ids,
    deals: &[(Uuid, Uuid)],
) -> anyhow::Result<()> {
    let (proposal, won) = (ids.stages[2].0, ids.stages[4].0);
    let rules = [
        (
            "Big proposals get a follow-up",
            serde_json::json!({ "type": "stage_entered", "stage_id": proposal }),
            "value>=100k",
            serde_json::json!([
                { "type": "create_task", "title": "Walk {company} through the proposal for {deal}",
                  "due_in_days": 2, "assignee_id": null },
                { "type": "add_note", "body": "{deal} reached {stage} at {value}. Proposal call booked automatically." }
            ]),
        ),
        (
            "Kick off won deals",
            serde_json::json!({ "type": "stage_entered", "stage_id": won }),
            "",
            serde_json::json!([
                { "type": "create_task", "title": "Kickoff call with {company}", "due_in_days": 3,
                  "assignee_id": null }
            ]),
        ),
        (
            "Always plan the next step",
            serde_json::json!({ "type": "task_completed" }),
            "status:open",
            serde_json::json!([
                { "type": "create_task", "title": "Plan the next step for {deal}", "due_in_days": 1,
                  "assignee_id": null }
            ]),
        ),
    ];
    let mut rule_ids = Vec::new();
    for (name, trigger, condition, actions) in rules {
        let id = Uuid::now_v7();
        sqlx::query(
            "INSERT INTO automation_rules (id, organization_id, name, trigger, condition, actions, created_by)
             VALUES ($1, $2, $3, $4, $5, $6, $7)",
        )
        .bind(id)
        .bind(ids.org)
        .bind(name)
        .bind(trigger)
        .bind(condition)
        .bind(actions)
        .bind(ids.users[0])
        .execute(&mut **tx)
        .await?;
        rule_ids.push(id);
    }
    for (n, (deal, _)) in deals.iter().take(6).enumerate() {
        let failed = n == 4;
        sqlx::query(
            "INSERT INTO automation_runs (organization_id, rule_id, deal_id, succeeded, detail, created_at)
             VALUES ($1, $2, $3, $4, $5, now() - make_interval(hours => $6))",
        )
        .bind(ids.org)
        .bind(rule_ids[n % 3])
        .bind(deal)
        .bind(!failed)
        .bind(if failed {
            "active user not found"
        } else {
            "created the follow-up"
        })
        .bind(i32::try_from(n * 7 + 1)?)
        .execute(&mut **tx)
        .await?;
    }
    Ok(())
}

async fn seed_targets(
    tx: &mut Transaction<'_, Postgres>,
    ids: &Ids,
    rng: &mut StdRng,
) -> anyhow::Result<()> {
    for &user in &ids.users {
        let (won, weighted): (i64, i64) = sqlx::query_as(
            "SELECT coalesce(sum(d.value_cents) FILTER (
                        WHERE s.kind = 'won' AND d.closed_at >= date_trunc('month', now())), 0)::bigint,
                    coalesce(sum(d.value_cents * s.probability / 100) FILTER (
                        WHERE s.kind = 'open'
                          AND d.expected_close_date < date_trunc('month', now()) + interval '1 month'), 0)::bigint
             FROM deals d JOIN pipeline_stages s ON s.id = d.stage_id
             WHERE d.organization_id = $1 AND d.owner_id = $2",
        )
        .bind(ids.org)
        .bind(user)
        .fetch_one(&mut **tx)
        .await?;
        let stretch = (won as f64 + weighted as f64 * rng.gen_range(0.25..0.55)) / 500_000.0;
        sqlx::query(
            "INSERT INTO sales_targets (organization_id, user_id, month, amount_cents)
             VALUES ($1, $2, date_trunc('month', now())::date, $3)",
        )
        .bind(ids.org)
        .bind(user)
        .bind((stretch.round() as i64 * 500_000).max(5_000_000))
        .execute(&mut **tx)
        .await?;
    }
    Ok(())
}

fn days_ago(days: i64) -> DateTime<Utc> {
    Utc::now() - Duration::days(days)
}

async fn seed_companies(
    tx: &mut Transaction<'_, Postgres>,
    ids: &Ids,
    rng: &mut StdRng,
) -> anyhow::Result<Vec<(Uuid, Uuid)>> {
    let mut companies = Vec::new();
    let mut names: Vec<(&str, &str)> = COMPANY_PREFIXES
        .iter()
        .flat_map(|p| COMPANY_SUFFIXES.iter().map(move |s| (*p, *s)))
        .collect();
    names.shuffle(rng);

    for (prefix, suffix) in names.into_iter().take(32) {
        let id = Uuid::now_v7();
        let owner = *ids.users.choose(rng).expect("team is not empty");
        let created = days_ago(rng.gen_range(200..400));
        sqlx::query(
            "INSERT INTO companies (id, organization_id, name, domain, industry, owner_id, created_at, updated_at)
             VALUES ($1, $2, $3, $4, $5, $6, $7, $7)",
        )
        .bind(id)
        .bind(ids.org)
        .bind(format!("{prefix} {suffix}"))
        .bind(format!("{}{}.example", prefix.to_lowercase(), suffix.to_lowercase()))
        .bind(*INDUSTRIES.choose(rng).expect("non-empty"))
        .bind(owner)
        .bind(created)
        .execute(&mut **tx)
        .await?;
        companies.push((id, owner));
    }
    Ok(companies)
}

async fn seed_contacts(
    tx: &mut Transaction<'_, Postgres>,
    ids: &Ids,
    companies: &[(Uuid, Uuid)],
    rng: &mut StdRng,
) -> anyhow::Result<Vec<(Uuid, Uuid, Uuid)>> {
    let mut contacts = Vec::new();
    for &(company, owner) in companies {
        for _ in 0..rng.gen_range(1..=3) {
            let id = Uuid::now_v7();
            let first = *FIRST_NAMES.choose(rng).expect("non-empty");
            let last = *LAST_NAMES.choose(rng).expect("non-empty");
            sqlx::query(
                "INSERT INTO contacts (id, organization_id, first_name, last_name, email, phone,
                                       job_title, company_id, owner_id)
                 VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)",
            )
            .bind(id)
            .bind(ids.org)
            .bind(first)
            .bind(last)
            .bind(format!(
                "{}.{}@example.com",
                first.to_lowercase(),
                last.to_lowercase()
            ))
            .bind(format!("+1 555 01{:02}", rng.gen_range(0..100)))
            .bind(*JOB_TITLES.choose(rng).expect("non-empty"))
            .bind(company)
            .bind(owner)
            .execute(&mut **tx)
            .await?;
            contacts.push((id, company, owner));
        }
    }
    Ok(contacts)
}

async fn seed_deals(
    tx: &mut Transaction<'_, Postgres>,
    ids: &Ids,
    contacts: &[(Uuid, Uuid, Uuid)],
    rng: &mut StdRng,
) -> anyhow::Result<Vec<(Uuid, Uuid)>> {
    let open: Vec<Uuid> = stage_ids(ids, StageKind::Open);
    let won = stage_ids(ids, StageKind::Won)[0];
    let lost = stage_ids(ids, StageKind::Lost)[0];
    let now = Utc::now();
    let month_start = now
        - Duration::days(i64::from(now.day0()))
        - Duration::seconds(i64::from(now.num_seconds_from_midnight()));
    let hours_into_month = (now - month_start).num_hours().max(2);
    let mut deals = Vec::new();

    for n in 0..72 {
        let &(contact, company, owner) = contacts.choose(rng).expect("contacts exist");
        let created = days_ago(rng.gen_range(5..270));
        let mut path = vec![open[0]];
        let mut at = created;
        let mut changes = vec![(None, open[0], at)];

        for &next in &open[1..] {
            if !rng.gen_bool(0.68) {
                break;
            }
            at = (at + Duration::days(rng.gen_range(4..25))).min(Utc::now() - Duration::hours(2));
            changes.push((path.last().copied(), next, at));
            path.push(next);
        }

        let age_days = (Utc::now() - created).num_days();
        let reached_end = path.len() == open.len();
        let outcome = if age_days > 45 && rng.gen_bool(if reached_end { 0.75 } else { 0.4 }) {
            Some(if reached_end && rng.gen_bool(0.7) {
                won
            } else {
                lost
            })
        } else {
            None
        };
        let mut closed_at = None;
        if let Some(final_stage) = outcome {
            at = (at + Duration::days(rng.gen_range(3..20))).min(Utc::now() - Duration::hours(1));
            if final_stage == won && rng.gen_bool(0.3) {
                at = at.max(month_start + Duration::hours(rng.gen_range(1..hours_into_month)));
            }
            changes.push((path.last().copied(), final_stage, at));
            path.push(final_stage);
            closed_at = Some(at);
        }
        let stage = *path.last().expect("path starts with a stage");
        let expected_close: Option<NaiveDate> = match outcome {
            None => {
                let days = if rng.gen_bool(0.3) { 0..10 } else { 10..150 };
                let date = Utc::now().date_naive() + Days::new(rng.gen_range(days));
                let date = if rng.gen_bool(0.1) {
                    date - Days::new(60)
                } else {
                    date
                };
                rng.gen_bool(0.85).then_some(date)
            }
            Some(_) => closed_at.map(|c| c.date_naive()),
        };

        let id = Uuid::now_v7();
        let value = rng.gen_range(18..=480) * 50_000_i64;
        sqlx::query(
            "INSERT INTO deals (id, organization_id, title, value_cents, stage_id, board_position,
                                company_id, contact_id, owner_id, expected_close_date, closed_at,
                                created_at, updated_at)
             VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13)",
        )
        .bind(id)
        .bind(ids.org)
        .bind(format!(
            "{} #{}",
            DEAL_KINDS.choose(rng).expect("non-empty"),
            1040 + n
        ))
        .bind(value)
        .bind(stage)
        .bind(f64::from(n) * 1024.0)
        .bind(company)
        .bind(contact)
        .bind(owner)
        .bind(expected_close)
        .bind(closed_at)
        .bind(created)
        .bind(if outcome.is_none() {
            at.max(days_ago(rng.gen_range(1..24)))
        } else {
            at
        })
        .execute(&mut **tx)
        .await?;

        for (from, to, when) in changes {
            insert_stage_change(tx, ids.org, id, from, to, owner, when).await?;
        }
        deals.push((id, owner));
    }
    Ok(deals)
}

fn stage_ids(ids: &Ids, kind: StageKind) -> Vec<Uuid> {
    ids.stages
        .iter()
        .filter(|(_, k)| *k == kind)
        .map(|(id, _)| *id)
        .collect()
}

async fn insert_stage_change(
    tx: &mut PgConnection,
    org: Uuid,
    deal: Uuid,
    from: Option<Uuid>,
    to: Uuid,
    by: Uuid,
    at: DateTime<Utc>,
) -> anyhow::Result<()> {
    sqlx::query(
        "INSERT INTO deal_stage_changes (organization_id, deal_id, from_stage_id, to_stage_id, changed_by, changed_at)
         VALUES ($1, $2, $3, $4, $5, $6)",
    )
    .bind(org)
    .bind(deal)
    .bind(from)
    .bind(to)
    .bind(by)
    .bind(at)
    .execute(tx)
    .await?;
    Ok(())
}

async fn seed_tasks_and_notes(
    tx: &mut Transaction<'_, Postgres>,
    ids: &Ids,
    deals: &[(Uuid, Uuid)],
    rng: &mut StdRng,
) -> anyhow::Result<()> {
    for &(deal, owner) in deals.iter().step_by(2) {
        let due = Utc::now() + Duration::hours(rng.gen_range(-96..240));
        let completed = (due < Utc::now() && rng.gen_bool(0.6)).then(|| due - Duration::hours(2));
        sqlx::query(
            "INSERT INTO tasks (id, organization_id, title, due_at, completed_at, assignee_id,
                                created_by, deal_id)
             VALUES ($1, $2, $3, $4, $5, $6, $6, $7)",
        )
        .bind(Uuid::now_v7())
        .bind(ids.org)
        .bind(*TASK_TITLES.choose(rng).expect("non-empty"))
        .bind(due)
        .bind(completed)
        .bind(owner)
        .bind(deal)
        .execute(&mut **tx)
        .await?;
    }

    for &(deal, owner) in deals.iter().step_by(3) {
        sqlx::query(
            "INSERT INTO notes (id, organization_id, author_id, body, deal_id, created_at)
             VALUES ($1, $2, $3, $4, $5, $6)",
        )
        .bind(Uuid::now_v7())
        .bind(ids.org)
        .bind(owner)
        .bind(*NOTE_BODIES.choose(rng).expect("non-empty"))
        .bind(deal)
        .bind(days_ago(rng.gen_range(1..30)))
        .execute(&mut **tx)
        .await?;
    }
    Ok(())
}
