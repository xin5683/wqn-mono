use axum::{Json, http::StatusCode, response::IntoResponse};
use chrono::{SecondsFormat, Utc};
use serde::Serialize;
use serde_json::{Value, json};
use utoipa::ToSchema;

#[derive(Debug, Serialize, ToSchema)]
pub struct ApiSuccess<T>
where
    T: Serialize,
{
    pub data: T,
    pub success: bool,
    pub timestamp: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct ApiErrorBody {
    pub error: String,
    pub status: u16,
    pub timestamp: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub details: Option<Value>,
}

pub fn now_iso() -> String {
    Utc::now().to_rfc3339_opts(SecondsFormat::Millis, true)
}

pub fn success<T: Serialize>(data: T) -> Json<ApiSuccess<T>> {
    Json(ApiSuccess {
        data,
        success: true,
        timestamp: now_iso(),
        message: None,
    })
}

pub fn created<T: Serialize>(data: T) -> (StatusCode, Json<ApiSuccess<T>>) {
    (StatusCode::CREATED, success(data))
}

pub fn empty_ok() -> Json<ApiSuccess<Value>> {
    success(json!({ "ok": true }))
}

pub fn error_response(
    status: StatusCode,
    message: impl Into<String>,
    details: Option<Value>,
) -> impl IntoResponse {
    (
        status,
        Json(ApiErrorBody {
            error: message.into(),
            status: status.as_u16(),
            timestamp: now_iso(),
            details,
        }),
    )
}
