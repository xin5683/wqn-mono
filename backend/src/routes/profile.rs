use axum::{
    Json, Router,
    extract::{Multipart, Query, State},
    routing::{get, post},
};
use chrono::NaiveDate;
use serde::Deserialize;
use serde_json::{Value, json};
use sqlx::types::Json as SqlJson;

use crate::{
    auth::AuthUser,
    dto::patch::PatchField,
    error::{AppError, AppResult},
    response,
    services::storage,
    state::AppState,
};

#[derive(Debug, Deserialize)]
struct UsernameQuery {
    username: String,
}

#[derive(Debug, Deserialize)]
struct PatchProfile {
    #[serde(default)]
    username: PatchField<String>,
    #[serde(default)]
    first_name: PatchField<String>,
    #[serde(default)]
    last_name: PatchField<String>,
    #[serde(default)]
    date_of_birth: PatchField<String>,
    #[serde(default)]
    gender: PatchField<String>,
    #[serde(default)]
    region: PatchField<String>,
    #[serde(default)]
    timezone: PatchField<String>,
    #[serde(default)]
    avatar_url: PatchField<String>,
    #[serde(default)]
    bio: PatchField<String>,
}

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/profile", get(get_profile).patch(update_profile))
        .route("/profile/username-check", get(username_check))
        .route("/profile/avatar", post(upload_avatar).delete(delete_avatar))
        .route("/onboarding/status", get(onboarding_status))
        .route("/onboarding/complete", post(complete_onboarding))
}

async fn get_profile(
    State(state): State<AppState>,
    auth: AuthUser,
) -> AppResult<Json<response::ApiSuccess<Value>>> {
    let row = sqlx::query_scalar::<_, SqlJson<Value>>(
        "select to_jsonb(user_profiles) as data from user_profiles where id = $1",
    )
    .bind(auth.id)
    .fetch_optional(&state.pool)
    .await?;
    row.map(|SqlJson(v)| response::success(v))
        .ok_or_else(|| AppError::NotFound("Profile not found".to_owned()))
}

async fn update_profile(
    State(state): State<AppState>,
    auth: AuthUser,
    Json(body): Json<PatchProfile>,
) -> AppResult<Json<response::ApiSuccess<Value>>> {
    if let Some(username) = body.username.as_nullable_value().filter(|s| !s.is_empty()) {
        let taken = sqlx::query_scalar::<_, bool>(
            "select exists(select 1 from user_profiles where username = $1 and id <> $2)",
        )
        .bind(username)
        .bind(auth.id)
        .fetch_one(&state.pool)
        .await?;
        if taken {
            return Err(AppError::Conflict("Username is already taken".to_owned()));
        }
    }

    let (username_present, username) = nullable_string_patch(body.username);
    let (first_name_present, first_name) = nullable_string_patch(body.first_name);
    let (last_name_present, last_name) = nullable_string_patch(body.last_name);
    let (date_of_birth_present, date_of_birth) = date_patch(body.date_of_birth)?;
    let (gender_present, gender) = nullable_string_patch(body.gender);
    let (region_present, region) = nullable_string_patch(body.region);
    let (timezone_present, timezone) = required_nonempty_string_patch(body.timezone, "timezone")?;
    let (avatar_url_present, avatar_url) = nullable_string_patch(body.avatar_url);
    let (bio_present, bio) = nullable_string_patch(body.bio);

    sqlx::query("insert into user_profiles (id) values ($1) on conflict (id) do nothing")
        .bind(auth.id)
        .execute(&state.pool)
        .await?;
    let updated = sqlx::query_scalar::<_, SqlJson<Value>>(
        r#"
        update user_profiles
        set
          username = case when $2 then $3 else username end,
          first_name = case when $4 then $5 else first_name end,
          last_name = case when $6 then $7 else last_name end,
          date_of_birth = case when $8 then $9 else date_of_birth end,
          gender = case when $10 then $11 else gender end,
          region = case when $12 then $13 else region end,
          timezone = case when $14 then $15 else timezone end,
          avatar_url = case when $16 then $17 else avatar_url end,
          bio = case when $18 then $19 else bio end,
          updated_at = now()
        where id = $1
        returning to_jsonb(user_profiles) as data
        "#,
    )
    .bind(auth.id)
    .bind(username_present)
    .bind(username)
    .bind(first_name_present)
    .bind(first_name)
    .bind(last_name_present)
    .bind(last_name)
    .bind(date_of_birth_present)
    .bind(date_of_birth)
    .bind(gender_present)
    .bind(gender)
    .bind(region_present)
    .bind(region)
    .bind(timezone_present)
    .bind(timezone)
    .bind(avatar_url_present)
    .bind(avatar_url)
    .bind(bio_present)
    .bind(bio)
    .fetch_one(&state.pool)
    .await?;
    Ok(response::success(updated.0))
}

fn nullable_string_patch(field: PatchField<String>) -> (bool, Option<String>) {
    match field {
        PatchField::Missing => (false, None),
        PatchField::Null => (true, None),
        PatchField::Value(value) => {
            let value = value.trim().to_owned();
            (true, (!value.is_empty()).then_some(value))
        }
    }
}

fn required_nonempty_string_patch(
    field: PatchField<String>,
    field_name: &str,
) -> AppResult<(bool, Option<String>)> {
    match field {
        PatchField::Missing => Ok((false, None)),
        PatchField::Null => Err(AppError::BadRequest(format!("{field_name} cannot be null"))),
        PatchField::Value(value) => {
            let value = value.trim().to_owned();
            if value.is_empty() {
                return Err(AppError::BadRequest(format!(
                    "{field_name} cannot be empty"
                )));
            }
            Ok((true, Some(value)))
        }
    }
}

