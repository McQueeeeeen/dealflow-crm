use std::collections::HashMap;

use axum::extract::State;
use axum::http::header::{CONTENT_DISPOSITION, CONTENT_TYPE};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Json, Router};
use chrono::Utc;
use serde::{Deserialize, Serialize};
use sqlx::{FromRow, PgConnection, Postgres, Transaction};
use uuid::Uuid;

use crate::api::{companies, contacts};
use crate::audit;
use crate::auth::AuthUser;
use crate::error::{AppError, AppResult};
use crate::extract::{ApiJson, ApiPath};
use crate::models::{AuditAction, EntityType};
use crate::state::AppState;

const MAX_ROWS: usize = 1000;

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/export/{kind}", get(export))
        .route("/import/companies", post(import_companies))
        .route("/import/contacts", post(import_contacts))
}

const COMPANY_EXPORT: (&[&str], &str) = (
    &["Name", "Domain", "Industry", "Owner", "Created"],
    "SELECT ARRAY[c.name, c.domain, c.industry, u.full_name, to_char(c.created_at, 'YYYY-MM-DD')]
     FROM companies c JOIN users u ON u.id = c.owner_id
     WHERE c.organization_id = $1 ORDER BY c.name, c.id",
);

const CONTACT_EXPORT: (&[&str], &str) = (
    &["First name", "Last name", "Email", "Phone", "Job title", "Company", "Owner"],
    "SELECT ARRAY[ct.first_name, ct.last_name, ct.email, ct.phone, ct.job_title, co.name, u.full_name]
     FROM contacts ct JOIN users u ON u.id = ct.owner_id
     LEFT JOIN companies co ON co.id = ct.company_id
     WHERE ct.organization_id = $1 ORDER BY ct.first_name, ct.last_name, ct.id",
);

const DEAL_EXPORT: (&[&str], &str) = (
    &[
        "Title",
        "Value",
        "Stage",
        "Company",
        "Contact",
        "Owner",
        "Expected close",
        "Closed",
    ],
    "SELECT ARRAY[d.title, to_char(d.value_cents / 100.0, 'FM999999999999990.00'), s.name, co.name,
                  trim(ct.first_name || ' ' || ct.last_name), u.full_name,
                  d.expected_close_date::text, to_char(d.closed_at, 'YYYY-MM-DD')]
     FROM deals d
     JOIN pipeline_stages s ON s.id = d.stage_id
     JOIN users u ON u.id = d.owner_id
     LEFT JOIN companies co ON co.id = d.company_id
     LEFT JOIN contacts ct ON ct.id = d.contact_id
     WHERE d.organization_id = $1 ORDER BY s.position, d.board_position, d.id",
);

pub fn csv_cell(value: &str) -> String {
    let guarded = if value.starts_with(['=', '+', '-', '@', '\t', '\r']) {
        format!("'{value}")
    } else {
        value.to_owned()
    };
    if guarded.contains([',', '"', '\n', '\r']) {
        format!("\"{}\"", guarded.replace('"', "\"\""))
    } else {
        guarded
    }
}

fn csv_line<'a>(cells: impl Iterator<Item = Option<&'a str>>) -> String {
    let cells: Vec<String> = cells.map(|c| csv_cell(c.unwrap_or_default())).collect();
    cells.join(",") + "\r\n"
}

pub async fn export(
    State(state): State<AppState>,
    user: AuthUser,
    ApiPath(kind): ApiPath<String>,
) -> AppResult<Response> {
    let (header, sql) = match kind.as_str() {
        "companies" => COMPANY_EXPORT,
        "contacts" => CONTACT_EXPORT,
        "deals" => DEAL_EXPORT,
        _ => return Err(AppError::NotFound("export")),
    };
    let rows: Vec<Vec<Option<String>>> = sqlx::query_scalar(sql)
        .bind(user.org_id)
        .fetch_all(&state.pool)
        .await?;
    let mut body = String::from("\u{feff}");
    body += &csv_line(header.iter().map(|h| Some(*h)));
    for row in &rows {
        body += &csv_line(row.iter().map(Option::as_deref));
    }
    let filename = format!("dealflow-{kind}-{}.csv", Utc::now().date_naive());
    Ok((
        [
            (CONTENT_TYPE, "text/csv; charset=utf-8".to_owned()),
            (
                CONTENT_DISPOSITION,
                format!("attachment; filename=\"{filename}\""),
            ),
        ],
        body,
    )
        .into_response())
}

