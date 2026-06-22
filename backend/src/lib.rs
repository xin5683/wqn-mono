mod auth;
mod config;
mod db;
mod dto;
mod error;
mod middleware;
mod models;
mod openapi;
mod response;
mod routes;
mod services;
mod state;

use axum::Router;
use tower_http::{
    cors::CorsLayer,
    request_id::{MakeRequestUuid, PropagateRequestIdLayer, SetRequestIdLayer},
    sensitive_headers::SetSensitiveRequestHeadersLayer,
    timeout::TimeoutLayer,
    trace::TraceLayer,
};

pub use crate::config::AppConfig;
pub use crate::services::cleanup::spawn_reaper;
pub use crate::state::AppState;

/// Assemble the full application router: the API routes, Swagger UI, and the
/// middleware stack (tracing, request IDs, sensitive-header stripping, timeout,
/// security headers, rate limiting, CORS). Shared by the binary's `main` and by
/// the in-process integration tests under `tests/`.
pub fn build_router(state: AppState) -> Router {
    let request_id = axum::http::HeaderName::from_static("x-request-id");
    let cors = CorsLayer::new()
        .allow_methods(config::allowed_methods())
        .allow_headers(tower_http::cors::Any)
        .allow_origin(state.config.cors_origins.clone());

    Router::new()
        .merge(routes::router(state.clone()))
        .merge(openapi::swagger_router())
        .layer((
            TraceLayer::new_for_http(),
            PropagateRequestIdLayer::new(request_id.clone()),
            SetRequestIdLayer::new(request_id, MakeRequestUuid),
            SetSensitiveRequestHeadersLayer::new(std::iter::once(
                axum::http::header::AUTHORIZATION,
            )),
            TimeoutLayer::with_status_code(
                axum::http::StatusCode::REQUEST_TIMEOUT,
                std::time::Duration::from_secs(30),
            ),
            axum::middleware::from_fn(middleware::security_headers),
            axum::middleware::from_fn_with_state(state.clone(), middleware::rate_limit),
            cors,
        ))
        .with_state(state)
}
