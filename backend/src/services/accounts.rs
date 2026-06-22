use serde::Serialize;
use sqlx::PgPool;
use uuid::Uuid;

use crate::{
    dto::admin::AdminUserDto,
    error::{AppError, AppResult},
    response,
};

pub const ADMIN_USER_SELECT: &str = r#"
  au.id as id,
  au.email as email,
  up.username as username,
  up.first_name as first_name,
  up.last_name as last_name,
  up.date_of_birth as date_of_birth,
  up.gender as gender,
  up.region as region,
  coalesce(up.timezone, 'UTC') as timezone,
  up.avatar_url as avatar_url,
  up.bio as bio,
  coalesce(up.user_role, 'user') as user_role,
  (au.disabled_at is null) as is_active,
  au.disabled_at as disabled_at,
  coalesce(au.last_login_at, up.last_login_at) as last_login_at,
  coalesce(up.created_at, au.created_at) as created_at,
  coalesce(greatest(up.updated_at, au.updated_at), up.updated_at, au.updated_at) as updated_at
"#;

pub fn is_admin_role(role: &str) -> bool {
    matches!(role, "admin" | "super_admin" | "moderator")
}

pub fn is_super_admin_role(role: &str) -> bool {
    role == "super_admin"
}

pub fn is_valid_role(role: &str) -> bool {
    matches!(role, "user" | "moderator" | "admin" | "super_admin")
}

pub async fn role_for_user(pool: &PgPool, user_id: Uuid) -> AppResult<String> {
    let role = sqlx::query_scalar::<_, Option<String>>(
        r#"
        select coalesce(up.user_role, 'user')
        from app_users au
        left join user_profiles up on up.id = au.id
        where au.id = $1 and au.disabled_at is null
        "#,
    )
    .bind(user_id)
    .fetch_optional(pool)
    .await?
    .flatten()
    .unwrap_or_else(|| "user".to_owned());
    Ok(role)
}

pub async fn admin_user(pool: &PgPool, user_id: Uuid) -> AppResult<Option<AdminUserDto>> {
    let sql = format!(
        r#"
        select {ADMIN_USER_SELECT}
        from app_users au
        left join user_profiles up on up.id = au.id
        where au.id = $1
        "#
    );
    let row = sqlx::query_as::<_, AdminUserDto>(&sql)
        .bind(user_id)
        .fetch_optional(pool)
        .await?;
    Ok(row)
}

pub async fn set_account_active(
    pool: &PgPool,
    user_id: Uuid,
    active: bool,
) -> AppResult<Option<AdminUserDto>> {
    let updated = sqlx::query_as::<_, (Uuid, bool)>(
        r#"
        with target as (
          select au.id, au.disabled_at, coalesce(up.user_role, 'user') as user_role
          from app_users au
          left join user_profiles up on up.id = au.id
          where au.id = $1
        )
        update app_users au
        set
          disabled_at = case when $2 then null else coalesce(au.disabled_at, now()) end,
          token_version = case
            when (au.disabled_at is null) <> $2 then au.token_version + 1
            else au.token_version
          end,
          updated_at = case
            when (au.disabled_at is null) <> $2 then now()
            else au.updated_at
          end
        from target
        where au.id = target.id and target.user_role <> 'super_admin'
        returning au.id, au.disabled_at is null
        "#,
    )
    .bind(user_id)
    .bind(active)
    .fetch_optional(pool)
    .await?;

    let Some((updated_id, is_active)) = updated else {
        return Ok(None);
    };

    sqlx::query("update user_profiles set is_active = $2, updated_at = now() where id = $1")
        .bind(updated_id)
        .bind(is_active)
        .execute(pool)
        .await?;

    admin_user(pool, user_id).await
}

pub async fn toggle_account_active(
    pool: &PgPool,
    user_id: Uuid,
) -> AppResult<Option<AdminUserDto>> {
    let updated = sqlx::query_as::<_, (Uuid, bool)>(
        r#"
        with target as (
          select au.id, coalesce(up.user_role, 'user') as user_role
          from app_users au
          left join user_profiles up on up.id = au.id
          where au.id = $1
        )
        update app_users au
        set
          disabled_at = case when au.disabled_at is null then now() else null end,
          token_version = au.token_version + 1,
          updated_at = now()
        from target
        where au.id = target.id and target.user_role <> 'super_admin'
        returning au.id, au.disabled_at is null
        "#,
    )
    .bind(user_id)
    .fetch_optional(pool)
    .await?;

    let Some((updated_id, is_active)) = updated else {
        return Ok(None);
    };

    sqlx::query("update user_profiles set is_active = $2, updated_at = now() where id = $1")
        .bind(updated_id)
        .bind(is_active)
        .execute(pool)
        .await?;

    admin_user(pool, user_id).await
}

pub async fn delete_account(pool: &PgPool, user_id: Uuid) -> AppResult<bool> {
    let deleted = sqlx::query_scalar::<_, i64>(
        r#"
        with target as (
          select au.id, coalesce(up.user_role, 'user') as user_role
          from app_users au
          left join user_profiles up on up.id = au.id
          where au.id = $1
        ),
        deleted_profile as (
          delete from user_profiles up
          using target
          where up.id = target.id and target.user_role <> 'super_admin'
          returning up.id
        ),
        deleted_account as (
          delete from app_users au
          using target
          where au.id = target.id and target.user_role <> 'super_admin'
          returning au.id
        )
        select count(*)::bigint from deleted_account
        "#,
    )
    .bind(user_id)
    .fetch_one(pool)
    .await?;
    Ok(deleted > 0)
}

pub fn not_found() -> AppError {
    AppError::NotFound("User not found".to_owned())
}

pub fn success_or_not_found<T: Serialize>(
    value: Option<T>,
) -> AppResult<axum::Json<response::ApiSuccess<T>>> {
    value.map(response::success).ok_or_else(not_found)
}

#[cfg(test)]
mod tests {
    use super::{is_admin_role, is_super_admin_role, is_valid_role};

    #[test]
    fn role_helpers_match_admin_policy() {
        assert!(is_admin_role("moderator"));
        assert!(is_admin_role("admin"));
        assert!(is_admin_role("super_admin"));
        assert!(!is_admin_role("user"));

        assert!(is_super_admin_role("super_admin"));
        assert!(!is_super_admin_role("admin"));

        assert!(is_valid_role("user"));
        assert!(is_valid_role("moderator"));
        assert!(is_valid_role("admin"));
        assert!(is_valid_role("super_admin"));
        assert!(!is_valid_role("owner"));
    }
}
