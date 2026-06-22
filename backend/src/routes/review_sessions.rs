use axum::{
    Json, Router,
    extract::{Path, Query, State},
    http::StatusCode,
    routing::{get, patch, post},
};
use serde::Deserialize;
use serde_json::{Value, json};
use sqlx::types::Json as SqlJson;
use uuid::Uuid;

use crate::{
    auth::AuthUser,
    error::{AppError, AppResult},
    models::{StartInsightsSession, StartSpacedSession},
    response,
    state::AppState,
};

#[derive(Debug, Deserialize)]
struct ActiveSessionQuery {
    subject_id: Uuid,
}

pub fn router() -> Router<AppState> {
    Router::new()
        .route(
            "/review-sessions/start-spaced",
            get(check_active_spaced).post(start_spaced),
        )
        .route("/review-sessions/start-insights", post(start_insights))
        .route(
            "/review-sessions/{session_id}",
            get(get_session).delete(delete_session),
        )
        .route(
            "/review-sessions/{session_id}/progress",
            patch(update_progress),
        )
        .route(
            "/review-sessions/{session_id}/complete",
            post(complete_session),
        )
}

async fn check_active_spaced(
    State(state): State<AppState>,
    auth: AuthUser,
    Query(query): Query<ActiveSessionQuery>,
) -> AppResult<Json<response::ApiSuccess<Value>>> {
    let row = sqlx::query_as::<_, (Uuid, SqlJson<Value>)>(
        r#"
        select id, session_state from review_session_state
        where user_id = $1 and session_type = 'spaced_repetition'
          and subject_id = $2 and is_active = true
        limit 1
        "#,
    )
    .bind(auth.id)
    .bind(query.subject_id)
    .fetch_optional(&state.pool)
    .await?;
    if let Some((id, SqlJson(session_state))) = row {
        let problem_ids = session_state
            .get("problem_ids")
            .and_then(Value::as_array)
            .map_or(0, Vec::len);
        let completed = session_state
            .get("completed_problem_ids")
            .and_then(Value::as_array)
            .map_or(0, Vec::len);
        let skipped = session_state
            .get("skipped_problem_ids")
            .and_then(Value::as_array)
            .map_or(0, Vec::len);
        Ok(response::success(json!({
            "exists": true,
            "sessionId": id,
            "progress": { "total": problem_ids, "completed": completed, "skipped": skipped }
        })))
    } else {
        Ok(response::success(json!({ "exists": false })))
    }
}

async fn start_spaced(
    State(state): State<AppState>,
    auth: AuthUser,
    Json(body): Json<StartSpacedSession>,
) -> AppResult<impl axum::response::IntoResponse> {
    let limit = body.session_size.unwrap_or(20).clamp(1, 100);
    let problem_ids = sqlx::query_scalar::<_, Uuid>(
        r#"
        select p.id
        from problems p
        left join review_schedule rs on rs.problem_id = p.id and rs.user_id = p.user_id
        where p.user_id = $1 and p.subject_id = $2
          and coalesce(rs.next_review_at, now()) <= now()
        order by coalesce(rs.next_review_at, p.created_at) asc
        limit $3
        "#,
    )
    .bind(auth.id)
    .bind(body.subject_id)
    .bind(limit)
    .fetch_all(&state.pool)
    .await?;
    if problem_ids.is_empty() {
        return Err(AppError::NotFound("No problems due for review".to_owned()));
    }
    create_session(
        &state.pool,
        auth.id,
        "spaced_repetition",
        Some(body.subject_id),
        None,
        problem_ids,
        false,
    )
    .await
}

async fn start_insights(
    State(state): State<AppState>,
    auth: AuthUser,
    Json(body): Json<StartInsightsSession>,
) -> AppResult<impl axum::response::IntoResponse> {
    create_session(
        &state.pool,
        auth.id,
        "insights",
        Some(body.subject_id),
        None,
        body.problem_ids,
        false,
    )
    .await
}

