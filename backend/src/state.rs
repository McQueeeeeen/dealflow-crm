use std::sync::Arc;

use sqlx::PgPool;

use crate::auth::jwt::JwtKeys;
use crate::auth::throttle::LoginThrottle;
use crate::config::Config;

#[derive(Clone)]
pub struct AppState {
    pub pool: PgPool,
    pub jwt: Arc<JwtKeys>,
    pub config: Arc<Config>,
    pub login_throttle: Arc<LoginThrottle>,
}

impl AppState {
    pub fn new(pool: PgPool, config: Config) -> Self {
        Self {
            pool,
            jwt: Arc::new(JwtKeys::new(config.jwt_secret.as_bytes())),
            config: Arc::new(config),
            login_throttle: Arc::default(),
        }
    }
}
