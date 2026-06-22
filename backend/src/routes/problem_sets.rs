use axum::{
    Json, Router,
    extract::{Path, Query, State},
    routing::{get, post},
};
use serde::Deserialize;
use serde_json::{Value, json};
use uuid::Uuid;

use crate::{
    auth::{AuthUser, OptionalAuthUser},
    dto::problem_set::{ProblemSetDto, ProblemSetProblemDto},
    error::{AppError, AppResult},
    models::{CopyProblemSetBody, CreateProblemSet, ProblemIdsBody, ReportProblemSetBody},
    response,
    services::problem_sets::{self as problem_set_service, PatchProblemSet},
    state::AppState,
};

#[derive(Debug, Deserialize)]
struct ProblemSetsQuery {
    subject_id: Option<Uuid>,
}

pub fn router() -> Router<AppState> {
    Router::new()
        .route(
            "/problem-sets",
            get(list_problem_sets).post(create_problem_set),
        )
        .route("/problem-sets/favourites", get(list_favourites))
        .route(
            "/problem-sets/{id}",
            get(get_problem_set)
                .put(update_problem_set)
                .delete(delete_problem_set),
        )
        .route(
            "/problem-sets/{id}/problems",
            get(list_problem_set_problems)
                .post(add_problems_to_set)
                .delete(remove_problems_from_set),
        )
        .route("/problem-sets/{id}/like", post(toggle_like))
        .route("/problem-sets/{id}/favourite", post(toggle_favourite))
        .route("/problem-sets/{id}/stats", get(problem_set_stats))
        .route("/problem-sets/{id}/view", post(record_view))
        .route("/problem-sets/{id}/report", post(report_problem_set))
        .route("/problem-sets/{id}/progress", get(problem_set_progress))
        .route("/problem-sets/{id}/start-session", post(start_session))
        .route("/problem-sets/{id}/copy", post(copy_problem_set))
        .route("/problem-sets/{id}/copy-problem", post(copy_problem))
}

async fn list_problem_sets(
    State(state): State<AppState>,
    auth: AuthUser,
    Query(query): Query<ProblemSetsQuery>,
) -> AppResult<Json<response::ApiSuccess<Vec<ProblemSetDto>>>> {
    Ok(response::success(
        problem_set_service::list_user_problem_sets(&state.pool, auth.id, query.subject_id).await?,
    ))
}

async fn create_problem_set(
    State(state): State<AppState>,
    auth: AuthUser,
    Json(body): Json<CreateProblemSet>,
) -> AppResult<impl axum::response::IntoResponse> {
    Ok(response::created(
        problem_set_service::create_problem_set(&state.pool, auth.id, body).await?,
    ))
}

async fn get_problem_set(
    State(state): State<AppState>,
    OptionalAuthUser(auth): OptionalAuthUser,
    Path(id): Path<Uuid>,
) -> AppResult<Json<response::ApiSuccess<ProblemSetDto>>> {
    if !problem_set_service::can_view_problem_set(
        &state.pool,
        id,
        auth.as_ref().map(|u| u.id),
        auth.as_ref().and_then(|u| u.email.as_deref()),
    )
    .await?
    {
        return Err(AppError::NotFound(
            "Problem set not found or access denied".to_owned(),
        ));
    }
    Ok(response::success(
        problem_set_service::fetch_problem_set_full(&state.pool, id).await?,
    ))
}

async fn update_problem_set(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(id): Path<Uuid>,
    Json(body): Json<PatchProblemSet>,
) -> AppResult<Json<response::ApiSuccess<ProblemSetDto>>> {
    Ok(response::success(
        problem_set_service::update_problem_set(&state.pool, auth.id, id, body).await?,
    ))
}

async fn delete_problem_set(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(id): Path<Uuid>,
) -> AppResult<Json<response::ApiSuccess<Value>>> {
    problem_set_service::delete_problem_set(&state.pool, auth.id, id).await?;
    Ok(response::success(json!({ "id": id })))
}

async fn list_problem_set_problems(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(id): Path<Uuid>,
) -> AppResult<Json<response::ApiSuccess<Vec<ProblemSetProblemDto>>>> {
    Ok(response::success(
        problem_set_service::list_problem_set_problems(&state.pool, auth.id, id).await?,
    ))
}

async fn add_problems_to_set(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(id): Path<Uuid>,
    Json(body): Json<ProblemIdsBody>,
) -> AppResult<Json<response::ApiSuccess<Value>>> {
    let added =
        problem_set_service::add_problems_to_set(&state.pool, auth.id, id, body.problem_ids)
            .await?;
    Ok(response::success(json!({
        "added_count": added,
        "skipped_count": 0,
    })))
}