async fn get_session(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(session_id): Path<Uuid>,
) -> AppResult<Json<response::ApiSuccess<Value>>> {
    let session = fetch_session(&state.pool, auth.id, session_id).await?;
    let problem_ids = session
        .get("session_state")
        .and_then(|s| s.get("problem_ids"))
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    let ids: Vec<Uuid> = problem_ids
        .iter()
        .filter_map(Value::as_str)
        .filter_map(|id| Uuid::parse_str(id).ok())
        .collect();
    let problems = if ids.is_empty() {
        Vec::new()
    } else {
        sqlx::query_scalar::<_, SqlJson<Value>>(
            r#"
            select to_jsonb(p) || jsonb_build_object('tags', coalesce(tag_data.tags, '[]'::jsonb)) as data
            from problems p
            left join lateral (
              select jsonb_agg(to_jsonb(t) order by t.name) as tags
              from problem_tag pt join tags t on t.id = pt.tag_id
              where pt.problem_id = p.id
            ) tag_data on true
            where p.id = any($1)
            "#,
        )
        .bind(ids)
        .fetch_all(&state.pool)
        .await?
        .into_iter()
        .map(|SqlJson(v)| v)
        .collect()
    };
    let results = sqlx::query_scalar::<_, SqlJson<Value>>(
        r#"
        select to_jsonb(review_session_results) as data
        from review_session_results
        where session_state_id = $1
        order by completed_at asc
        "#,
    )
    .bind(session_id)
    .fetch_all(&state.pool)
    .await?
    .into_iter()
    .map(|SqlJson(v)| v)
    .collect::<Vec<_>>();
    Ok(response::success(json!({
        "session": session,
        "problems": problems,
        "results": results,
    })))
}

async fn delete_session(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(session_id): Path<Uuid>,
) -> AppResult<Json<response::ApiSuccess<Value>>> {
    sqlx::query(
        "update review_session_state set is_active = false, last_activity_at = now() where id = $1 and user_id = $2",
    )
    .bind(session_id)
    .bind(auth.id)
    .execute(&state.pool)
    .await?;
    Ok(response::success(
        json!({ "id": session_id, "is_active": false }),
    ))
}

async fn update_progress(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(session_id): Path<Uuid>,
    Json(body): Json<Value>,
) -> AppResult<Json<response::ApiSuccess<Value>>> {
    let mut session = fetch_session(&state.pool, auth.id, session_id).await?;
    let mut session_state = session
        .get_mut("session_state")
        .and_then(Value::as_object_mut)
        .map(|m| Value::Object(m.clone()))
        .unwrap_or_else(|| json!({}));

    if let Some(current_index) = body
        .get("currentIndex")
        .or_else(|| body.get("current_index"))
        .and_then(Value::as_i64)
    {
        session_state["current_index"] = json!(current_index);
    }
    if let Some(elapsed_ms) = body.get("elapsed_ms").and_then(Value::as_i64) {
        session_state["elapsed_ms"] = json!(elapsed_ms);
    }
    if let Some(problem_id) = body
        .get("problemId")
        .or_else(|| body.get("problem_id"))
        .and_then(Value::as_str)
    {
        if body.get("wasSkipped").and_then(Value::as_bool) == Some(true) {
            push_unique(&mut session_state, "skipped_problem_ids", problem_id);
        } else if body.get("wasCorrect").and_then(Value::as_bool).is_some() {
            push_unique(&mut session_state, "completed_problem_ids", problem_id);
        }
        if body.get("wasSkipped").is_some() || body.get("wasCorrect").is_some() {
            sqlx::query(
                r#"
                insert into review_session_results (session_state_id, problem_id, was_correct, was_skipped)
                values ($1, $2, $3, coalesce($4, false))
                "#,
            )
            .bind(session_id)
            .bind(Uuid::parse_str(problem_id).map_err(|_| AppError::BadRequest("Invalid problem ID".to_owned()))?)
            .bind(body.get("wasCorrect").and_then(Value::as_bool))
            .bind(body.get("wasSkipped").and_then(Value::as_bool))
            .execute(&state.pool)
            .await?;
        }
    }

    let updated = sqlx::query_scalar::<_, SqlJson<Value>>(
        r#"
        update review_session_state
        set session_state = $3, last_activity_at = now()
        where id = $1 and user_id = $2
        returning to_jsonb(review_session_state) as data
        "#,
    )
    .bind(session_id)
    .bind(auth.id)
    .bind(SqlJson(session_state))
    .fetch_one(&state.pool)
    .await?;
    Ok(response::success(json!({ "session": updated.0 })))
}

