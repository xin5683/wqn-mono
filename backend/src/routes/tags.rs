use axum::{
    Json, Router,
    extract::{Path, Query, State},
    routing::{get, patch},
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
    models::CreateTag,
    response,
    services::content_limits,
    state::AppState,
};

#[derive(Debug, Deserialize)]
struct TagsQuery {
    subject_id: Uuid,
}

#[derive(Debug, Deserialize)]
struct PatchTag {
    #[serde(default)]
    name: PatchField<String>,
}

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/tags", get(list_tags).post(create_tag))
        .route("/tags/{id}", patch(update_tag).delete(delete_tag))
}

async fn list_tags(
    State(state): State<AppState>,
    auth: AuthUser,
    Query(query): Query<TagsQuery>,
) -> AppResult<Json<response::ApiSuccess<Vec<Value>>>> {
    let rows = sqlx::query_scalar::<_, SqlJson<Value>>(
        r#"
        select to_jsonb(t) as data
        from tags t
        where t.user_id = $1 and t.subject_id = $2
        order by t.name asc
        "#,
    )
    .bind(auth.id)
    .bind(query.subject_id)
    .fetch_all(&state.pool)
    .await?;
    Ok(response::success(
        rows.into_iter().map(|SqlJson(v)| v).collect(),
    ))
}

async fn create_tag(
    State(state): State<AppState>,
    auth: AuthUser,
    Json(body): Json<CreateTag>,
) -> AppResult<impl axum::response::IntoResponse> {
    body.validate().map_err(|err| {
        validation(
            "Invalid request body",
            serde_json::to_value(err).unwrap_or_default(),
        )
    })?;
    let limit = content_limits::check_content_limit(
        &state.pool,
        auth.id,
        content_limits::TAGS_PER_SUBJECT,
        Some(body.subject_id),
    )
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
        insert into tags (user_id, subject_id, name)
        values ($1, $2, $3)
        returning to_jsonb(tags) as data
        "#,
    )
    .bind(auth.id)
    .bind(body.subject_id)
    .bind(body.name)
    .fetch_one(&state.pool)
    .await?;
    Ok(response::created(data.0))
}

async fn update_tag(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(id): Path<Uuid>,
    Json(body): Json<PatchTag>,
) -> AppResult<Json<response::ApiSuccess<Value>>> {
    let (name_present, name) = required_string_patch(body.name, "name", 1, 30)?;
    let data = sqlx::query_scalar::<_, SqlJson<Value>>(
        r#"
        update tags
        set
          name = case when $3 then $4 else name end,
          updated_at = now()
        where id = $1 and user_id = $2
        returning to_jsonb(tags) as data
        "#,
    )
    .bind(id)
    .bind(auth.id)
    .bind(name_present)
    .bind(name)
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

async fn delete_tag(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(id): Path<Uuid>,
) -> AppResult<Json<response::ApiSuccess<Value>>> {
    let affected = sqlx::query("delete from tags where id = $1 and user_id = $2")
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
