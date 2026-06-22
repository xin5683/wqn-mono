use axum::{
    Json, Router,
    extract::{Path, State},
    routing::get,
};
use serde::Deserialize;
use serde_json::Value;
use sqlx::types::Json as SqlJson;
use uuid::Uuid;
use validator::Validate;

use crate::{
    auth::AuthUser,
    dto::patch::PatchField,
    error::{AppError, AppResult, validation},
    models::CreateSubject,
    response,
    services::content_limits,
    state::AppState,
};

#[derive(Debug, Deserialize)]
struct PatchSubject {
    #[serde(default)]
    name: PatchField<String>,
    #[serde(default)]
    color: PatchField<String>,
    #[serde(default)]
    icon: PatchField<String>,
}

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/subjects", get(list_subjects).post(create_subject))
        .route(
            "/subjects/{id}",
            get(get_subject)
                .patch(update_subject)
                .delete(delete_subject),
        )
}

async fn list_subjects(
    State(state): State<AppState>,
    auth: AuthUser,
) -> AppResult<Json<response::ApiSuccess<Vec<Value>>>> {
    let rows = sqlx::query_scalar::<_, SqlJson<Value>>(
        r#"
        select to_jsonb(s)
          || jsonb_build_object(
            'problem_count', count(distinct p.id),
            'tag_count', count(distinct t.id)
          ) as data
        from subjects s
        left join problems p on p.subject_id = s.id and p.user_id = s.user_id
        left join tags t on t.subject_id = s.id and t.user_id = s.user_id
        where s.user_id = $1
        group by s.id
        order by s.name asc
        "#,
    )
    .bind(auth.id)
    .fetch_all(&state.pool)
    .await?;
    Ok(response::success(
        rows.into_iter().map(|SqlJson(v)| v).collect(),
    ))
}

async fn get_subject(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(id): Path<Uuid>,
) -> AppResult<Json<response::ApiSuccess<Value>>> {
    let row = sqlx::query_scalar::<_, SqlJson<Value>>(
        r#"
        select to_jsonb(s)
          || jsonb_build_object(
            'problem_count', count(distinct p.id),
            'tag_count', count(distinct t.id)
          ) as data
        from subjects s
        left join problems p on p.subject_id = s.id and p.user_id = s.user_id
        left join tags t on t.subject_id = s.id and t.user_id = s.user_id
        where s.id = $1 and s.user_id = $2
        group by s.id
        "#,
    )
    .bind(id)
    .bind(auth.id)
    .fetch_optional(&state.pool)
    .await?;
    row.map(|SqlJson(v)| response::success(v))
        .ok_or_else(|| AppError::NotFound("Not found".to_owned()))
}

async fn create_subject(
    State(state): State<AppState>,
    auth: AuthUser,
    Json(body): Json<CreateSubject>,
) -> AppResult<impl axum::response::IntoResponse> {
    body.validate().map_err(|err| {
        validation(
            "Invalid request body",
            serde_json::to_value(err).unwrap_or_default(),
        )
    })?;
    let limit =
        content_limits::check_content_limit(&state.pool, auth.id, content_limits::SUBJECTS, None)
            .await?;
    if !limit
        .get("allowed")
        .and_then(Value::as_bool)
        .unwrap_or(false)
    {
        return Err(AppError::Forbidden);
    }

    let data = sqlx::query_scalar::<_, SqlJson<Value>>(
        r#"
        insert into subjects (user_id, name, color, icon)
        values ($1, $2, $3, $4)
        returning to_jsonb(subjects) as data
        "#,
    )
    .bind(auth.id)
    .bind(body.name)
    .bind(body.color)
    .bind(body.icon)
    .fetch_one(&state.pool)
    .await?;
    Ok(response::created(data.0))
}

async fn update_subject(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(id): Path<Uuid>,
    Json(body): Json<PatchSubject>,
) -> AppResult<Json<response::ApiSuccess<Value>>> {
    let (name_present, name) = required_string_patch(body.name, "name", 1, 30)?;
    let (color_present, color) = body.color.into_nullable();
    let (icon_present, icon) = body.icon.into_nullable();
    let data = sqlx::query_scalar::<_, SqlJson<Value>>(
        r#"
        update subjects
        set
          name = case when $3 then $4 else name end,
          color = case when $5 then $6 else color end,
          icon = case when $7 then $8 else icon end,
          updated_at = now()
        where id = $1 and user_id = $2
        returning to_jsonb(subjects) as data
        "#,
    )
    .bind(id)
    .bind(auth.id)
    .bind(name_present)
    .bind(name)
    .bind(color_present)
    .bind(color)
    .bind(icon_present)
    .bind(icon)
    .fetch_optional(&state.pool)
    .await?;

    data.map(|SqlJson(v)| response::success(v))
        .ok_or_else(|| AppError::NotFound("Not found".to_owned()))
}

fn required_string_patch(
    field: PatchField<String>,
    field_name: &str,
    min: usize,
    max: usize,
) -> AppResult<(bool, Option<String>)> {
    match field {
        PatchField::Missing => Ok((false, None)),
        PatchField::Null => Err(AppError::BadRequest(format!("{field_name} cannot be null"))),
        PatchField::Value(value) => {
            let len = value.chars().count();
            if len < min || len > max {
                return Err(validation(
                    "Invalid request body",
                    serde_json::json!({ field_name: format!("length must be between {min} and {max}") }),
                ));
            }
            Ok((true, Some(value)))
        }
    }
}

async fn delete_subject(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(id): Path<Uuid>,
) -> AppResult<Json<response::ApiSuccess<Value>>> {
    let affected = sqlx::query("delete from subjects where id = $1 and user_id = $2")
        .bind(id)
        .bind(auth.id)
        .execute(&state.pool)
        .await?
        .rows_affected();
    if affected == 0 {
        return Err(AppError::NotFound("Not found".to_owned()));
    }
    Ok(response::empty_ok())
}
