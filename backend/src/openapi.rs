use axum::Router;
use utoipa::OpenApi;
use utoipa_swagger_ui::SwaggerUi;

use crate::{
    models,
    response::{ApiErrorBody, ApiSuccess},
};

#[derive(OpenApi)]
#[openapi(
    info(
        title = "Wrong Question Notebook Rust API",
        version = "0.1.0",
        description = "Axum-compatible backend for the existing /api/* surface. Runtime authentication accepts local HS256 Bearer JWTs or the configured HttpOnly session cookie."
    ),
    paths(
        crate::routes::healthz,
        crate::routes::readyz
    ),
    components(
        schemas(
            ApiErrorBody,
            ApiSuccess<serde_json::Value>,
            models::HealthResponse,
            models::ReadyResponse,
            models::CreateSubject,
            models::UpdateSubject,
            models::CreateTag,
            models::UpdateTag,
            models::Asset,
            models::CreateProblem,
            models::UpdateProblem,
            models::CreateAttempt,
            models::UpdateAttempt,
            models::CreateProblemSet,
            models::UpdateProblemSet,
            models::ProblemIdsBody,
            models::UpdateProfile,
            models::StartSpacedSession,
            models::StartInsightsSession,
            models::UpdateSessionProgress,
            models::FileDeleteBody,
            models::AdminSettingBody,
            models::AdminRoleBody,
            models::AdminQuotaBody,
            models::AdminContentLimitBody,
            models::ExtractProblemBody,
            models::UpdateErrorCategorisation
        )
    ),
    tags(
        (name = "system", description = "Health and readiness"),
        (name = "api", description = "Existing /api compatibility routes")
    )
)]
pub struct ApiDoc;

pub fn swagger_router() -> Router<crate::state::AppState> {
    SwaggerUi::new("/swagger-ui")
        .url("/api-docs/openapi.json", ApiDoc::openapi())
        .into()
}
