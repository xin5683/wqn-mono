use axum::{
    Json, Router,
    extract::{Path, Query, State},
    routing::get,
};
use serde::Deserialize;
use serde_json::{Value, json};

use crate::{
    auth::AuthUser,
    error::{AppError, AppResult},
    response,
    services::{content_limits, quota},
    state::AppState,
};

#[derive(Debug, Deserialize)]
struct UsageQuery {
    subject_id: Option<uuid::Uuid>,
}

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/usage", get(get_usage))
        .route("/usage/{resource_type}", get(get_usage_resource))
}

async fn get_usage(
    State(state): State<AppState>,
    auth: AuthUser,
) -> AppResult<Json<response::ApiSuccess<Value>>> {
    let content_limits = content_limits::all_content_limits(&state.pool, auth.id).await?;
    let ai_extraction = quota::quota_usage(&state.pool, auth.id, quota::AI_EXTRACTION).await?;
    let ai_categorisation =
        quota::quota_usage(&state.pool, auth.id, quota::AI_CATEGORISATION).await?;
    Ok(response::success(json!({
        "content_limits": content_limits,
        "daily_quotas": {
            "ai_extraction": ai_extraction,
            "ai_categorisation": ai_categorisation,
        }
    })))
}

async fn get_usage_resource(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(resource_type): Path<String>,
    Query(query): Query<UsageQuery>,
) -> AppResult<Json<response::ApiSuccess<Value>>> {
    if !matches!(
        resource_type.as_str(),
        content_limits::SUBJECTS
            | content_limits::PROBLEMS_PER_SUBJECT
            | content_limits::PROBLEM_SETS
            | content_limits::TAGS_PER_SUBJECT
            | content_limits::STORAGE_BYTES
    ) {
        return Err(AppError::BadRequest("Invalid resource type".to_owned()));
    }
    Ok(response::success(
        content_limits::check_content_limit(&state.pool, auth.id, &resource_type, query.subject_id)
            .await?,
    ))
}