async fn complete_session(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(session_id): Path<Uuid>,
) -> AppResult<Json<response::ApiSuccess<Value>>> {
    let session = sqlx::query_scalar::<_, SqlJson<Value>>(
        r#"
        update review_session_state
        set is_active = false, last_activity_at = now()
        where id = $1 and user_id = $2
        returning to_jsonb(review_session_state) as data
        "#,
    )
    .bind(session_id)
    .bind(auth.id)
    .fetch_optional(&state.pool)
    .await?
    .ok_or_else(|| AppError::NotFound("Session not found".to_owned()))?;
    Ok(response::success(json!({
        "session": session.0,
        "summary": session_summary(&session.0),
    })))
}

pub async fn create_session(
    pool: &sqlx::PgPool,
    user_id: Uuid,
    session_type: &str,
    subject_id: Option<Uuid>,
    problem_set_id: Option<Uuid>,
    problem_ids: Vec<Uuid>,
    is_read_only: bool,
) -> AppResult<(StatusCode, Json<response::ApiSuccess<Value>>)> {
    let session_state = json!({
        "problem_ids": problem_ids,
        "current_index": 0,
        "completed_problem_ids": [],
        "skipped_problem_ids": [],
        "elapsed_ms": 0,
        "is_read_only": is_read_only,
    });
    let data = sqlx::query_scalar::<_, SqlJson<Value>>(
        r#"
        insert into review_session_state (
          user_id, session_type, subject_id, problem_set_id, session_state, is_active
        )
        values ($1, $2, $3, $4, $5, true)
        returning to_jsonb(review_session_state) as data
        "#,
    )
    .bind(user_id)
    .bind(session_type)
    .bind(subject_id)
    .bind(problem_set_id)
    .bind(SqlJson(session_state))
    .fetch_one(pool)
    .await?;
    let session_id = data.0.get("id").cloned().unwrap_or(Value::Null);
    Ok(response::created(json!({
        "sessionId": session_id,
        "problemCount": problem_ids.len(),
        "firstProblemId": problem_ids.first(),
        "session": data.0,
    })))
}

async fn fetch_session(pool: &sqlx::PgPool, user_id: Uuid, session_id: Uuid) -> AppResult<Value> {
    let row = sqlx::query_scalar::<_, SqlJson<Value>>(
        "select to_jsonb(review_session_state) as data from review_session_state where id = $1 and user_id = $2",
    )
    .bind(session_id)
    .bind(user_id)
    .fetch_optional(pool)
    .await?;
    row.map(|SqlJson(v)| v)
        .ok_or_else(|| AppError::NotFound("Session not found".to_owned()))
}

fn push_unique(state: &mut Value, key: &str, value: &str) {
    let mut values = state
        .get(key)
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    if !values.iter().any(|v| v.as_str() == Some(value)) {
        values.push(Value::String(value.to_owned()));
    }
    state[key] = Value::Array(values);
}

fn session_summary(session: &Value) -> Value {
    let state = session.get("session_state").unwrap_or(&Value::Null);
    let total = state
        .get("problem_ids")
        .and_then(Value::as_array)
        .map_or(0, Vec::len);
    let completed = state
        .get("completed_problem_ids")
        .and_then(Value::as_array)
        .map_or(0, Vec::len);
    let skipped = state
        .get("skipped_problem_ids")
        .and_then(Value::as_array)
        .map_or(0, Vec::len);
    json!({
        "total": total,
        "completed": completed,
        "skipped": skipped,
    })
}
