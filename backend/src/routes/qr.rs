use axum::{
    Json, Router,
    extract::{Multipart, Path, Query, State},
    routing::{get, post},
};
use chrono::{Duration, Utc};
use serde::Deserialize;
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use sqlx::types::Json as SqlJson;
use uuid::Uuid;

use crate::{
    auth::AuthUser,
    error::{AppError, AppResult},
    response,
    services::storage,
    state::AppState,
};

#[derive(Debug, Deserialize)]
struct UploadQuery {
    token: String,
}

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/qr-sessions", post(create_session))
        .route("/qr-sessions/{session_id}/status", get(status))
        .route("/qr-sessions/{session_id}/consume", post(consume))
        .route("/qr-upload/{session_id}", post(upload))
}

async fn create_session(
    State(state): State<AppState>,
    auth: AuthUser,
) -> AppResult<Json<response::ApiSuccess<Value>>> {
    let token = Uuid::new_v4().to_string().replace('-', "");
    let token_hash = hash_token(&token);
    let expires_at = Utc::now() + Duration::minutes(10);
    let row = sqlx::query_scalar::<_, SqlJson<Value>>(
        r#"
        insert into qr_upload_sessions (user_id, token_hash, status, expires_at)
        values ($1, $2, 'pending', $3)
        returning to_jsonb(qr_upload_sessions) as data
        "#,
    )
    .bind(auth.id)
    .bind(token_hash)
    .bind(expires_at)
    .fetch_one(&state.pool)
    .await?;
    let session_id = row.0.get("id").cloned().unwrap_or(Value::Null);
    Ok(response::success(json!({
        "sessionId": session_id,
        "token": token,
        "expiresAt": row.0.get("expires_at"),
        "uploadUrl": format!("/upload/{}?token={}", session_id.as_str().unwrap_or_default(), token),
    })))
}

async fn status(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(session_id): Path<Uuid>,
) -> AppResult<Json<response::ApiSuccess<Value>>> {
    let row = qr_session(&state.pool, auth.id, session_id).await?;
    let mut status = row
        .get("status")
        .and_then(Value::as_str)
        .unwrap_or("pending")
        .to_owned();
    if status == "pending"
        && let Some(expires_at) = row.get("expires_at").and_then(Value::as_str)
        && chrono::DateTime::parse_from_rfc3339(expires_at)
            .map(|dt| dt.with_timezone(&Utc) < Utc::now())
            .unwrap_or(false)
    {
        status = "expired".to_owned();
    }
    Ok(response::success(json!({
        "status": status,
        "filePath": row.get("file_path"),
        "mimeType": row.get("mime_type"),
    })))
}

async fn consume(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(session_id): Path<Uuid>,
) -> AppResult<Json<response::ApiSuccess<Value>>> {
    let row = qr_session(&state.pool, auth.id, session_id).await?;
    if row.get("status").and_then(Value::as_str) != Some("uploaded") {
        return Err(AppError::BadRequest(
            "Session has not uploaded a file".to_owned(),
        ));
    }
    sqlx::query(
        r#"
        update qr_upload_sessions
        set status = 'consumed', consumed_at = now()
        where id = $1 and user_id = $2 and status = 'uploaded'
        "#,
    )
    .bind(session_id)
    .bind(auth.id)
    .execute(&state.pool)
    .await?;
    Ok(response::success(json!({
        "filePath": row.get("file_path"),
        "mimeType": row.get("mime_type"),
    })))
}

async fn upload(
    State(state): State<AppState>,
    Path(session_id): Path<Uuid>,
    Query(query): Query<UploadQuery>,
    mut multipart: Multipart,
) -> AppResult<Json<response::ApiSuccess<Value>>> {
    let row = sqlx::query_scalar::<_, SqlJson<Value>>(
        "select to_jsonb(qr_upload_sessions) as data from qr_upload_sessions where id = $1",
    )
    .bind(session_id)
    .fetch_optional(&state.pool)
    .await?
    .ok_or_else(|| AppError::NotFound("Session not found".to_owned()))?
    .0;
    if row.get("token_hash").and_then(Value::as_str) != Some(hash_token(&query.token).as_str()) {
        return Err(AppError::Forbidden);
    }
    if row.get("status").and_then(Value::as_str) != Some("pending") {
        return Err(AppError::Conflict("Session already used".to_owned()));
    }
    let expires_at = row
        .get("expires_at")
        .and_then(Value::as_str)
        .and_then(|value| chrono::DateTime::parse_from_rfc3339(value).ok())
        .map(|dt| dt.with_timezone(&Utc));
    if expires_at.is_some_and(|dt| dt < Utc::now()) {
        return Err(AppError::BadRequest("Session expired".to_owned()));
    }

    let mut uploaded = None;
    while let Some(field) = multipart
        .next_field()
        .await
        .map_err(|err| AppError::BadRequest(err.to_string()))?
    {
        if field.name() != Some("file") {
            continue;
        }
        let file_name = field.file_name().map(ToOwned::to_owned);
        let content_type = field.content_type().map(ToOwned::to_owned);
        let user_id = row
            .get("user_id")
            .and_then(Value::as_str)
            .ok_or_else(|| AppError::Internal("QR session missing user_id".to_owned()))?;
        let bytes = field
            .bytes()
            .await
            .map_err(|err| AppError::BadRequest(err.to_string()))?;
        let file =
            storage::validate_problem_upload(file_name.as_deref(), content_type.as_deref(), bytes)?;
        let path = format!("user/{user_id}/qr/{session_id}/{}", file.file_name);
        storage::upload_object(
            &state,
            storage::PROBLEM_UPLOADS_BUCKET,
            &path,
            &file.content_type,
            file.bytes,
            false,
        )
        .await?;
        uploaded = Some((path, file.content_type));
        break;
    }
    let (path, content_type) =
        uploaded.ok_or_else(|| AppError::BadRequest("No file provided".to_owned()))?;
    sqlx::query(
        r#"
        update qr_upload_sessions
        set status = 'uploaded', file_path = $2, mime_type = $3, uploaded_at = now()
        where id = $1 and status = 'pending'
        "#,
    )
    .bind(session_id)
    .bind(path)
    .bind(content_type)
    .execute(&state.pool)
    .await?;
    Ok(response::success(
        json!({ "message": "File uploaded successfully" }),
    ))
}

async fn qr_session(pool: &sqlx::PgPool, user_id: Uuid, session_id: Uuid) -> AppResult<Value> {
    let row = sqlx::query_scalar::<_, SqlJson<Value>>(
        "select to_jsonb(qr_upload_sessions) as data from qr_upload_sessions where id = $1 and user_id = $2",
    )
    .bind(session_id)
    .bind(user_id)
    .fetch_optional(pool)
    .await?;
    row.map(|SqlJson(v)| v)
        .ok_or_else(|| AppError::NotFound("Session not found".to_owned()))
}

fn hash_token(token: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(token.as_bytes());
    format!("{:x}", hasher.finalize())
}
