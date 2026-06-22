use argon2::{
    Argon2, PasswordHash, PasswordVerifier,
    password_hash::{PasswordHasher, SaltString, rand_core::OsRng},
};
use axum::{
    Json, Router,
    extract::State,
    http::{HeaderValue, header},
    response::{IntoResponse, Response},
    routing::{get, patch, post},
};
use serde::Deserialize;
use serde_json::{Value, json};
use sqlx::{PgPool, types::Json as SqlJson};
use uuid::Uuid;

use crate::{
    auth::{self, AuthUser},
    error::{AppError, AppResult},
    response,
    services::misc,
    state::AppState,
};

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct CredentialsBody {
    email: String,
    password: String,
    timezone: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ChangePasswordBody {
    #[serde(alias = "current_password")]
    current_password: Option<String>,
    #[serde(alias = "new_password")]
    new_password: String,
}

#[derive(Debug, Deserialize)]
struct ForgotPasswordBody {
    email: String,
}

struct AppUserAuthRow {
    id: Uuid,
    email: String,
    password_hash: String,
    token_version: i32,
    disabled: bool,
}

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/auth/sign-up", post(sign_up))
        .route("/auth/login", post(login))
        .route("/auth/logout", post(logout))
        .route("/auth/me", get(me))
        .route("/auth/password", patch(change_password))
        .route("/auth/forgot-password", post(forgot_password))
}

async fn sign_up(
    State(state): State<AppState>,
    Json(body): Json<CredentialsBody>,
) -> AppResult<Response> {
    let email = normalize_email(&body.email)?;
    validate_password(&body.password)?;
    // Self-service registration can be closed by a super-admin via the
    // `user_registration` admin setting. Default is open (see `misc`).
    if !misc::registration_enabled(&state.pool).await? {
        return Err(AppError::Forbidden);
    }
    let password_hash = hash_password(&body.password)?;
    let user_id = Uuid::new_v4();
    let timezone = body.timezone.unwrap_or_else(|| "UTC".to_owned());

    let mut tx = state.pool.begin().await?;
    let insert_result = sqlx::query(
        r#"
        insert into app_users (id, email, password_hash)
        values ($1, $2, $3)
        "#,
    )
    .bind(user_id)
    .bind(&email)
    .bind(&password_hash)
    .execute(&mut *tx)
    .await;

    if let Err(err) = insert_result {
        if is_unique_violation(&err) {
            return Err(AppError::Conflict("Email is already registered".to_owned()));
        }
        return Err(err.into());
    }

    sqlx::query(
        r#"
        insert into user_profiles (id, timezone)
        values ($1, coalesce(nullif($2, ''), 'UTC'))
        on conflict (id) do nothing
        "#,
    )
    .bind(user_id)
    .bind(timezone)
    .execute(&mut *tx)
    .await?;
    tx.commit().await?;

    session_response(&state, user_id, &email, 0, "Signed up")
}

async fn login(
    State(state): State<AppState>,
    Json(body): Json<CredentialsBody>,
) -> AppResult<Response> {
    let email = normalize_email(&body.email)?;
    let user = user_by_email(&state.pool, &email)
        .await?
        .ok_or(AppError::Unauthorized)?;
    if user.disabled || !verify_password(&body.password, &user.password_hash)? {
        return Err(AppError::Unauthorized);
    }

    sqlx::query(
        r#"
        update app_users
        set last_login_at = now()
        where id = $1
        "#,
    )
    .bind(user.id)
    .execute(&state.pool)
    .await?;
    sqlx::query("update user_profiles set last_login_at = now() where id = $1")
        .bind(user.id)
        .execute(&state.pool)
        .await?;

    session_response(
        &state,
        user.id,
        &user.email,
        user.token_version,
        "Logged in",
    )
}

async fn logout(State(state): State<AppState>) -> AppResult<Response> {
    let cookie = auth::expired_session_cookie_value(
        &state.config.auth_cookie_name,
        state.config.auth_cookie_secure,
    );
    let mut response = response::success(json!({ "ok": true })).into_response();
    response.headers_mut().insert(
        header::SET_COOKIE,
        HeaderValue::from_str(&cookie)
            .map_err(|err| AppError::Internal(format!("Invalid cookie value: {err}")))?,
    );
    Ok(response)
}

async fn me(
    State(state): State<AppState>,
    auth: AuthUser,
) -> AppResult<Json<response::ApiSuccess<Value>>> {
    let user = public_user(&state.pool, auth.id).await?;
    Ok(response::success(user))
}

async fn change_password(
    State(state): State<AppState>,
    auth: AuthUser,
    Json(body): Json<ChangePasswordBody>,
) -> AppResult<Response> {
    validate_password(&body.new_password)?;
    let user = user_by_id(&state.pool, auth.id)
        .await?
        .ok_or(AppError::Unauthorized)?;
    if user.disabled {
        return Err(AppError::Unauthorized);
    }
    if let Some(current_password) = body.current_password.as_deref()
        && !verify_password(current_password, &user.password_hash)?
    {
        return Err(AppError::Unauthorized);
    }

    let password_hash = hash_password(&body.new_password)?;
    let token_version = user.token_version + 1;
    sqlx::query(
        r#"
        update app_users
        set password_hash = $2, token_version = token_version + 1, updated_at = now()
        where id = $1
        "#,
    )
    .bind(user.id)
    .bind(password_hash)
    .execute(&state.pool)
    .await?;

    session_response(
        &state,
        user.id,
        &user.email,
        token_version,
        "Password updated",
    )
}

