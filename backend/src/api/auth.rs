use axum::extract::State;
use axum::http::StatusCode;
use axum::routing::{get, post};
use axum::{Json, Router};
use chrono::{DateTime, Duration, Utc};
use serde::{Deserialize, Serialize};
use sqlx::{FromRow, PgConnection};
use uuid::Uuid;

use crate::auth::password::{self, MAX_PASSWORD_LEN, MIN_PASSWORD_LEN};
use crate::auth::{refresh, AuthUser};
use crate::error::{AppError, AppResult};
use crate::extract::ApiJson;
use crate::models::{Role, StageKind};
use crate::state::AppState;
use crate::validation::{normalize_email, Validator};

const ROTATION_GRACE_SECONDS: i64 = 10;

const DEFAULT_STAGES: [(&str, i16, StageKind); 6] = [
    ("Lead", 10, StageKind::Open),
    ("Qualified", 25, StageKind::Open),
    ("Proposal", 50, StageKind::Open),
    ("Negotiation", 75, StageKind::Open),
    ("Won", 100, StageKind::Won),
    ("Lost", 0, StageKind::Lost),
];

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/auth/register", post(register))
        .route("/auth/login", post(login))
        .route("/auth/refresh", post(refresh_session))
        .route("/auth/logout", post(logout))
        .route("/auth/me", get(me))
}

#[derive(Deserialize)]
pub struct RegisterRequest {
    pub organization_name: String,
    pub full_name: String,
    pub email: String,
    pub password: String,
    pub currency: Option<String>,
}

#[derive(Deserialize)]
pub struct LoginRequest {
    pub email: String,
    pub password: String,
}

#[derive(Deserialize)]
pub struct RefreshRequest {
    pub refresh_token: String,
}

#[derive(Serialize, FromRow)]
pub struct Me {
    pub id: Uuid,
    pub email: String,
    pub full_name: String,
    pub role: Role,
    pub organization_id: Uuid,
    pub organization_name: String,
    pub currency: String,
}

#[derive(Serialize)]
pub struct Session {
    pub access_token: String,
    pub refresh_token: String,
    pub token_type: &'static str,
    pub expires_in: i64,
    pub user: Me,
}

pub async fn register(
    State(state): State<AppState>,
    ApiJson(req): ApiJson<RegisterRequest>,
) -> AppResult<(StatusCode, Json<Session>)> {
    let organization_name = req.organization_name.trim();
    let full_name = req.full_name.trim();
    let email = normalize_email(&req.email);
    let currency = req
        .currency
        .as_deref()
        .map(|c| c.trim().to_uppercase())
        .unwrap_or_else(|| "USD".to_owned());

    Validator::new()
        .required("organization_name", organization_name, 120)
        .required("full_name", full_name, 120)
        .email("email", &email)
        .check(
            (MIN_PASSWORD_LEN..=MAX_PASSWORD_LEN).contains(&req.password.chars().count()),
            "password",
            &format!("must be {MIN_PASSWORD_LEN} to {MAX_PASSWORD_LEN} characters"),
        )
        .check(
            currency.len() == 3 && currency.chars().all(|c| c.is_ascii_uppercase()),
            "currency",
            "must be a three-letter ISO 4217 code",
        )
        .finish()?;

    let password_hash = password::hash_password(&req.password)?;
    let org_id = Uuid::now_v7();
    let user_id = Uuid::now_v7();

    let mut tx = state.pool.begin().await?;
    sqlx::query("INSERT INTO organizations (id, name, currency) VALUES ($1, $2, $3)")
        .bind(org_id)
        .bind(organization_name)
        .bind(&currency)
        .execute(&mut *tx)
        .await?;
    sqlx::query(
        "INSERT INTO users (id, organization_id, email, full_name, password_hash, role)
         VALUES ($1, $2, $3, $4, $5, 'admin')",
    )
    .bind(user_id)
    .bind(org_id)
    .bind(&email)
    .bind(full_name)
    .bind(&password_hash)
    .execute(&mut *tx)
    .await?;
    for (position, (name, probability, kind)) in DEFAULT_STAGES.iter().enumerate() {
        sqlx::query(
            "INSERT INTO pipeline_stages (id, organization_id, name, position, probability, kind)
             VALUES ($1, $2, $3, $4, $5, $6)",
        )
        .bind(Uuid::now_v7())
        .bind(org_id)
        .bind(name)
        .bind(position as i32)
        .bind(probability)
        .bind(kind)
        .execute(&mut *tx)
        .await?;
    }
    let session = start_session(&mut tx, &state, user_id, org_id, None).await?;
    tx.commit().await?;

    Ok((StatusCode::CREATED, Json(session)))
}

pub async fn login(
    State(state): State<AppState>,
    ApiJson(req): ApiJson<LoginRequest>,
) -> AppResult<Json<Session>> {
    let email = normalize_email(&req.email);
    if state.login_throttle.is_locked(&email) {
        return Err(AppError::TooManyAttempts);
    }
    let user: Option<(Uuid, Uuid, String, bool)> = sqlx::query_as(
        "SELECT id, organization_id, password_hash, is_active FROM users WHERE email = $1",
    )
    .bind(&email)
    .fetch_optional(&state.pool)
    .await?;

    let Some((user_id, org_id, hash, is_active)) = user else {
        password::burn_verification_time(&req.password);
        state.login_throttle.record_failure(&email);
        return Err(AppError::InvalidCredentials);
    };
    if !password::verify_password(&req.password, &hash) || !is_active {
        state.login_throttle.record_failure(&email);
        return Err(AppError::InvalidCredentials);
    }
    state.login_throttle.clear(&email);

    let mut conn = state.pool.acquire().await?;
    let session = start_session(&mut conn, &state, user_id, org_id, None).await?;
    Ok(Json(session))
}

