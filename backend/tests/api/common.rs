use axum::body::Body;
use axum::http::{header, Method, Request, StatusCode};
use axum::Router;
use chrono::Duration;
use dealflow_api::config::Config;
use dealflow_api::state::AppState;
use http_body_util::BodyExt;
use serde_json::{json, Value};
use sqlx::PgPool;
use tower::ServiceExt;
use uuid::Uuid;

pub const PASSWORD: &str = "correct-horse-battery";

pub struct TestApp {
    router: Router,
    pub pool: PgPool,
}

#[derive(Debug, Clone)]
pub struct Session {
    pub token: String,
    pub refresh: String,
    pub user_id: Uuid,
}

pub struct Response {
    pub status: StatusCode,
    pub body: Value,
}

impl Response {
    pub fn id(&self) -> Uuid {
        self.uuid("id")
    }

    pub fn uuid(&self, field: &str) -> Uuid {
        self.body[field]
            .as_str()
            .and_then(|s| s.parse().ok())
            .unwrap_or_else(|| panic!("{field} missing in {}", self.body))
    }

    pub fn error_code(&self) -> &str {
        self.body["error"]["code"].as_str().unwrap_or_default()
    }

    pub fn error_fields(&self) -> Vec<String> {
        self.body["error"]["fields"]
            .as_array()
            .map(|fields| {
                fields
                    .iter()
                    .filter_map(|f| f["field"].as_str().map(str::to_owned))
                    .collect()
            })
            .unwrap_or_default()
    }

    #[track_caller]
    pub fn assert_status(&self, expected: StatusCode) -> &Self {
        assert_eq!(
            self.status, expected,
            "unexpected status, body: {}",
            self.body
        );
        self
    }
}

impl TestApp {
    pub fn new(pool: PgPool) -> Self {
        let config = Config {
            database_url: String::new(),
            bind_addr: ([127, 0, 0, 1], 0).into(),
            jwt_secret: "integration-test-secret-with-enough-bytes".into(),
            access_token_ttl: Duration::minutes(15),
            refresh_token_ttl: Duration::days(30),
            log_json: false,
        };
        Self {
            router: dealflow_api::app(AppState::new(pool.clone(), config)),
            pool,
        }
    }

    pub async fn request(
        &self,
        method: Method,
        path: &str,
        token: Option<&str>,
        body: Option<Value>,
    ) -> Response {
        let mut builder = Request::builder().method(method).uri(path);
        if let Some(token) = token {
            builder = builder.header(header::AUTHORIZATION, format!("Bearer {token}"));
        }
        let request = match body {
            Some(body) => builder
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(body.to_string())),
            None => builder.body(Body::empty()),
        }
        .expect("valid request");

        let response = self
            .router
            .clone()
            .oneshot(request)
            .await
            .expect("infallible");
        let status = response.status();
        let bytes = response
            .into_body()
            .collect()
            .await
            .expect("readable body")
            .to_bytes();
        let body = if bytes.is_empty() {
            Value::Null
        } else {
            serde_json::from_slice(&bytes)
                .unwrap_or_else(|_| Value::String(String::from_utf8_lossy(&bytes).into_owned()))
        };
        Response { status, body }
    }

    pub async fn get(&self, path: &str, s: &Session) -> Response {
        self.request(Method::GET, path, Some(&s.token), None).await
    }

    pub async fn post(&self, path: &str, s: &Session, body: Value) -> Response {
        self.request(Method::POST, path, Some(&s.token), Some(body))
            .await
    }

    pub async fn put(&self, path: &str, s: &Session, body: Value) -> Response {
        self.request(Method::PUT, path, Some(&s.token), Some(body))
            .await
    }

    pub async fn delete(&self, path: &str, s: &Session) -> Response {
        self.request(Method::DELETE, path, Some(&s.token), None)
            .await
    }

    /// Creates a new organization and returns its admin's session.
    pub async fn register(&self, org: &str, email: &str) -> Session {
        let res = self
            .request(
                Method::POST,
                "/api/v1/auth/register",
                None,
                Some(json!({
                    "organization_name": org,
                    "full_name": format!("{org} Admin"),
                    "email": email,
                    "password": PASSWORD,
                })),
            )
            .await;
        res.assert_status(StatusCode::CREATED);
        session_from(&res.body)
    }

    pub async fn login(&self, email: &str) -> Session {
        let res = self
            .request(
                Method::POST,
                "/api/v1/auth/login",
                None,
                Some(json!({ "email": email, "password": PASSWORD })),
            )
            .await;
        res.assert_status(StatusCode::OK);
        session_from(&res.body)
    }

    /// Adds a teammate to the admin's organization and signs them in.
    pub async fn add_user(&self, admin: &Session, email: &str, role: &str) -> Session {
        self.post(
            "/api/v1/users",
            admin,
            json!({ "email": email, "full_name": email, "role": role, "password": PASSWORD }),
        )
        .await
        .assert_status(StatusCode::CREATED);
        self.login(email).await
    }

    pub async fn stages(&self, s: &Session) -> Vec<Value> {
        let res = self.get("/api/v1/stages", s).await;
        res.body.as_array().cloned().expect("stage list")
    }

    pub async fn stage_id(&self, s: &Session, name: &str) -> Uuid {
        self.stages(s)
            .await
            .iter()
            .find(|stage| stage["name"] == name)
            .and_then(|stage| stage["id"].as_str())
            .and_then(|id| id.parse().ok())
            .unwrap_or_else(|| panic!("stage {name} not found"))
    }

    pub async fn create_deal(&self, s: &Session, body: Value) -> Response {
        let res = self.post("/api/v1/deals", s, body).await;
        res.assert_status(StatusCode::CREATED);
        res
    }

    pub async fn move_deal(&self, s: &Session, deal: &Value, stage: Uuid) -> Response {
        self.post(
            &format!("/api/v1/deals/{}/move", deal["id"].as_str().unwrap()),
            s,
            json!({ "stage_id": stage, "version": deal["version"] }),
        )
        .await
    }
}

fn session_from(body: &Value) -> Session {
    Session {
        token: body["access_token"]
            .as_str()
            .expect("access token")
            .to_owned(),
        refresh: body["refresh_token"]
            .as_str()
            .expect("refresh token")
            .to_owned(),
        user_id: body["user"]["id"].as_str().unwrap().parse().unwrap(),
    }
}
