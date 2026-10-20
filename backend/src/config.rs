use std::net::SocketAddr;

use anyhow::{bail, Context};
use chrono::Duration;

#[derive(Debug, Clone)]
pub struct Config {
    pub database_url: String,
    pub bind_addr: SocketAddr,
    pub jwt_secret: String,
    pub access_token_ttl: Duration,
    pub refresh_token_ttl: Duration,
    pub log_json: bool,
}

impl Config {
    pub fn from_env() -> anyhow::Result<Self> {
        let database_url =
            required("DATABASE_URL_UNPOOLED").or_else(|_| required("DATABASE_URL"))?;
        let jwt_secret = required("JWT_SECRET")?;
        if jwt_secret.len() < 32 {
            bail!("JWT_SECRET must be at least 32 bytes long");
        }

        let port: u16 = optional("PORT", "8080")
            .parse()
            .context("PORT must be a valid port number")?;
        let access_minutes: i64 = optional("ACCESS_TOKEN_TTL_MINUTES", "15")
            .parse()
            .context("ACCESS_TOKEN_TTL_MINUTES must be an integer")?;
        let refresh_days: i64 = optional("REFRESH_TOKEN_TTL_DAYS", "30")
            .parse()
            .context("REFRESH_TOKEN_TTL_DAYS must be an integer")?;

        Ok(Self {
            database_url,
            bind_addr: SocketAddr::from(([0, 0, 0, 0], port)),
            jwt_secret,
            access_token_ttl: Duration::minutes(access_minutes),
            refresh_token_ttl: Duration::days(refresh_days),
            log_json: optional("LOG_FORMAT", "pretty") == "json",
        })
    }
}

fn required(key: &str) -> anyhow::Result<String> {
    std::env::var(key).with_context(|| format!("{key} must be set"))
}

fn optional(key: &str, default: &str) -> String {
    std::env::var(key).unwrap_or_else(|_| default.to_owned())
}
