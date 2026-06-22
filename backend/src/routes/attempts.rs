use axum::{
    Json, Router,
    extract::{Path, Query, State},
    routing::{get, patch},
};
use serde::Deserialize;
use serde_json::{Value, json};
use sqlx::types::Json as SqlJson;
use uuid::Uuid;
use validator::Validate;

use crate::{
    auth::AuthUser,
    dto::patch::PatchField,
    error::{AppError, AppResult, validation},
    models::{AnswerConfig, CreateAttempt, ShortAnswerMode},
    response,
    services::{problem_filters::validate_statuses, review},
    state::AppState,
};

#[derive(Debug, Deserialize)]
struct AttemptsQuery {
    problem_id: Uuid,
}

#[derive(Debug, Deserialize)]
struct PatchAttempt {
    #[serde(default)]
    submitted_answer: PatchField<Value>,
    #[serde(default)]
    confidence: PatchField<i32>,
    #[serde(default)]
    cause: PatchField<String>,
    #[serde(default)]
    reflection_notes: PatchField<String>,
    #[serde(default)]
    selected_status: PatchField<String>,
}

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/attempts", get(list_attempts).post(create_attempt))
        .route("/attempts/{id}", patch(update_attempt))
}

async fn list_attempts(
    State(state): State<AppState>,
    auth: AuthUser,
    Query(query): Query<AttemptsQuery>,
) -> AppResult<Json<response::ApiSuccess<Vec<Value>>>> {
    let rows = sqlx::query_scalar::<_, SqlJson<Value>>(
        r#"
        select to_jsonb(a) as data
        from attempts a
        where a.user_id = $1 and a.problem_id = $2
        order by a.created_at desc
        "#,
    )
    .bind(auth.id)
    .bind(query.problem_id)
    .fetch_all(&state.pool)
    .await?;
    Ok(response::success(
        rows.into_iter().map(|SqlJson(v)| v).collect(),
    ))
}

async fn create_attempt(
    State(state): State<AppState>,
    auth: AuthUser,
    Json(body): Json<CreateAttempt>,
) -> AppResult<impl axum::response::IntoResponse> {
    body.validate().map_err(|err| {
        validation(
            "Invalid request body",
            serde_json::to_value(err).unwrap_or_default(),
        )
    })?;
    create_attempt_typed(state, auth, body).await
}

pub async fn create_attempt_inner(
    state: AppState,
    auth: AuthUser,
    body: Value,
) -> AppResult<impl axum::response::IntoResponse> {
    let body: CreateAttempt =
        serde_json::from_value(body).map_err(|err| AppError::BadRequest(err.to_string()))?;
    body.validate().map_err(|err| {
        validation(
            "Invalid request body",
            serde_json::to_value(err).unwrap_or_default(),
        )
    })?;
    create_attempt_typed(state, auth, body).await
}

