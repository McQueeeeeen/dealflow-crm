use axum::extract::rejection::{JsonRejection, PathRejection, QueryRejection};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::Json;
use serde::Serialize;
use serde_json::json;

pub type AppResult<T> = Result<T, AppError>;

#[derive(Debug, Clone, Serialize)]
pub struct FieldError {
    pub field: String,
    pub message: String,
}

#[derive(Debug, thiserror::Error)]
pub enum AppError {
    #[error("authentication required")]
    Unauthorized,
    #[error("invalid email or password")]
    InvalidCredentials,
    #[error("you do not have permission to perform this action")]
    Forbidden,
    #[error("{0} not found")]
    NotFound(&'static str),
    #[error("{0}")]
    Conflict(String),
    #[error("validation failed")]
    Validation(Vec<FieldError>),
    #[error("{0}")]
    BadRequest(String),
    #[error(transparent)]
    Database(sqlx::Error),
    #[error(transparent)]
    Internal(#[from] anyhow::Error),
}

impl AppError {
    pub fn validation(field: &str, message: &str) -> Self {
        Self::Validation(vec![FieldError {
            field: field.to_owned(),
            message: message.to_owned(),
        }])
    }

    fn status_and_code(&self) -> (StatusCode, &'static str) {
        match self {
            Self::Unauthorized => (StatusCode::UNAUTHORIZED, "unauthorized"),
            Self::InvalidCredentials => (StatusCode::UNAUTHORIZED, "invalid_credentials"),
            Self::Forbidden => (StatusCode::FORBIDDEN, "forbidden"),
            Self::NotFound(_) => (StatusCode::NOT_FOUND, "not_found"),
            Self::Conflict(_) => (StatusCode::CONFLICT, "conflict"),
            Self::Validation(_) => (StatusCode::UNPROCESSABLE_ENTITY, "validation_failed"),
            Self::BadRequest(_) => (StatusCode::BAD_REQUEST, "bad_request"),
            Self::Database(_) | Self::Internal(_) => {
                (StatusCode::INTERNAL_SERVER_ERROR, "internal_error")
            }
        }
    }
}

impl From<sqlx::Error> for AppError {
    fn from(err: sqlx::Error) -> Self {
        if let sqlx::Error::Database(db) = &err {
            let constraint = db.constraint().unwrap_or_default();
            if db.is_foreign_key_violation() {
                return match constraint_field(constraint) {
                    Some(field) => Self::validation(field, "does not exist"),
                    None => Self::Conflict("record is still referenced by other data".into()),
                };
            }
            if db.is_unique_violation() {
                return match constraint {
                    "users_email_key" => Self::validation("email", "is already registered"),
                    "pipeline_stages_name_key" => {
                        Self::validation("name", "a stage with this name already exists")
                    }
                    _ => Self::Conflict("record already exists".into()),
                };
            }
            if db.is_check_violation() {
                return Self::BadRequest(format!("value violates constraint {constraint}"));
            }
        }
        Self::Database(err)
    }
}

fn constraint_field(constraint: &str) -> Option<&'static str> {
    Some(match constraint {
        "companies_owner_fkey" | "contacts_owner_fkey" | "deals_owner_fkey" => "owner_id",
        "contacts_company_fkey" | "deals_company_fkey" | "notes_company_fkey" => "company_id",
        "deals_contact_fkey" | "tasks_contact_fkey" | "notes_contact_fkey" => "contact_id",
        "deals_stage_fkey" | "deal_stage_changes_to_stage_fkey" => "stage_id",
        "tasks_deal_fkey" | "notes_deal_fkey" => "deal_id",
        "tasks_assignee_fkey" => "assignee_id",
        _ => return None,
    })
}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        let (status, code) = self.status_and_code();
        if status.is_server_error() {
            tracing::error!(error = ?self, "request failed");
        }
        let message = if status.is_server_error() {
            "internal server error".to_owned()
        } else {
            self.to_string()
        };
        let fields = match self {
            Self::Validation(fields) => fields,
            _ => Vec::new(),
        };
        let body = json!({ "error": { "code": code, "message": message, "fields": fields } });
        (status, Json(body)).into_response()
    }
}

impl From<JsonRejection> for AppError {
    fn from(rejection: JsonRejection) -> Self {
        if rejection.status() == StatusCode::UNPROCESSABLE_ENTITY {
            Self::validation("body", &rejection.body_text())
        } else {
            Self::BadRequest(rejection.body_text())
        }
    }
}

impl From<QueryRejection> for AppError {
    fn from(rejection: QueryRejection) -> Self {
        Self::BadRequest(rejection.body_text())
    }
}

impl From<PathRejection> for AppError {
    fn from(rejection: PathRejection) -> Self {
        Self::BadRequest(rejection.body_text())
    }
}
