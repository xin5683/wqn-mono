use axum::{
    Json, Router,
    extract::{Path, Query, State},
    routing::{get, patch, post},
};
use serde::Deserialize;
use serde_json::{Value, json};
use sqlx::types::Json as SqlJson;
use uuid::Uuid;

use crate::{
    auth::AuthUser,
    error::{AppError, AppResult},
    response,
    services::{gemini, quota},
    state::AppState,
};

#[derive(Debug, Deserialize)]
struct CategorisationQuery {
    attempt_id: Uuid,
}

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/ai/extract-problem", post(extract_problem))
        .route("/ai/extract-problem/quota", get(extract_quota))
        .route(
            "/ai/categorise-error",
            get(get_categorisation).post(categorise_error),
        )
        .route(
            "/ai/categorise-error/{id}",
            patch(update_categorisation).delete(reset_categorisation),
        )
}

async fn extract_quota(
    State(state): State<AppState>,
    auth: AuthUser,
) -> AppResult<Json<response::ApiSuccess<Value>>> {
    let quota = quota::quota_usage(&state.pool, auth.id, quota::AI_EXTRACTION).await?;
    Ok(response::success(json!({
        "used": quota.get("current_usage"),
        "limit": quota.get("daily_limit"),
        "remaining": quota.get("remaining"),
    })))
}

async fn extract_problem(
    State(state): State<AppState>,
    auth: AuthUser,
    Json(body): Json<Value>,
) -> AppResult<Json<response::ApiSuccess<Value>>> {
    let quota_result =
        quota::check_and_increment_quota(&state.pool, auth.id, quota::AI_EXTRACTION).await?;
    if !quota_result
        .get("allowed")
        .and_then(Value::as_bool)
        .unwrap_or(false)
        && quota_result
            .get("remaining")
            .and_then(Value::as_i64)
            .unwrap_or(0)
            == 0
    {
        return Err(AppError::Forbidden);
    }
    let extracted = gemini::extract_problem(&state, body).await?;
    Ok(response::success(extracted))
}

async fn categorise_error(
    State(state): State<AppState>,
    auth: AuthUser,
    Json(body): Json<Value>,
) -> AppResult<impl axum::response::IntoResponse> {
    let quota_result =
        quota::check_and_increment_quota(&state.pool, auth.id, quota::AI_CATEGORISATION).await?;
    if !quota_result
        .get("allowed")
        .and_then(Value::as_bool)
        .unwrap_or(false)
        && quota_result
            .get("remaining")
            .and_then(Value::as_i64)
            .unwrap_or(0)
            == 0
    {
        return Err(AppError::Forbidden);
    }
    let result = gemini::categorise_error(&state, body.clone()).await?;
    let attempt_id = body
        .get("attempt_id")
        .and_then(Value::as_str)
        .and_then(|id| Uuid::parse_str(id).ok());
    let problem_id = body
        .get("problem_id")
        .and_then(Value::as_str)
        .and_then(|id| Uuid::parse_str(id).ok());
    let subject_id = body
        .get("subject_id")
        .and_then(Value::as_str)
        .and_then(|id| Uuid::parse_str(id).ok());

    if let (Some(attempt_id), Some(problem_id), Some(subject_id)) =
        (attempt_id, problem_id, subject_id)
    {
        let broad_category = result
            .get("broad_category")
            .and_then(Value::as_str)
            .map(ToOwned::to_owned);
        let granular_tag = result
            .get("granular_tag")
            .and_then(Value::as_str)
            .map(ToOwned::to_owned);
        let topic_label = result
            .get("topic_label")
            .and_then(Value::as_str)
            .map(ToOwned::to_owned);
        let confidence = result.get("confidence").and_then(Value::as_f64);
        let reasoning = result
            .get("reasoning")
            .and_then(Value::as_str)
            .map(ToOwned::to_owned);
        let row = sqlx::query_scalar::<_, SqlJson<Value>>(
            r#"
            insert into error_categorisations (
              user_id, attempt_id, problem_id, subject_id, broad_category,
              granular_tag, topic_label, confidence, reasoning, model_response
            )
            values ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10)
            on conflict (attempt_id)
            do update set
              broad_category = excluded.broad_category,
              granular_tag = excluded.granular_tag,
              topic_label = excluded.topic_label,
              confidence = excluded.confidence,
              reasoning = excluded.reasoning,
              model_response = excluded.model_response,
              updated_at = now()
            returning to_jsonb(error_categorisations) as data
            "#,
        )
        .bind(auth.id)
        .bind(attempt_id)
        .bind(problem_id)
        .bind(subject_id)
        .bind(broad_category)
        .bind(granular_tag)
        .bind(topic_label)
        .bind(confidence)
        .bind(reasoning)
        .bind(SqlJson(result))
        .fetch_one(&state.pool)
        .await?;
        Ok(response::created(row.0))
    } else {
        Ok(response::created(result))
    }
}