async fn create_attempt_typed(
    state: AppState,
    auth: AuthUser,
    body: CreateAttempt,
) -> AppResult<impl axum::response::IntoResponse> {
    #[derive(sqlx::FromRow)]
    struct ProblemForGrading {
        subject_id: Uuid,
        problem_type: String,
        answer_config: Option<SqlJson<Value>>,
        correct_answer: Option<String>,
        auto_mark: bool,
    }

    let problem = sqlx::query_as::<_, ProblemForGrading>(
        r#"
        select subject_id, problem_type, answer_config, correct_answer, auto_mark
        from problems
        where id = $1 and user_id = $2
        "#,
    )
    .bind(body.problem_id)
    .bind(auth.id)
    .fetch_optional(&state.pool)
    .await?
    .ok_or_else(|| AppError::NotFound("Not found".to_owned()))?;

    // Compute correctness server-side for auto-marked problems when the
    // client did not already supply it. The self-assessment path
    // (AttemptStatusForm) sends `is_correct` explicitly and is trusted as-is;
    // the auto-mark submit path omits it, so the backend grades against the
    // problem's answer_config (or legacy correct_answer). Mirrors the
    // frontend `markAnswer` logic so grading is authoritative.
    let is_correct = if let Some(client_correct) = body.is_correct {
        Some(client_correct)
    } else if problem.auto_mark {
        let parsed_config = problem
            .answer_config
            .as_ref()
            .and_then(|SqlJson(v)| serde_json::from_value::<AnswerConfig>(v.clone()).ok());
        grade_answer(
            &problem.problem_type,
            parsed_config.as_ref(),
            problem.correct_answer.as_deref(),
            &body.submitted_answer,
        )
    } else {
        None
    };

    let data = sqlx::query_scalar::<_, SqlJson<Value>>(
        r#"
        insert into attempts (
          user_id, problem_id, submitted_answer, is_correct, cause,
          is_self_assessed, confidence, reflection_notes, selected_status
        )
        values ($1, $2, $3, $4, $5, coalesce($6, false), $7, $8, $9)
        returning to_jsonb(attempts) as data
        "#,
    )
    .bind(auth.id)
    .bind(body.problem_id)
    .bind(SqlJson(body.submitted_answer))
    .bind(is_correct)
    .bind(body.cause)
    .bind(body.is_self_assessed)
    .bind(body.confidence)
    .bind(body.reflection_notes)
    .bind(body.selected_status.clone())
    .fetch_one(&state.pool)
    .await?;

    let selected_status = body.selected_status.or_else(|| {
        is_correct.map(|correct| if correct { "mastered" } else { "wrong" }.to_owned())
    });
    if let Some(status) = selected_status {
        sqlx::query(
            r#"
            update problems
            set status = $3, last_reviewed_date = now(), updated_at = now()
            where id = $1 and user_id = $2
            "#,
        )
        .bind(body.problem_id)
        .bind(auth.id)
        .bind(&status)
        .execute(&state.pool)
        .await?;
        review::update_review_schedule(&state.pool, auth.id, body.problem_id, &status).await?;
    }

    log_attempt_subject_touch(&state.pool, auth.id, problem.subject_id).await?;
    Ok(response::created(data.0))
}

/// Server-side grading for auto-marked problems. Returns `Some(bool)` when
/// the answer could be graded, or `None` when it cannot (extended problems,
/// answer-config/problem-type mismatches, or a numeric config missing its
/// `numeric_config`/unparseable submission). A `None` result leaves
/// `is_correct` NULL so the problem is not wrongly marked wrong by a
/// misconfiguration — the caller can still self-assess. Mirrors the frontend
/// `markAnswer` logic in `web/lib/answer-marking.ts`.
fn grade_answer(
    problem_type: &str,
    answer_config: Option<&AnswerConfig>,
    legacy_correct_answer: Option<&str>,
    submitted: &Value,
) -> Option<bool> {
    if problem_type == "extended" {
        return None;
    }
    match answer_config {
        Some(AnswerConfig::Mcq {
            correct_choice_id, ..
        }) => {
            if problem_type != "mcq" {
                return None;
            }
            let submitted = submitted.as_str().unwrap_or("").trim();
            Some(submitted == correct_choice_id.trim())
        }
        Some(AnswerConfig::Short {
            mode,
            acceptable_answers,
            numeric_config,
        }) => {
            if problem_type != "short" {
                return None;
            }
            match mode {
                ShortAnswerMode::Text => {
                    let submitted = submitted
                        .as_str()
                        .unwrap_or("")
                        .trim()
                        .to_lowercase();
                    let accepted = acceptable_answers.as_deref().unwrap_or_default();
                    Some(
                        accepted
                            .iter()
                            .any(|a| a.trim().to_lowercase() == submitted),
                    )
                }
                ShortAnswerMode::Numeric => {
                    let numeric = numeric_config.as_ref()?;
                    let submitted = value_as_f64(submitted)?;
                    Some((submitted - numeric.correct_value).abs() <= numeric.tolerance)
                }
            }
        }
        None => {
            // Legacy fallback: case-insensitive comparison against
            // correct_answer. Only reached for problems created before
            // answer_config existed (or without one).
            let correct = legacy_correct_answer?;
            let submitted = submitted
                .as_str()
                .unwrap_or("")
                .trim()
                .to_lowercase();
            Some(submitted == correct.trim().to_lowercase())
        }
    }
}