async fn remove_problems_from_set(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(id): Path<Uuid>,
    Json(body): Json<ProblemIdsBody>,
) -> AppResult<Json<response::ApiSuccess<Value>>> {
    let problem_ids = body.problem_ids;
    let affected =
        problem_set_service::remove_problems_from_set(&state.pool, auth.id, id, &problem_ids)
            .await?;
    Ok(response::success(json!({
        "removed_count": affected,
        "removed_problem_ids": problem_ids,
    })))
}

async fn toggle_like(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(id): Path<Uuid>,
) -> AppResult<Json<response::ApiSuccess<Value>>> {
    Ok(response::success(
        problem_set_service::toggle_like(&state.pool, id, auth.id).await?,
    ))
}

async fn toggle_favourite(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(id): Path<Uuid>,
) -> AppResult<Json<response::ApiSuccess<Value>>> {
    Ok(response::success(
        problem_set_service::toggle_favourite(&state.pool, id, auth.id).await?,
    ))
}

async fn problem_set_stats(
    State(state): State<AppState>,
    OptionalAuthUser(auth): OptionalAuthUser,
    Path(id): Path<Uuid>,
) -> AppResult<Json<response::ApiSuccess<Value>>> {
    Ok(response::success(
        problem_set_service::stats(&state.pool, id, auth.map(|u| u.id)).await?,
    ))
}

async fn record_view(
    State(state): State<AppState>,
    OptionalAuthUser(auth): OptionalAuthUser,
    Path(id): Path<Uuid>,
) -> AppResult<Json<response::ApiSuccess<Value>>> {
    problem_set_service::record_problem_set_view(&state.pool, id, auth.map(|u| u.id), None).await?;
    Ok(response::success(json!({ "success": true })))
}

async fn report_problem_set(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(id): Path<Uuid>,
    Json(body): Json<ReportProblemSetBody>,
) -> AppResult<Json<response::ApiSuccess<Value>>> {
    problem_set_service::report_problem_set(&state.pool, auth.id, id, body).await?;
    Ok(response::success(json!({ "ok": true })))
}

async fn problem_set_progress(
    State(state): State<AppState>,
    OptionalAuthUser(auth): OptionalAuthUser,
    Path(id): Path<Uuid>,
) -> AppResult<Json<response::ApiSuccess<Value>>> {
    Ok(response::success(
        problem_set_service::problem_set_progress(&state.pool, id, auth.map(|u| u.id)).await?,
    ))
}

async fn start_session(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(id): Path<Uuid>,
) -> AppResult<impl axum::response::IntoResponse> {
    let problems = problem_set_service::problem_ids_for_set(&state.pool, id).await?;
    crate::routes::review_sessions::create_session(
        &state.pool,
        auth.id,
        "problem_set",
        None,
        Some(id),
        problems,
        false,
    )
    .await
}

async fn list_favourites(
    State(state): State<AppState>,
    auth: AuthUser,
) -> AppResult<Json<response::ApiSuccess<Vec<ProblemSetDto>>>> {
    Ok(response::success(
        problem_set_service::list_favourites(&state.pool, auth.id).await?,
    ))
}

async fn copy_problem_set(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(id): Path<Uuid>,
    Json(body): Json<CopyProblemSetBody>,
) -> AppResult<impl axum::response::IntoResponse> {
    Ok(response::created(
        problem_set_service::copy_problem_set(&state.pool, auth.id, id, body).await?,
    ))
}

async fn copy_problem(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(id): Path<Uuid>,
    Json(body): Json<Value>,
) -> AppResult<impl axum::response::IntoResponse> {
    let source_problem_id = body
        .get("problem_id")
        .and_then(Value::as_str)
        .and_then(|id| Uuid::parse_str(id).ok())
        .ok_or_else(|| AppError::BadRequest("problem_id is required".to_owned()))?;
    let target_subject_id = body
        .get("target_subject_id")
        .and_then(Value::as_str)
        .and_then(|id| Uuid::parse_str(id).ok())
        .ok_or_else(|| AppError::BadRequest("target_subject_id is required".to_owned()))?;
    Ok(response::created(
        problem_set_service::copy_problem(
            &state.pool,
            auth.id,
            id,
            source_problem_id,
            target_subject_id,
        )
        .await?,
    ))
}
