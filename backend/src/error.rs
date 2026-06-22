use axum::{http::StatusCode, response::IntoResponse};
use serde_json::Value;

use crate::response;

pub type AppResult<T> = Result<T, AppError>;

#[derive(Debug, thiserror::Error)]
pub enum AppError {
    #[error("Unauthorized")]
    Unauthorized,
    #[error("Forbidden")]
    Forbidden,
    #[error("{0}")]
    BadRequest(String),
    #[error("{0}")]
    NotFound(String),
    #[error("{0}")]
    Conflict(String),
    #[error("Database operation failed")]
    Database(#[from] sqlx::Error),
    #[error("{0}")]
    External(String),
    #[error("{0}")]
    Internal(String),
    #[error("{0}")]
    Configuration(String),
    #[error("{message}")]
    Validation {
        message: String,
        details: Option<Value>,
    },
}

impl AppError {
    pub fn status(&self) -> StatusCode {
        match self {
            Self::Unauthorized => StatusCode::UNAUTHORIZED,
            Self::Forbidden => StatusCode::FORBIDDEN,
            Self::BadRequest(_) | Self::Validation { .. } => StatusCode::BAD_REQUEST,
            Self::NotFound(_) => StatusCode::NOT_FOUND,
            Self::Conflict(_) => StatusCode::CONFLICT,
            Self::Database(_) | Self::External(_) | Self::Internal(_) | Self::Configuration(_) => {
                StatusCode::INTERNAL_SERVER_ERROR
            }
        }
    }

    pub fn public_message(&self) -> String {
        match self {
            Self::Database(_) => "Database operation failed".to_owned(),
            Self::Configuration(message) => message.clone(),
            Self::Unauthorized => "Unauthorized".to_owned(),
            Self::Forbidden => "Forbidden".to_owned(),
            Self::BadRequest(message)
            | Self::NotFound(message)
            | Self::Conflict(message)
            | Self::External(message)
            | Self::Internal(message) => message.clone(),
            Self::Validation { message, .. } => message.clone(),
        }
    }

    pub fn details(&self) -> Option<Value> {
        match self {
            Self::Validation { details, .. } => details.clone(),
            Self::Database(error) => Some(Value::String(error.to_string())),
            _ => None,
        }
    }
}

impl IntoResponse for AppError {
    fn into_response(self) -> axum::response::Response {
        let status = self.status();
        tracing::warn!(error = %self, status = status.as_u16(), "request failed");
        response::error_response(status, self.public_message(), self.details()).into_response()
    }
}

pub fn validation(message: impl Into<String>, details: impl Into<Value>) -> AppError {
    AppError::Validation {
        message: message.into(),
        details: Some(details.into()),
    }
}
