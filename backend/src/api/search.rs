use axum::extract::State;
use axum::routing::get;
use axum::{Json, Router};
use serde::{Deserialize, Serialize};
use sqlx::{FromRow, PgConnection};
use uuid::Uuid;

use crate::auth::AuthUser;
use crate::error::AppResult;
use crate::extract::ApiQuery;
use crate::state::AppState;
use crate::validation::{clean, like_pattern};

const HITS: i64 = 6;

pub fn router() -> Router<AppState> {
    Router::new().route("/search", get(search))
}

#[derive(Deserialize)]
pub struct SearchQuery {
    pub q: Option<String>,
}

#[derive(Debug, Serialize, FromRow)]
pub struct Hit {
    pub id: Uuid,
    pub title: String,
    pub detail: Option<String>,
}

#[derive(Debug, Default, Serialize)]
pub struct SearchResults {
    pub deals: Vec<Hit>,
    pub companies: Vec<Hit>,
    pub contacts: Vec<Hit>,
}

const DEALS: &str = "
    SELECT d.id, d.title, co.name AS detail
    FROM deals d LEFT JOIN companies co ON co.id = d.company_id
    WHERE d.organization_id = $1 AND (d.title ILIKE $2 OR $3 <% d.title)
    ORDER BY d.title ILIKE $2 DESC, word_similarity($3, d.title) DESC, d.updated_at DESC
    LIMIT $4";

const COMPANIES: &str = "
    SELECT c.id, c.name AS title, c.domain AS detail
    FROM companies c
    WHERE c.organization_id = $1 AND (c.name ILIKE $2 OR c.domain ILIKE $2 OR $3 <% c.name)
    ORDER BY c.name ILIKE $2 DESC, word_similarity($3, c.name) DESC, c.name
    LIMIT $4";

const CONTACTS: &str = "
    SELECT ct.id, trim(ct.first_name || ' ' || ct.last_name) AS title,
           coalesce(ct.email, ct.job_title) AS detail
    FROM contacts ct
    WHERE ct.organization_id = $1
      AND ((ct.first_name || ' ' || ct.last_name || ' ' || coalesce(ct.email, '')) ILIKE $2
           OR $3 <% (ct.first_name || ' ' || ct.last_name || ' ' || coalesce(ct.email, '')))
    ORDER BY (ct.first_name || ' ' || ct.last_name) ILIKE $2 DESC,
             word_similarity($3, ct.first_name || ' ' || ct.last_name) DESC, ct.first_name
    LIMIT $4";

pub async fn search(
    State(state): State<AppState>,
    user: AuthUser,
    ApiQuery(query): ApiQuery<SearchQuery>,
) -> AppResult<Json<SearchResults>> {
    let Some(q) = clean(query.q).filter(|q| q.chars().count() >= 2) else {
        return Ok(Json(SearchResults::default()));
    };
    let q: String = q.chars().take(100).collect();
    let pattern = like_pattern(&q);
    let mut tx = state.pool.begin().await?;
    sqlx::query("SET LOCAL pg_trgm.word_similarity_threshold = 0.5")
        .execute(&mut *tx)
        .await?;
    let results = SearchResults {
        deals: hits(&mut tx, DEALS, user.org_id, &pattern, &q).await?,
        companies: hits(&mut tx, COMPANIES, user.org_id, &pattern, &q).await?,
        contacts: hits(&mut tx, CONTACTS, user.org_id, &pattern, &q).await?,
    };
    Ok(Json(results))
}

async fn hits(
    conn: &mut PgConnection,
    sql: &str,
    org_id: Uuid,
    pattern: &str,
    q: &str,
) -> AppResult<Vec<Hit>> {
    Ok(sqlx::query_as(sql)
        .bind(org_id)
        .bind(pattern)
        .bind(q)
        .bind(HITS)
        .fetch_all(conn)
        .await?)
}