async fn forgot_password(
    Json(body): Json<ForgotPasswordBody>,
) -> AppResult<Json<response::ApiSuccess<Value>>> {
    let _ = normalize_email(&body.email)?;
    Err(AppError::Configuration(
        "Password reset email service is not configured".to_owned(),
    ))
}

fn session_response(
    state: &AppState,
    user_id: Uuid,
    email: &str,
    token_version: i32,
    message: &str,
) -> AppResult<Response> {
    let token = auth::issue_token(
        user_id,
        email,
        token_version,
        &state.config.auth_jwt_secret,
        state.config.auth_session_ttl_seconds,
    )?;
    let cookie = auth::session_cookie_value(
        &state.config.auth_cookie_name,
        &token,
        state.config.auth_session_ttl_seconds,
        state.config.auth_cookie_secure,
    );
    let mut response = response::success(json!({
        "user": {
            "id": user_id,
            "email": email,
        },
        "access_token": token,
        "token_type": "bearer",
        "expires_in": state.config.auth_session_ttl_seconds,
        "message": message,
    }))
    .into_response();
    response.headers_mut().insert(
        header::SET_COOKIE,
        HeaderValue::from_str(&cookie)
            .map_err(|err| AppError::Internal(format!("Invalid cookie value: {err}")))?,
    );
    Ok(response)
}

async fn public_user(pool: &PgPool, user_id: Uuid) -> AppResult<Value> {
    let row = sqlx::query_scalar::<_, SqlJson<Value>>(
        r#"
        select jsonb_build_object(
          'id', au.id,
          'email', au.email,
          'disabled_at', au.disabled_at,
          'last_login_at', au.last_login_at,
          'created_at', au.created_at,
          'updated_at', au.updated_at,
          'role', coalesce(up.user_role, 'user'),
          'is_admin', coalesce(up.user_role, 'user') in ('admin', 'super_admin', 'moderator'),
          'profile', to_jsonb(up)
        ) as data
        from app_users au
        left join user_profiles up on up.id = au.id
        where au.id = $1 and au.disabled_at is null
        "#,
    )
    .bind(user_id)
    .fetch_optional(pool)
    .await?;
    row.map(|SqlJson(v)| v).ok_or(AppError::Unauthorized)
}

async fn user_by_email(pool: &PgPool, email: &str) -> AppResult<Option<AppUserAuthRow>> {
    user_query("lower(email) = lower($1)", email, pool).await
}

async fn user_by_id(pool: &PgPool, user_id: Uuid) -> AppResult<Option<AppUserAuthRow>> {
    let row = sqlx::query_as::<_, (Uuid, String, String, i32, bool)>(
        r#"
        select id, email, password_hash, token_version, disabled_at is not null
        from app_users
        where id = $1
        "#,
    )
    .bind(user_id)
    .fetch_optional(pool)
    .await?;
    Ok(row.map(app_user_row))
}

async fn user_query(
    predicate: &str,
    value: &str,
    pool: &PgPool,
) -> AppResult<Option<AppUserAuthRow>> {
    let sql = format!(
        r#"
        select id, email, password_hash, token_version, disabled_at is not null
        from app_users
        where {predicate}
        "#
    );
    let row = sqlx::query_as::<_, (Uuid, String, String, i32, bool)>(&sql)
        .bind(value)
        .fetch_optional(pool)
        .await?;
    Ok(row.map(app_user_row))
}

fn app_user_row(row: (Uuid, String, String, i32, bool)) -> AppUserAuthRow {
    AppUserAuthRow {
        id: row.0,
        email: row.1,
        password_hash: row.2,
        token_version: row.3,
        disabled: row.4,
    }
}

fn normalize_email(email: &str) -> AppResult<String> {
    let trimmed = email.trim().to_ascii_lowercase();
    if !trimmed.contains('@') || trimmed.len() > 320 {
        return Err(AppError::BadRequest("Invalid email".to_owned()));
    }
    Ok(trimmed)
}

fn validate_password(password: &str) -> AppResult<()> {
    if password.len() < 8 {
        return Err(AppError::BadRequest(
            "Password must be at least 8 characters".to_owned(),
        ));
    }
    Ok(())
}

fn hash_password(password: &str) -> AppResult<String> {
    let salt = SaltString::generate(&mut OsRng);
    Argon2::default()
        .hash_password(password.as_bytes(), &salt)
        .map(|hash| hash.to_string())
        .map_err(|err| AppError::Internal(format!("Password hashing failed: {err}")))
}

fn verify_password(password: &str, hash: &str) -> AppResult<bool> {
    let parsed_hash = PasswordHash::new(hash)
        .map_err(|err| AppError::Internal(format!("Invalid password hash: {err}")))?;
    Ok(Argon2::default()
        .verify_password(password.as_bytes(), &parsed_hash)
        .is_ok())
}

fn is_unique_violation(err: &sqlx::Error) -> bool {
    matches!(err, sqlx::Error::Database(db) if db.code().as_deref() == Some("23505"))
}
