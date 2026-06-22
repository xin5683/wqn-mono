use axum::{
    Json, Router,
    extract::{Path, Query, State},
    routing::{get, patch, post},
};
use serde_json::{Value, json};
use uuid::Uuid;

use crate::{
    auth::AuthUser,
    dto::problem::ProblemDto,
    error::AppResult,
    models::CreateProblem,
    response,
    services::{problems as problem_service, storage},
    state::AppState,
};

use problem_service::{PatchProblem, ProblemsQuery};

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/problems", get(list_problems).post(create_problem))
        .route("/problems/filter-count", post(filter_count))
        .route(
            "/problems/{id}",
            get(get_problem)
                .patch(update_problem)
                .delete(delete_problem),
        )
        .route("/problems/{id}/assets", patch(update_problem_assets))
        .route(
            "/problems/{id}/cleanup",
            post(cleanup_problem).delete(cleanup_problem),
        )
        .route("/problems/{id}/attempt", post(create_problem_attempt))
}

async fn list_problems(
    State(state): State<AppState>,
    auth: AuthUser,
    Query(query): Query<ProblemsQuery>,
) -> AppResult<Json<response::ApiSuccess<Vec<ProblemDto>>>> {
    Ok(response::success(
        problem_service::list_user_problems(&state.pool, auth.id, query).await?,
    ))
}

async fn create_problem(
    State(state): State<AppState>,
    auth: AuthUser,
    Json(body): Json<CreateProblem>,
) -> AppResult<impl axum::response::IntoResponse> {
    Ok(response::created(
        problem_service::create_problem(&state.pool, auth.id, body).await?,
    ))
}

async fn get_problem(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(id): Path<Uuid>,
) -> AppResult<Json<response::ApiSuccess<ProblemDto>>> {
    Ok(response::success(
        problem_service::fetch_problem(&state.pool, auth.id, id).await?,
    ))
}

async fn update_problem(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(id): Path<Uuid>,
    Json(body): Json<PatchProblem>,
) -> AppResult<Json<response::ApiSuccess<ProblemDto>>> {
    Ok(response::success(
        problem_service::update_problem(&state.pool, auth.id, id, body).await?,
    ))
}

async fn delete_problem(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(id): Path<Uuid>,
) -> AppResult<Json<response::ApiSuccess<Value>>> {
    problem_service::delete_problem(&state.pool, auth.id, id).await?;
    Ok(response::empty_ok())
}

async fn update_problem_assets(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(id): Path<Uuid>,
    Json(body): Json<Value>,
) -> AppResult<Json<response::ApiSuccess<ProblemDto>>> {
    Ok(response::success(
        problem_service::update_problem_assets(&state.pool, auth.id, id, &body).await?,
    ))
}

async fn cleanup_problem(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(id): Path<Uuid>,
) -> AppResult<Json<response::ApiSuccess<Value>>> {
    // Draft cleanup is only safe when the problem was NEVER persisted. If a
    // row exists, its files belong to a saved problem — deleting them (or, as
    // the previous implementation did, deleting the row itself) would destroy
    // real data. That was the root cause of "刷新就没了": an unmount fired this
    // endpoint right after a successful create and deleted the new problem.
    //
    // Now this endpoint only ever reclaims orphan draft files for a UUID that
    // has no problem row; it never deletes a row or a saved problem's assets.
    // The background reaper (services::cleanup) is the兜底 for the same class
    // of orphan when the browser's sendBeacon never lands.
    if problem_service::problem_exists(&state.pool, auth.id, id).await? {
        return Ok(response::empty_ok());
    }
    storage::delete_problem_draft_dir(&state, auth.id, id).await?;
    Ok(response::empty_ok())
}

async fn filter_count(
    State(state): State<AppState>,
    auth: AuthUser,
    Json(body): Json<Value>,
) -> AppResult<Json<response::ApiSuccess<Value>>> {
    let count = problem_service::filter_count(&state.pool, auth.id, &body).await?;
    Ok(response::success(json!({ "count": count })))
}

async fn create_problem_attempt(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(id): Path<Uuid>,
    Json(mut body): Json<Value>,
) -> AppResult<impl axum::response::IntoResponse> {
    body["problem_id"] = Value::String(id.to_string());
    super::attempts::create_attempt_inner(state, auth, body).await
}
