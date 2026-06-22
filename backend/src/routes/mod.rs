pub mod admin;
pub mod ai;
pub mod attempts;
pub mod auth;
pub mod files;
pub mod insights;
pub mod misc;
pub mod problem_sets;
pub mod problems;
pub mod profile;
pub mod qr;
pub mod review_sessions;
pub mod subjects;
pub mod tags;
pub mod usage;

use axum::{Json, Router, extract::State, routing::get};

use crate::{
    error::AppError,
    models::{HealthResponse, ReadyResponse},
    response::{self, ApiErrorBody, ApiSuccess},
    state::AppState,
};

pub fn router(_state: AppState) -> Router<AppState> {
    Router::new()
        .route("/healthz", get(healthz))
        .route("/readyz", get(readyz))
        .nest(
            "/api",
            Router::new()
                .merge(subjects::router())
                .merge(auth::router())
                .merge(tags::router())
                .merge(problems::router())
                .merge(attempts::router())
                .merge(problem_sets::router())
                .merge(profile::router())
                .merge(usage::router())
                .merge(files::router())
                .merge(review_sessions::router())
                .merge(ai::router())
                .merge(insights::router())
                .merge(qr::router())
                .merge(admin::router())
                .merge(misc::router()),
        )
}

#[utoipa::path(
    get,
    path = "/healthz",
    tag = "system",
    responses((status = 200, description = "Service health", body = ApiSuccess<HealthResponse>))
)]
pub async fn healthz(State(state): State<AppState>) -> Json<response::ApiSuccess<HealthResponse>> {
    response::success(HealthResponse {
        status: "ok".to_owned(),
        timestamp: response::now_iso(),
        uptime_seconds: state.started_at.elapsed().as_secs(),
    })
}

#[utoipa::path(
    get,
    path = "/readyz",
    tag = "system",
    responses(
        (status = 200, description = "Service readiness", body = ApiSuccess<ReadyResponse>),
        (status = 500, description = "Not ready", body = ApiErrorBody)
    )
)]
pub async fn readyz(
    State(state): State<AppState>,
) -> Result<Json<response::ApiSuccess<ReadyResponse>>, AppError> {
    sqlx::query("select 1").execute(&state.pool).await?;
    Ok(response::success(ReadyResponse {
        status: "ready".to_owned(),
        database: "ok".to_owned(),
        timestamp: response::now_iso(),
    }))
}
