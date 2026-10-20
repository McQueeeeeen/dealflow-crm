use chrono::{Duration, Utc};
use jsonwebtoken::{decode, encode, Algorithm, DecodingKey, EncodingKey, Header, Validation};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::error::AppError;

#[derive(Debug, Serialize, Deserialize)]
pub struct Claims {
    pub sub: Uuid,
    pub org: Uuid,
    pub iat: i64,
    pub exp: i64,
}

pub struct JwtKeys {
    encoding: EncodingKey,
    decoding: DecodingKey,
    validation: Validation,
}

impl JwtKeys {
    pub fn new(secret: &[u8]) -> Self {
        let mut validation = Validation::new(Algorithm::HS256);
        validation.leeway = 5;
        Self {
            encoding: EncodingKey::from_secret(secret),
            decoding: DecodingKey::from_secret(secret),
            validation,
        }
    }

    pub fn issue(&self, user_id: Uuid, org_id: Uuid, ttl: Duration) -> anyhow::Result<String> {
        let now = Utc::now();
        let claims = Claims {
            sub: user_id,
            org: org_id,
            iat: now.timestamp(),
            exp: (now + ttl).timestamp(),
        };
        Ok(encode(&Header::new(Algorithm::HS256), &claims, &self.encoding)?)
    }

    pub fn verify(&self, token: &str) -> Result<Claims, AppError> {
        decode::<Claims>(token, &self.decoding, &self.validation)
            .map(|data| data.claims)
            .map_err(|_| AppError::Unauthorized)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SECRET: &[u8] = b"test-secret-that-is-long-enough-for-hs256";

    #[test]
    fn issued_token_verifies() {
        let keys = JwtKeys::new(SECRET);
        let (user, org) = (Uuid::now_v7(), Uuid::now_v7());
        let token = keys.issue(user, org, Duration::minutes(5)).unwrap();
        let claims = keys.verify(&token).unwrap();
        assert_eq!(claims.sub, user);
        assert_eq!(claims.org, org);
    }

    #[test]
    fn expired_token_is_rejected() {
        let keys = JwtKeys::new(SECRET);
        let token = keys
            .issue(Uuid::now_v7(), Uuid::now_v7(), Duration::minutes(-10))
            .unwrap();
        assert!(keys.verify(&token).is_err());
    }

    #[test]
    fn token_signed_with_other_key_is_rejected() {
        let token = JwtKeys::new(b"another-secret-that-is-long-enough-too")
            .issue(Uuid::now_v7(), Uuid::now_v7(), Duration::minutes(5))
            .unwrap();
        assert!(JwtKeys::new(SECRET).verify(&token).is_err());
    }
}
