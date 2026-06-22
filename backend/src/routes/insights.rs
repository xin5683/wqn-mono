use axum::{
    Json, Router,
    extract::State,
    routing::{get, post},
};
use serde_json::{Value, json};
use sqlx::types::Json as SqlJson;

use crate::{auth::AuthUser, error::AppResult, response, services::insights, state::AppState};

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/insights/status", get(status))
        .route("/insights/generate", post(generate))
}

async fn status(
    State(state): State<AppState>,
    auth: AuthUser,
) -> AppResult<Json<response::ApiSuccess<Value>>> {
    let latest = sqlx::query_scalar::<_, SqlJson<Value>>(
        r#"
        select to_jsonb(insight_digests) as data
        from insight_digests
        where user_id = $1
        order by created_at desc
        limit 1
        "#,
    )
    .bind(auth.id)
    .fetch_optional(&state.pool)
    .await?
    .map(|SqlJson(v)| insights::flatten_digest_row(v));
    let is_generating = latest
        .as_ref()
        .and_then(|v| v.get("status"))
        .and_then(Value::as_str)
        == Some("generating");
    let status = latest
        .as_ref()
        .and_then(|v| v.get("status"))
        .and_then(Value::as_str)
        .unwrap_or("none");
    let digest = latest
        .as_ref()
        .filter(|v| v.get("status").and_then(Value::as_str) == Some("completed"))
        .cloned();
    Ok(response::success(json!({
        "latest": latest,
        "is_generating": is_generating,
        "status": status,
        "digest": digest,
    })))
}

async fn generate(
    State(state): State<AppState>,
    auth: AuthUser,
) -> AppResult<impl axum::response::IntoResponse> {
    let digest = insights::generate_and_store_digest(&state, auth.id).await?;
    Ok(response::created(digest))
}