fn value_as_f64(v: &Value) -> Option<f64> {
    match v {
        Value::Number(n) => n.as_f64(),
        Value::String(s) => s.trim().parse::<f64>().ok(),
        _ => None,
    }
}

async fn update_attempt(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(id): Path<Uuid>,
    Json(body): Json<PatchAttempt>,
) -> AppResult<Json<response::ApiSuccess<Value>>> {
    if let Some(confidence) = body.confidence.as_nullable_value()
        && !(1..=5).contains(confidence)
    {
        return Err(validation(
            "Invalid request body",
            json!({ "confidence": "must be between 1 and 5" }),
        ));
    }
    if let Some(cause) = body.cause.as_nullable_value()
        && cause.chars().count() > 1000
    {
        return Err(validation(
            "Invalid request body",
            json!({ "cause": "length must be at most 1000" }),
        ));
    }
    if let Some(notes) = body.reflection_notes.as_nullable_value()
        && notes.chars().count() > 5000
    {
        return Err(validation(
            "Invalid request body",
            json!({ "reflection_notes": "length must be at most 5000" }),
        ));
    }
    if let Some(status) = body.selected_status.as_nullable_value() {
        validate_statuses(std::slice::from_ref(status))?;
    }

    let (submitted_answer_present, submitted_answer) =
        required_patch(body.submitted_answer, "submitted_answer")?;
    let (confidence_present, confidence) = body.confidence.into_nullable();
    let (cause_present, cause) = body.cause.into_nullable();
    let (reflection_notes_present, reflection_notes) = body.reflection_notes.into_nullable();
    let (selected_status_present, selected_status) = body.selected_status.into_nullable();

    let data = sqlx::query_scalar::<_, SqlJson<Value>>(
        r#"
        update attempts
        set
          submitted_answer = case when $3 then $4 else submitted_answer end,
          confidence = case when $5 then $6 else confidence end,
          cause = case when $7 then $8 else cause end,
          reflection_notes = case when $9 then $10 else reflection_notes end,
          selected_status = case when $11 then $12 else selected_status end,
          updated_at = now()
        where id = $1 and user_id = $2
        returning to_jsonb(attempts) as data
        "#,
    )
    .bind(id)
    .bind(auth.id)
    .bind(submitted_answer_present)
    .bind(submitted_answer.map(SqlJson))
    .bind(confidence_present)
    .bind(confidence)
    .bind(cause_present)
    .bind(cause)
    .bind(reflection_notes_present)
    .bind(reflection_notes)
    .bind(selected_status_present)
    .bind(selected_status)
    .fetch_optional(&state.pool)
    .await?;

    data.map(|SqlJson(v)| response::success(v))
        .ok_or_else(|| AppError::NotFound("Not found".to_owned()))
}

fn required_patch<T>(field: PatchField<T>, field_name: &str) -> AppResult<(bool, Option<T>)> {
    match field {
        PatchField::Missing => Ok((false, None)),
        PatchField::Null => Err(AppError::BadRequest(format!("{field_name} cannot be null"))),
        PatchField::Value(value) => Ok((true, Some(value))),
    }
}

async fn log_attempt_subject_touch(
    pool: &sqlx::PgPool,
    user_id: Uuid,
    subject_id: Uuid,
) -> AppResult<()> {
    sqlx::query(
        r#"
        insert into user_activity_log (user_id, action, resource_type, resource_id, details)
        values ($1, 'attempt_created', 'subject', $2, $3)
        "#,
    )
    .bind(user_id)
    .bind(subject_id)
    .bind(SqlJson(json!({ "source": "rust-api" })))
    .execute(pool)
    .await?;
    Ok(())
}