#[derive(FromRow)]
struct StoredRefreshToken {
    id: Uuid,
    user_id: Uuid,
    organization_id: Uuid,
    family_id: Uuid,
    expires_at: DateTime<Utc>,
    used_at: Option<DateTime<Utc>>,
    revoked_at: Option<DateTime<Utc>>,
    is_active: bool,
}

pub async fn refresh_session(
    State(state): State<AppState>,
    ApiJson(req): ApiJson<RefreshRequest>,
) -> AppResult<Json<Session>> {
    let mut tx = state.pool.begin().await?;
    let stored: Option<StoredRefreshToken> = sqlx::query_as(
        "SELECT t.id, t.user_id, u.organization_id, t.family_id, t.expires_at,
                t.used_at, t.revoked_at, u.is_active
         FROM refresh_tokens t JOIN users u ON u.id = t.user_id
         WHERE t.token_hash = $1
         FOR UPDATE OF t",
    )
    .bind(refresh::digest(&req.refresh_token))
    .fetch_optional(&mut *tx)
    .await?;

    let Some(token) = stored else {
        return Err(AppError::Unauthorized);
    };
    let now = Utc::now();

    if let Some(used_at) = token.used_at {
        if now - used_at > Duration::seconds(ROTATION_GRACE_SECONDS) {
            revoke_family(&mut tx, token.family_id).await?;
            tx.commit().await?;
            tracing::warn!(user_id = %token.user_id, "refresh token reuse detected, session family revoked");
        }
        return Err(AppError::Unauthorized);
    }
    if token.revoked_at.is_some() || token.expires_at <= now || !token.is_active {
        return Err(AppError::Unauthorized);
    }

    sqlx::query("UPDATE refresh_tokens SET used_at = now() WHERE id = $1")
        .bind(token.id)
        .execute(&mut *tx)
        .await?;
    let session = start_session(
        &mut tx,
        &state,
        token.user_id,
        token.organization_id,
        Some(token.family_id),
    )
    .await?;
    tx.commit().await?;
    Ok(Json(session))
}

pub async fn logout(
    State(state): State<AppState>,
    ApiJson(req): ApiJson<RefreshRequest>,
) -> AppResult<StatusCode> {
    let family: Option<Uuid> =
        sqlx::query_scalar("SELECT family_id FROM refresh_tokens WHERE token_hash = $1")
            .bind(refresh::digest(&req.refresh_token))
            .fetch_optional(&state.pool)
            .await?;
    if let Some(family) = family {
        let mut conn = state.pool.acquire().await?;
        revoke_family(&mut conn, family).await?;
    }
    Ok(StatusCode::NO_CONTENT)
}

pub async fn me(State(state): State<AppState>, user: AuthUser) -> AppResult<Json<Me>> {
    let mut conn = state.pool.acquire().await?;
    Ok(Json(load_me(&mut conn, user.user_id).await?))
}

async fn load_me(conn: &mut PgConnection, user_id: Uuid) -> AppResult<Me> {
    sqlx::query_as(
        "SELECT u.id, u.email, u.full_name, u.role, u.organization_id,
                o.name AS organization_name, o.currency
         FROM users u JOIN organizations o ON o.id = u.organization_id
         WHERE u.id = $1",
    )
    .bind(user_id)
    .fetch_optional(conn)
    .await?
    .ok_or(AppError::NotFound("user"))
}

async fn start_session(
    conn: &mut PgConnection,
    state: &AppState,
    user_id: Uuid,
    org_id: Uuid,
    family_id: Option<Uuid>,
) -> AppResult<Session> {
    let config = &state.config;
    let access_token = state.jwt.issue(user_id, org_id, config.access_token_ttl)?;
    let refresh_token = refresh::generate();

    sqlx::query(
        "INSERT INTO refresh_tokens (id, user_id, family_id, token_hash, expires_at)
         VALUES ($1, $2, $3, $4, $5)",
    )
    .bind(Uuid::now_v7())
    .bind(user_id)
    .bind(family_id.unwrap_or_else(Uuid::now_v7))
    .bind(refresh::digest(&refresh_token))
    .bind(Utc::now() + config.refresh_token_ttl)
    .execute(&mut *conn)
    .await?;

    Ok(Session {
        access_token,
        refresh_token,
        token_type: "Bearer",
        expires_in: config.access_token_ttl.num_seconds(),
        user: load_me(conn, user_id).await?,
    })
}

async fn revoke_family(conn: &mut PgConnection, family_id: Uuid) -> AppResult<()> {
    sqlx::query(
        "UPDATE refresh_tokens SET revoked_at = now() WHERE family_id = $1 AND revoked_at IS NULL",
    )
    .bind(family_id)
    .execute(conn)
    .await?;
    Ok(())
}