#[derive(Deserialize)]
pub struct ImportRequest<T> {
    pub rows: Vec<T>,
    #[serde(default)]
    pub dry_run: bool,
}

#[derive(Deserialize)]
pub struct CompanyRow {
    #[serde(default)]
    pub name: String,
    pub domain: Option<String>,
    pub industry: Option<String>,
}

#[derive(Deserialize)]
pub struct ContactRow {
    #[serde(default)]
    pub first_name: String,
    #[serde(default)]
    pub last_name: String,
    pub email: Option<String>,
    pub phone: Option<String>,
    pub job_title: Option<String>,
    pub company: Option<String>,
}

#[derive(Debug, Default, Serialize)]
pub struct ImportReport {
    pub dry_run: bool,
    pub created: usize,
    pub updated: usize,
    pub unchanged: usize,
    pub companies_created: usize,
    pub errors: Vec<RowError>,
}

#[derive(Debug, Serialize)]
pub struct RowError {
    pub row: usize,
    pub message: String,
}

#[derive(Serialize, FromRow)]
struct CompanySnapshot {
    name: String,
    domain: Option<String>,
    industry: Option<String>,
}

#[derive(Serialize, FromRow)]
struct ContactSnapshot {
    first_name: String,
    last_name: String,
    email: Option<String>,
    phone: Option<String>,
    job_title: Option<String>,
    company_id: Option<Uuid>,
}

enum Outcome {
    Created,
    Updated,
    Unchanged,
}

impl ImportReport {
    fn count(&mut self, outcome: Outcome) {
        match outcome {
            Outcome::Created => self.created += 1,
            Outcome::Updated => self.updated += 1,
            Outcome::Unchanged => self.unchanged += 1,
        }
    }

    fn reject(&mut self, index: usize, err: AppError) {
        let message = match err {
            AppError::Validation(fields) => fields
                .iter()
                .map(|f| format!("{} {}", f.field.replace('_', " "), f.message))
                .collect::<Vec<_>>()
                .join(", "),
            other => other.to_string(),
        };
        self.errors.push(RowError {
            row: index + 1,
            message,
        });
    }
}

fn check_size(rows: usize) -> AppResult<()> {
    if (1..=MAX_ROWS).contains(&rows) {
        Ok(())
    } else {
        Err(AppError::validation(
            "rows",
            &format!("must contain between 1 and {MAX_ROWS} rows"),
        ))
    }
}

async fn finish(
    tx: Transaction<'_, Postgres>,
    report: ImportReport,
) -> AppResult<Json<ImportReport>> {
    if report.dry_run {
        tx.rollback().await?;
    } else {
        tx.commit().await?;
    }
    Ok(Json(report))
}

pub async fn import_companies(
    State(state): State<AppState>,
    actor: AuthUser,
    ApiJson(req): ApiJson<ImportRequest<CompanyRow>>,
) -> AppResult<Json<ImportReport>> {
    check_size(req.rows.len())?;
    let mut report = ImportReport {
        dry_run: req.dry_run,
        ..ImportReport::default()
    };
    let mut tx = state.pool.begin().await?;
    for (index, row) in req.rows.into_iter().enumerate() {
        let input = companies::CompanyInput {
            name: row.name,
            domain: row.domain,
            industry: row.industry,
            owner_id: None,
        };
        let outcome = match companies::validate(input) {
            Ok((input, _)) => upsert_company(&mut tx, &actor, input).await?,
            Err(err) => Err(err),
        };
        match outcome {
            Ok(outcome) => report.count(outcome),
            Err(err) => report.reject(index, err),
        }
    }
    finish(tx, report).await
}