fn date_patch(field: PatchField<String>) -> AppResult<(bool, Option<NaiveDate>)> {
    match field {
        PatchField::Missing => Ok((false, None)),
        PatchField::Null => Ok((true, None)),
        PatchField::Value(value) => {
            let value = value.trim();
            if value.is_empty() {
                return Ok((true, None));
            }
            let date = NaiveDate::parse_from_str(value, "%Y-%m-%d")
                .map_err(|_| AppError::BadRequest("date_of_birth must be YYYY-MM-DD".to_owned()))?;
            Ok((true, Some(date)))
        }
    }
}

async fn username_check(
    State(state): State<AppState>,
    auth: AuthUser,
    Query(query): Query<UsernameQuery>,
) -> AppResult<Json<response::ApiSuccess<Value>>> {
    let taken = sqlx::query_scalar::<_, bool>(
        "select exists(select 1 from user_profiles where username = $1 and id <> $2)",
    )
    .bind(query.username)
    .bind(auth.id)
    .fetch_one(&state.pool)
    .await?;
    Ok(response::success(json!({ "available": !taken })))
}

async fn upload_avatar(
    State(state): State<AppState>,
    auth: AuthUser,
    mut multipart: Multipart,
) -> AppResult<Json<response::ApiSuccess<Value>>> {
    let mut uploaded = None;
    while let Some(field) = multipart
        .next_field()
        .await
        .map_err(|err| AppError::BadRequest(err.to_string()))?
    {
        if field.name() != Some("file") && field.name() != Some("avatar") {
            continue;
        }
        let file_name = field.file_name().map(ToOwned::to_owned);
        let content_type = field.content_type().map(ToOwned::to_owned);
        let bytes = field
            .bytes()
            .await
            .map_err(|err| AppError::BadRequest(err.to_string()))?;
        let file =
            storage::validate_avatar_upload(file_name.as_deref(), content_type.as_deref(), bytes)?;
        let path = format!(
            "{}/avatar.{}",
            auth.id,
            storage::avatar_extension(&file.content_type)
        );
        storage::upload_object(
            &state,
            storage::AVATARS_BUCKET,
            &path,
            &file.content_type,
            file.bytes,
            true,
        )
        .await?;
        let public_url = storage::file_url(storage::AVATARS_BUCKET, &path);
        uploaded = Some(public_url);
        break;
    }

    let avatar_url = uploaded.ok_or_else(|| AppError::BadRequest("File is required".to_owned()))?;
    let data = sqlx::query_scalar::<_, SqlJson<Value>>(
        r#"
        insert into user_profiles (id, avatar_url)
        values ($1, $2)
        on conflict (id) do update set avatar_url = excluded.avatar_url, updated_at = now()
        returning to_jsonb(user_profiles) as data
        "#,
    )
    .bind(auth.id)
    .bind(&avatar_url)
    .fetch_one(&state.pool)
    .await?;
    Ok(response::success(json!({
        "avatar_url": avatar_url,
        "profile": data.0,
    })))
}

async fn delete_avatar(
    State(state): State<AppState>,
    auth: AuthUser,
) -> AppResult<Json<response::ApiSuccess<Value>>> {
    let existing = sqlx::query_scalar::<_, Option<String>>(
        "select avatar_url from user_profiles where id = $1",
    )
    .bind(auth.id)
    .fetch_optional(&state.pool)
    .await?
    .flatten();
    if let Some(url) = existing
        && let Some(path) = url.split("/avatars/").nth(1)
    {
        let _ = storage::delete_object(&state, storage::AVATARS_BUCKET, path).await;
    }
    sqlx::query("update user_profiles set avatar_url = null, updated_at = now() where id = $1")
        .bind(auth.id)
        .execute(&state.pool)
        .await?;
    Ok(response::empty_ok())
}

async fn onboarding_status(
    State(state): State<AppState>,
    auth: AuthUser,
) -> AppResult<Json<response::ApiSuccess<Value>>> {
    let (subjects, problems, reviewed) = sqlx::query_as::<_, (i64, i64, i64)>(
        r#"
        select
          (select count(*)::bigint from subjects where user_id = $1),
          (select count(*)::bigint from problems where user_id = $1),
          (select count(*)::bigint from problems where user_id = $1 and last_reviewed_date is not null)
        "#,
    )
    .bind(auth.id)
    .fetch_one(&state.pool)
    .await?;
    let first_subject = sqlx::query_scalar::<_, Option<uuid::Uuid>>(
        "select id from subjects where user_id = $1 order by created_at asc limit 1",
    )
    .bind(auth.id)
    .fetch_optional(&state.pool)
    .await?
    .flatten();
    let first_problem = sqlx::query_as::<_, (uuid::Uuid, uuid::Uuid)>(
        "select id, subject_id from problems where user_id = $1 order by created_at asc limit 1",
    )
    .bind(auth.id)
    .fetch_optional(&state.pool)
    .await?;

    Ok(response::success(json!({
        "hasSubject": subjects > 0,
        "hasProblem": problems > 0,
        "hasReviewed": reviewed > 0,
        "firstSubjectId": first_subject,
        "firstProblemId": first_problem.map(|p| p.0),
        "firstProblemSubjectId": first_problem.map(|p| p.1),
    })))
}

async fn complete_onboarding(
    State(state): State<AppState>,
    auth: AuthUser,
) -> AppResult<Json<response::ApiSuccess<Value>>> {
    sqlx::query(
        r#"
        insert into user_profiles (id, onboarding_completed_at)
        values ($1, now())
        on conflict (id)
        do update set onboarding_completed_at = coalesce(user_profiles.onboarding_completed_at, now()), updated_at = now()
        "#,
    )
    .bind(auth.id)
    .execute(&state.pool)
    .await?;
    Ok(response::success(json!({ "completed": true })))
}