async fn get_categorisation(
    State(state): State<AppState>,
    auth: AuthUser,
    Query(query): Query<CategorisationQuery>,
) -> AppResult<Json<response::ApiSuccess<Option<Value>>>> {
    let row = sqlx::query_scalar::<_, SqlJson<Value>>(
        "select to_jsonb(error_categorisations) as data from error_categorisations where attempt_id = $1 and user_id = $2",
    )
    .bind(query.attempt_id)
    .bind(auth.id)
    .fetch_optional(&state.pool)
    .await?;
    Ok(response::success(row.map(|SqlJson(v)| v)))
}

async fn update_categorisation(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(id): Path<Uuid>,
    Json(body): Json<Value>,
) -> AppResult<Json<response::ApiSuccess<Value>>> {
    let existing = sqlx::query_scalar::<_, SqlJson<Value>>(
        "select to_jsonb(error_categorisations) as data from error_categorisations where id = $1 and user_id = $2",
    )
    .bind(id)
    .bind(auth.id)
    .fetch_optional(&state.pool)
    .await?
    .ok_or_else(|| AppError::NotFound("Not found".to_owned()))?
    .0;
    let updated = sqlx::query_scalar::<_, SqlJson<Value>>(
        r#"
        update error_categorisations
        set
          is_user_override = true,
          original_broad_category = coalesce(original_broad_category, $3),
          original_granular_tag = coalesce(original_granular_tag, $4),
          broad_category = coalesce($5, broad_category),
          granular_tag = coalesce($6, granular_tag),
          updated_at = now()
        where id = $1 and user_id = $2
        returning to_jsonb(error_categorisations) as data
        "#,
    )
    .bind(id)
    .bind(auth.id)
    .bind(existing.get("broad_category").and_then(Value::as_str))
    .bind(existing.get("granular_tag").and_then(Value::as_str))
    .bind(body.get("broad_category").and_then(Value::as_str))
    .bind(body.get("granular_tag").and_then(Value::as_str))
    .fetch_one(&state.pool)
    .await?;
    Ok(response::success(updated.0))
}

async fn reset_categorisation(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(id): Path<Uuid>,
) -> AppResult<Json<response::ApiSuccess<Value>>> {
    let restored = sqlx::query_scalar::<_, SqlJson<Value>>(
        r#"
        update error_categorisations
        set
          is_user_override = false,
          broad_category = original_broad_category,
          granular_tag = original_granular_tag,
          original_broad_category = null,
          original_granular_tag = null,
          updated_at = now()
        where id = $1 and user_id = $2 and is_user_override = true
        returning to_jsonb(error_categorisations) as data
        "#,
    )
    .bind(id)
    .bind(auth.id)
    .fetch_optional(&state.pool)
    .await?;
    restored
        .map(|SqlJson(v)| response::success(v))
        .ok_or_else(|| AppError::BadRequest("No override to reset".to_owned()))
}