async fn find_company(
    conn: &mut PgConnection,
    org_id: Uuid,
    name: &str,
    domain: Option<&str>,
) -> AppResult<Option<(Uuid, Uuid)>> {
    Ok(sqlx::query_as(
        "SELECT id, owner_id FROM companies
         WHERE organization_id = $1 AND (($3::text IS NOT NULL AND domain = $3) OR lower(name) = lower($2))
         ORDER BY (domain = $3) DESC NULLS LAST, id LIMIT 1",
    )
    .bind(org_id)
    .bind(name)
    .bind(domain)
    .fetch_optional(conn)
    .await?)
}

async fn insert_company(
    conn: &mut PgConnection,
    actor: &AuthUser,
    input: &companies::CleanInput,
) -> AppResult<Uuid> {
    let id = Uuid::now_v7();
    let fields: CompanySnapshot = sqlx::query_as(
        "INSERT INTO companies (id, organization_id, name, domain, industry, owner_id)
         VALUES ($1, $2, $3, $4, $5, $6) RETURNING name, domain, industry",
    )
    .bind(id)
    .bind(actor.org_id)
    .bind(&input.name)
    .bind(&input.domain)
    .bind(&input.industry)
    .bind(actor.user_id)
    .fetch_one(&mut *conn)
    .await?;
    let snapshot = audit::snapshot(&fields);
    audit::record(
        conn,
        actor,
        EntityType::Company,
        id,
        AuditAction::Created,
        snapshot,
    )
    .await?;
    Ok(id)
}

async fn upsert_company(
    conn: &mut PgConnection,
    actor: &AuthUser,
    input: companies::CleanInput,
) -> AppResult<AppResult<Outcome>> {
    let Some((id, owner_id)) =
        find_company(conn, actor.org_id, &input.name, input.domain.as_deref()).await?
    else {
        insert_company(conn, actor, &input).await?;
        return Ok(Ok(Outcome::Created));
    };
    if !actor.can_modify(owner_id) {
        return Ok(Err(AppError::BadRequest(
            "matches a company owned by someone else".into(),
        )));
    }
    let before: CompanySnapshot =
        sqlx::query_as("SELECT name, domain, industry FROM companies WHERE id = $1 FOR UPDATE")
            .bind(id)
            .fetch_one(&mut *conn)
            .await?;
    let after: CompanySnapshot = sqlx::query_as(
        "UPDATE companies SET domain = coalesce($2, domain), industry = coalesce($3, industry),
                updated_at = now()
         WHERE id = $1 RETURNING name, domain, industry",
    )
    .bind(id)
    .bind(&input.domain)
    .bind(&input.industry)
    .fetch_one(&mut *conn)
    .await?;
    record_update(conn, actor, EntityType::Company, id, &before, &after).await
}

async fn record_update<T: Serialize>(
    conn: &mut PgConnection,
    actor: &AuthUser,
    entity: EntityType,
    id: Uuid,
    before: &T,
    after: &T,
) -> AppResult<AppResult<Outcome>> {
    let changes = audit::diff(before, after);
    if audit::is_empty_diff(&changes) {
        return Ok(Ok(Outcome::Unchanged));
    }
    audit::record(conn, actor, entity, id, AuditAction::Updated, changes).await?;
    Ok(Ok(Outcome::Updated))
}

pub async fn import_contacts(
    State(state): State<AppState>,
    actor: AuthUser,
    ApiJson(req): ApiJson<ImportRequest<ContactRow>>,
) -> AppResult<Json<ImportReport>> {
    check_size(req.rows.len())?;
    let mut report = ImportReport {
        dry_run: req.dry_run,
        ..ImportReport::default()
    };
    let mut companies_by_name: HashMap<String, Uuid> = HashMap::new();
    let mut tx = state.pool.begin().await?;
    for (index, row) in req.rows.into_iter().enumerate() {
        let input = contacts::ContactInput {
            first_name: row.first_name,
            last_name: row.last_name,
            email: row.email,
            phone: row.phone,
            job_title: row.job_title,
            company_id: None,
            owner_id: None,
        };
        let mut input = match contacts::validate(input) {
            Ok((input, _)) => input,
            Err(err) => {
                report.reject(index, err);
                continue;
            }
        };
        if let Some(name) = row
            .company
            .map(|c| c.trim().to_owned())
            .filter(|c| !c.is_empty())
        {
            let key = name.to_lowercase();
            let id = match companies_by_name.get(&key) {
                Some(id) => *id,
                None => {
                    let id = match find_company(&mut tx, actor.org_id, &name, None).await? {
                        Some((id, _)) => id,
                        None => {
                            let company = companies::CleanInput {
                                name: name.chars().take(200).collect(),
                                domain: None,
                                industry: None,
                            };
                            report.companies_created += 1;
                            insert_company(&mut tx, &actor, &company).await?
                        }
                    };
                    companies_by_name.insert(key, id);
                    id
                }
            };
            input.company_id = Some(id);
        }
        match upsert_contact(&mut tx, &actor, input).await? {
            Ok(outcome) => report.count(outcome),
            Err(err) => report.reject(index, err),
        }
    }
    finish(tx, report).await
}

