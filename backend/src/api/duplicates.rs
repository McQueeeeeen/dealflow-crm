use std::collections::HashMap;

use axum::extract::State;
use axum::routing::{get, post};
use axum::{Json, Router};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sqlx::PgConnection;
use uuid::Uuid;

use crate::api::companies::SELECT_COMPANY;
use crate::api::contacts::SELECT_CONTACT;
use crate::audit;
use crate::auth::AuthUser;
use crate::error::{AppError, AppResult};
use crate::extract::{ApiJson, ApiPath};
use crate::models::EntityType;
use crate::state::AppState;

const MAX_PAIRS: i64 = 50;
const MIN_SIMILARITY: f64 = 0.7;

struct Kind {
    table: &'static str,
    entity: EntityType,
    select: &'static str,
    alias: &'static str,
    name: &'static str,
    key: &'static str,
    fields: &'static [&'static str],
    nullable: &'static [&'static str],
    references: &'static [(&'static str, &'static str)],
}

const CONTACTS: Kind = Kind {
    table: "contacts",
    entity: EntityType::Contact,
    select: SELECT_CONTACT,
    alias: "ct",
    name: "{t}.first_name || ' ' || {t}.last_name",
    key: "email",
    fields: &[
        "first_name",
        "last_name",
        "email",
        "phone",
        "job_title",
        "company_id",
        "owner_id",
    ],
    nullable: &["email", "phone", "job_title", "company_id"],
    references: &[
        ("deals", "contact_id"),
        ("tasks", "contact_id"),
        ("notes", "contact_id"),
    ],
};

const COMPANIES: Kind = Kind {
    table: "companies",
    entity: EntityType::Company,
    select: SELECT_COMPANY,
    alias: "c",
    name: "{t}.name",
    key: "domain",
    fields: &["name", "domain", "industry", "owner_id"],
    nullable: &["domain", "industry"],
    references: &[
        ("contacts", "company_id"),
        ("deals", "company_id"),
        ("notes", "company_id"),
    ],
};

fn kind(raw: &str) -> AppResult<&'static Kind> {
    match raw {
        "contacts" => Ok(&CONTACTS),
        "companies" => Ok(&COMPANIES),
        _ => Err(AppError::NotFound("duplicate kind")),
    }
}

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/duplicates/{kind}", get(find_pairs))
        .route("/duplicates/{kind}/merge", post(merge))
}

#[derive(Serialize)]
pub struct Pair {
    pub first: Value,
    pub second: Value,
    pub same_key: bool,
    pub similarity: f64,
}

#[derive(Deserialize)]
pub struct MergeRequest {
    pub keep: Uuid,
    pub remove: Uuid,
    #[serde(default)]
    pub take: Vec<String>,
}

#[derive(Serialize)]
pub struct Merged {
    pub kept: Value,
    pub moved: HashMap<&'static str, u64>,
}

async fn records(
    conn: &mut PgConnection,
    kind: &Kind,
    org_id: Uuid,
    ids: &[Uuid],
) -> AppResult<HashMap<Uuid, Value>> {
    let rows: Vec<(Uuid, Value)> = sqlx::query_as(&format!(
        "SELECT id, to_jsonb(r) FROM ({} WHERE {a}.organization_id = $1 AND {a}.id = ANY($2)) r",
        kind.select,
        a = kind.alias
    ))
    .bind(org_id)
    .bind(ids)
    .fetch_all(conn)
    .await?;
    Ok(rows.into_iter().collect())
}

pub async fn find_pairs(
    State(state): State<AppState>,
    user: AuthUser,
    ApiPath(raw): ApiPath<String>,
) -> AppResult<Json<Vec<Pair>>> {
    let kind = kind(&raw)?;
    let (name_a, name_b) = (kind.name.replace("{t}", "a"), kind.name.replace("{t}", "b"));
    let candidates: Vec<(Uuid, Uuid, bool, f64)> = sqlx::query_as(&format!(
        "SELECT a.id, b.id, coalesce(a.{key} = b.{key}, false), similarity({name_a}, {name_b})::float8
         FROM {table} a
         JOIN {table} b ON b.organization_id = a.organization_id AND a.id < b.id
         WHERE a.organization_id = $1
           AND (a.{key} = b.{key}
                OR (similarity({name_a}, {name_b}) > $2
                    AND (a.{key} IS NULL OR b.{key} IS NULL)))
         ORDER BY coalesce(a.{key} = b.{key}, false) DESC, 4 DESC, a.id, b.id
         LIMIT $3",
        key = kind.key,
        table = kind.table,
    ))
    .bind(user.org_id)
    .bind(MIN_SIMILARITY)
    .bind(MAX_PAIRS)
    .fetch_all(&state.pool)
    .await?;

    let ids: Vec<Uuid> = candidates
        .iter()
        .flat_map(|(a, b, _, _)| [*a, *b])
        .collect();
    let mut conn = state.pool.acquire().await?;
    let found = records(&mut conn, kind, user.org_id, &ids).await?;
    let pairs = candidates
        .into_iter()
        .filter_map(|(a, b, same_key, similarity)| {
            Some(Pair {
                first: found.get(&a)?.clone(),
                second: found.get(&b)?.clone(),
                same_key,
                similarity,
            })
        })
        .collect();
    Ok(Json(pairs))
}

