use axum::{
    Json, Router,
    extract::{Path, Query, State},
    http::HeaderMap,
    routing::get,
};
use serde_json::{Value, json};

use crate::{
    auth::AuthUser,
    dto::statistics::StatisticsResponse,
    error::{AppError, AppResult},
    response,
    services::misc::{self as misc_service, DiscoverQuery},
    state::AppState,
};

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/announcement", get(announcement))
        .route("/registration-status", get(registration_status))
        .route("/discover", get(discover))
        .route("/statistics", get(statistics))
        .route("/creators/{username}", get(creator))
        .route("/cron/generate-digests", get(generate_digests))
}

async fn announcement(
    State(state): State<AppState>,
) -> AppResult<Json<response::ApiSuccess<Value>>> {
    Ok(response::success(
        misc_service::announcement(&state.pool).await?,
    ))
}

/// Public (no-auth) check of whether self-service sign-up is currently
/// allowed, so the sign-up page can render a "registration closed" notice
/// instead of the form.
async fn registration_status(
    State(state): State<AppState>,
) -> AppResult<Json<response::ApiSuccess<Value>>> {
    let enabled = misc_service::registration_enabled(&state.pool).await?;
    Ok(response::success(json!({ "enabled": enabled })))
}

async fn discover(
    State(state): State<AppState>,
    Query(query): Query<DiscoverQuery>,
) -> AppResult<Json<response::ApiSuccess<Value>>> {
    Ok(response::success(
        misc_service::discover(&state.pool, query).await?,
    ))
}

async fn statistics(
    State(state): State<AppState>,
    auth: AuthUser,
) -> AppResult<Json<response::ApiSuccess<StatisticsResponse>>> {
    Ok(response::success(
        misc_service::statistics(&state.pool, auth.id).await?,
    ))
}

async fn creator(
    State(state): State<AppState>,
    Path(username): Path<String>,
) -> AppResult<Json<response::ApiSuccess<Value>>> {
    Ok(response::success(
        misc_service::creator(&state.pool, username).await?,
    ))
}

async fn generate_digests(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> AppResult<Json<Value>> {
    if let Some(secret) = &state.config.cron_secret {
        let supplied = headers
            .get("authorization")
            .and_then(|v| v.to_str().ok())
            .and_then(|v| v.strip_prefix("Bearer "))
            .or_else(|| headers.get("x-cron-secret").and_then(|v| v.to_str().ok()));
        if supplied != Some(secret.as_str()) {
            return Err(AppError::Unauthorized);
        }
    }
    Ok(Json(misc_service::generate_digests(&state).await?))
}
