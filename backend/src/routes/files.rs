use axum::{
    Json, Router,
    body::Bytes,
    extract::{Multipart, Path, State},
    http::{HeaderValue, StatusCode, header},
    response::{IntoResponse, Response},
    routing::{delete, get, post},
};
use serde_json::{Value, json};

use crate::{
    auth::{AuthUser, OptionalAuthUser},
    error::{AppError, AppResult},
    models::FileDeleteBody,
    response,
    services::storage,
    state::AppState,
};

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/files/upload", post(upload_files))
        .route("/files/delete", delete(delete_file))
        .route("/files/{*path}", get(get_file))
}

async fn get_file(
    State(state): State<AppState>,
    OptionalAuthUser(auth): OptionalAuthUser,
    Path(path): Path<String>,
) -> AppResult<impl IntoResponse> {
    let decoded = urlencoding::decode(&path)
        .map_err(|_| AppError::BadRequest("Invalid file path".to_owned()))?
        .into_owned();

    if let Some(avatar_path) = decoded.strip_prefix("avatars/") {
        if !storage::validate_avatar_storage_path(avatar_path) {
            return Err(AppError::Forbidden);
        }
        let (bytes, content_type) =
            storage::read_object(&state, storage::AVATARS_BUCKET, avatar_path).await?;
        return bytes_response(bytes, &content_type);
    }

    if !storage::validate_user_storage_path(&decoded) {
        return Err(AppError::Forbidden);
    }

    let is_owned = auth
        .as_ref()
        .is_some_and(|user| storage::path_owned_by_user(&decoded, user.id));
    if !is_owned {
        let problem_id = storage::find_problem_by_asset(&state.pool, &decoded)
            .await?
            .ok_or_else(|| AppError::NotFound("Not found".to_owned()))?;
        let can_view = storage::can_view_problem(
            &state.pool,
            problem_id,
            auth.as_ref().map(|u| u.id),
            auth.as_ref().and_then(|u| u.email.as_deref()),
        )
        .await?;
        let owns_copy = if let Some(user) = &auth {
            storage::user_owns_problem_with_asset(&state.pool, user.id, &decoded).await?
        } else {
            false
        };
        if !can_view && !owns_copy {
            return Err(AppError::NotFound("Not found".to_owned()));
        }
    }

    let (bytes, content_type) =
        storage::read_object(&state, storage::PROBLEM_UPLOADS_BUCKET, &decoded).await?;
    bytes_response(bytes, &content_type)
}

async fn upload_files(
    State(state): State<AppState>,
    auth: AuthUser,
    mut multipart: Multipart,
) -> AppResult<Json<response::ApiSuccess<Value>>> {
    let mut role = None;
    let mut problem_id = None;
    let mut files = Vec::new();

    while let Some(field) = multipart
        .next_field()
        .await
        .map_err(|err| AppError::BadRequest(err.to_string()))?
    {
        let name = field.name().unwrap_or_default().to_owned();
        match name.as_str() {
            "role" => {
                role = Some(
                    field
                        .text()
                        .await
                        .map_err(|err| AppError::BadRequest(err.to_string()))?,
                );
            }
            "problem_id" | "problemId" => {
                problem_id = Some(
                    field
                        .text()
                        .await
                        .map_err(|err| AppError::BadRequest(err.to_string()))?,
                );
            }
            "files" | "files[]" | "file" => {
                let file_name = field.file_name().map(ToOwned::to_owned);
                let content_type = field.content_type().map(ToOwned::to_owned);
                let bytes = field
                    .bytes()
                    .await
                    .map_err(|err| AppError::BadRequest(err.to_string()))?;
                files.push(storage::validate_problem_upload(
                    file_name.as_deref(),
                    content_type.as_deref(),
                    bytes,
                )?);
            }
            _ => {}
        }
    }

    let role = role.ok_or_else(|| AppError::BadRequest("role is required".to_owned()))?;
    if !matches!(role.as_str(), "problem" | "solution") {
        return Err(AppError::BadRequest("Invalid upload role".to_owned()));
    }
    let problem_id =
        problem_id.ok_or_else(|| AppError::BadRequest("problem_id is required".to_owned()))?;
    if problem_id.is_empty() || problem_id.contains('/') || problem_id.contains('\\') {
        return Err(AppError::BadRequest("Invalid problem_id".to_owned()));
    }
    if files.is_empty() {
        return Err(AppError::BadRequest("files[] is required".to_owned()));
    }

    let mut paths = Vec::with_capacity(files.len());
    for file in files {
        let path = format!(
            "user/{}/problems/{}/{}/{}",
            auth.id, problem_id, role, file.file_name
        );
        storage::upload_object(
            &state,
            storage::PROBLEM_UPLOADS_BUCKET,
            &path,
            &file.content_type,
            file.bytes,
            false,
        )
        .await?;
        paths.push(path);
    }

    Ok(response::success(json!({ "paths": paths })))
}

async fn delete_file(
    State(state): State<AppState>,
    auth: AuthUser,
    Json(body): Json<FileDeleteBody>,
) -> AppResult<Json<response::ApiSuccess<Value>>> {
    if !storage::path_owned_by_user(&body.path, auth.id) {
        return Err(AppError::Forbidden);
    }
    storage::delete_object(&state, storage::PROBLEM_UPLOADS_BUCKET, &body.path).await?;
    Ok(response::success(json!({ "ok": true })))
}

fn bytes_response(bytes: Bytes, content_type: &str) -> AppResult<Response> {
    let content_type = HeaderValue::from_str(content_type)
        .map_err(|err| AppError::Internal(format!("Invalid content type: {err}")))?;
    Ok((
        StatusCode::OK,
        [
            (header::CONTENT_TYPE, content_type),
            (
                header::CACHE_CONTROL,
                HeaderValue::from_static("private, max-age=300"),
            ),
        ],
        bytes,
    )
        .into_response())
}