async fn upsert_contact(
    conn: &mut PgConnection,
    actor: &AuthUser,
    input: contacts::CleanInput,
) -> AppResult<AppResult<Outcome>> {
    let existing: Option<(Uuid, Uuid)> = match &input.email {
        Some(email) => {
            sqlx::query_as(
                "SELECT id, owner_id FROM contacts
                 WHERE organization_id = $1 AND email = $2 ORDER BY id LIMIT 1 FOR UPDATE",
            )
            .bind(actor.org_id)
            .bind(email)
            .fetch_optional(&mut *conn)
            .await?
        }
        None => None,
    };
    let Some((id, owner_id)) = existing else {
        let id = Uuid::now_v7();
        let fields: ContactSnapshot = sqlx::query_as(
            "INSERT INTO contacts
                (id, organization_id, first_name, last_name, email, phone, job_title, company_id, owner_id)
             VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)
             RETURNING first_name, last_name, email, phone, job_title, company_id",
        )
        .bind(id)
        .bind(actor.org_id)
        .bind(&input.first_name)
        .bind(&input.last_name)
        .bind(&input.email)
        .bind(&input.phone)
        .bind(&input.job_title)
        .bind(input.company_id)
        .bind(actor.user_id)
        .fetch_one(&mut *conn)
        .await?;
        let snapshot = audit::snapshot(&fields);
        audit::record(
            conn,
            actor,
            EntityType::Contact,
            id,
            AuditAction::Created,
            snapshot,
        )
        .await?;
        return Ok(Ok(Outcome::Created));
    };
    if !actor.can_modify(owner_id) {
        return Ok(Err(AppError::BadRequest(
            "matches a contact owned by someone else".into(),
        )));
    }
    let before: ContactSnapshot = sqlx::query_as(
        "SELECT first_name, last_name, email, phone, job_title, company_id FROM contacts WHERE id = $1",
    )
    .bind(id)
    .fetch_one(&mut *conn)
    .await?;
    let after: ContactSnapshot = sqlx::query_as(
        "UPDATE contacts SET first_name = $2,
                last_name = CASE WHEN $3 = '' THEN last_name ELSE $3 END,
                phone = coalesce($4, phone), job_title = coalesce($5, job_title),
                company_id = coalesce($6, company_id), updated_at = now()
         WHERE id = $1
         RETURNING first_name, last_name, email, phone, job_title, company_id",
    )
    .bind(id)
    .bind(&input.first_name)
    .bind(&input.last_name)
    .bind(&input.phone)
    .bind(&input.job_title)
    .bind(input.company_id)
    .fetch_one(&mut *conn)
    .await?;
    record_update(conn, actor, EntityType::Contact, id, &before, &after).await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cells_are_quoted_only_when_needed() {
        assert_eq!(csv_cell("Acme"), "Acme");
        assert_eq!(csv_cell("Acme, Inc"), "\"Acme, Inc\"");
        assert_eq!(csv_cell("say \"hi\""), "\"say \"\"hi\"\"\"");
        assert_eq!(csv_cell("line\nbreak"), "\"line\nbreak\"");
    }

    #[test]
    fn formulas_are_neutralised() {
        assert_eq!(csv_cell("=HYPERLINK(\"x\")"), "\"'=HYPERLINK(\"\"x\"\")\"");
        assert_eq!(csv_cell("@SUM(A1)"), "'@SUM(A1)");
    }
}