async fn snapshot(conn: &mut PgConnection, kind: &Kind, id: Uuid) -> AppResult<Value> {
    Ok(sqlx::query_scalar(&format!(
        "SELECT to_jsonb(r) - 'organization_id' - 'created_at' - 'updated_at' FROM {} r WHERE id = $1",
        kind.table
    ))
    .bind(id)
    .fetch_one(conn)
    .await?)
}

pub async fn merge(
    State(state): State<AppState>,
    actor: AuthUser,
    ApiPath(raw): ApiPath<String>,
    ApiJson(req): ApiJson<MergeRequest>,
) -> AppResult<Json<Merged>> {
    let kind = kind(&raw)?;
    if req.keep == req.remove {
        return Err(AppError::validation("remove", "pick two different records"));
    }
    if let Some(unknown) = req.take.iter().find(|f| !kind.fields.contains(&f.as_str())) {
        return Err(AppError::validation(
            "take",
            &format!("“{unknown}” cannot be merged"),
        ));
    }

    let mut tx = state.pool.begin().await?;
    let owners: Vec<(Uuid, Uuid)> = sqlx::query_as(&format!(
        "SELECT id, owner_id FROM {} WHERE organization_id = $1 AND id = ANY($2) ORDER BY id FOR UPDATE",
        kind.table
    ))
    .bind(actor.org_id)
    .bind(&[req.keep, req.remove][..])
    .fetch_all(&mut *tx)
    .await?;
    if owners.len() != 2 {
        return Err(AppError::NotFound(if kind.table == "contacts" {
            "contact"
        } else {
            "company"
        }));
    }
    for (_, owner) in &owners {
        actor.require_can_modify(*owner)?;
    }

    let before = snapshot(&mut tx, kind, req.keep).await?;
    let removed = snapshot(&mut tx, kind, req.remove).await?;
    let assignments: Vec<String> = kind
        .fields
        .iter()
        .filter_map(|field| {
            if req.take.iter().any(|t| t == field) {
                Some(format!("{field} = r.{field}"))
            } else if kind.nullable.contains(field) {
                Some(format!("{field} = coalesce(k.{field}, r.{field})"))
            } else {
                None
            }
        })
        .collect();
    let mut moved = HashMap::new();
    for (table, column) in kind.references {
        let result = sqlx::query(&format!(
            "UPDATE {table} SET {column} = $3 WHERE organization_id = $1 AND {column} = $2"
        ))
        .bind(actor.org_id)
        .bind(req.remove)
        .bind(req.keep)
        .execute(&mut *tx)
        .await?;
        moved.insert(*table, result.rows_affected());
    }
    sqlx::query(&format!(
        "UPDATE {t} k SET {sets}, updated_at = now() FROM {t} r WHERE k.id = $1 AND r.id = $2",
        t = kind.table,
        sets = assignments.join(", ")
    ))
    .bind(req.keep)
    .bind(req.remove)
    .execute(&mut *tx)
    .await?;
    sqlx::query(&format!("DELETE FROM {} WHERE id = $1", kind.table))
        .bind(req.remove)
        .execute(&mut *tx)
        .await?;

    let after = snapshot(&mut tx, kind, req.keep).await?;
    audit::updated(&mut tx, &actor, kind.entity, req.keep, &before, &after).await?;
    audit::deleted(&mut tx, &actor, kind.entity, req.remove, &removed).await?;
    let kept = records(&mut tx, kind, actor.org_id, &[req.keep])
        .await?
        .remove(&req.keep)
        .unwrap_or(Value::Null);
    tx.commit().await?;
    Ok(Json(Merged { kept, moved }))
}
