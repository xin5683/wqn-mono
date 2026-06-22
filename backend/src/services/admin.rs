use serde::Deserialize;
use serde_json::{Value, json};
use sqlx::{PgPool, Postgres, QueryBuilder, types::Json as SqlJson};
use uuid::Uuid;

use crate::{
    dto::{
        admin::{AdminUserDto, AdminUsersResponse},
        patch::PatchField,
    },
    error::{AppError, AppResult},
    services::{accounts, content_limits, quota},
};

#[derive(Debug, Deserialize)]
pub struct AdminQuery {
    page: Option<i64>,
    limit: Option<i64>,
    search: Option<String>,
    role: Option<String>,
    sort: Option<String>,
    dir: Option<String>,
    user_id: Option<Uuid>,
    action: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct PatchAdminUser {
    #[serde(default)]
    username: PatchField<String>,
    #[serde(default)]
    user_role: PatchField<String>,
    #[serde(default)]
    is_active: PatchField<bool>,
}

#[derive(Debug, Deserialize)]
pub struct PatchAdminRole {
    #[serde(default)]
    role: PatchField<String>,
    #[serde(default)]
    user_role: PatchField<String>,
}

#[derive(Debug, Deserialize)]
pub struct PatchAdminQuota {
    #[serde(default)]
    resource_type: PatchField<String>,
    #[serde(default)]
    daily_limit: PatchField<i64>,
}

#[derive(Debug, Deserialize)]
pub struct PatchAdminContentLimit {
    #[serde(default)]
    resource_type: PatchField<String>,
    #[serde(default)]
    limit_value: PatchField<i64>,
}

#[derive(Debug, Deserialize)]
pub struct PatchAdminSetting {
    #[serde(default)]
    value: PatchField<Value>,
    #[serde(default)]
    description: PatchField<String>,
}

pub async fn list_users(pool: &PgPool, query: AdminQuery) -> AppResult<AdminUsersResponse> {
    let page = query.page.unwrap_or(1).max(1);
    let limit = query.limit.unwrap_or(20).clamp(1, 100);
    let offset = (page - 1) * limit;

    let search = query
        .search
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty());
    let role = query
        .role
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty());

    let mut qb = QueryBuilder::<Postgres>::new("select ");
    qb.push(accounts::ADMIN_USER_SELECT)
        .push(" from app_users au left join user_profiles up on up.id = au.id where true");
    push_user_filters(&mut qb, search, role);
    let sort = match query.sort.as_deref() {
        Some("username") => "up.username",
        Some("user_role") => "coalesce(up.user_role, 'user')",
        Some("is_active") => "au.disabled_at is null",
        Some("updated_at") => {
            "coalesce(greatest(up.updated_at, au.updated_at), up.updated_at, au.updated_at)"
        }
        _ => "coalesce(up.created_at, au.created_at)",
    };
    let dir = if query.dir.as_deref() == Some("asc") {
        "asc"
    } else {
        "desc"
    };
    qb.push(format!(
        " order by {sort} {dir} nulls last, au.id asc limit "
    ))
    .push_bind(limit)
    .push(" offset ")
    .push_bind(offset);
    let users = qb.build_query_as::<AdminUserDto>().fetch_all(pool).await?;
    let mut count_qb = QueryBuilder::<Postgres>::new(
        "select count(*)::bigint from app_users au left join user_profiles up on up.id = au.id where true",
    );
    push_user_filters(&mut count_qb, search, role);
    let total_count = count_qb.build_query_scalar::<i64>().fetch_one(pool).await?;
    Ok(AdminUsersResponse {
        users,
        total_count,
        page,
        limit,
    })
}

pub async fn update_user(
    pool: &PgPool,
    actor_id: Uuid,
    id: Uuid,
    body: PatchAdminUser,
) -> AppResult<AdminUserDto> {
    let current_role = admin_role_for_target(pool, id).await?;
    if accounts::is_super_admin_role(&current_role) {
        return Err(accounts::not_found());
    }

    let (username_present, username) = nullable_string_patch(body.username);
    let (role_present, role) = optional_role_patch(body.user_role)?;
    if role.is_some() && id == actor_id {
        return Err(AppError::BadRequest(
            "Cannot change your own role".to_owned(),
        ));
    }
    let (active_present, active) = required_patch(body.is_active, "is_active")?;
    if let Some(active) = active
        && id == actor_id
        && !active
    {
        return Err(AppError::BadRequest(
            "Cannot deactivate your own account".to_owned(),
        ));
    }

    if username_present || role_present {
        sqlx::query("insert into user_profiles (id) values ($1) on conflict (id) do nothing")
            .bind(id)
            .execute(pool)
            .await?;
        sqlx::query(
            r#"
            update user_profiles
            set
              username = case when $2 then $3 else username end,
              user_role = case when $4 then $5 else user_role end,
              updated_at = now()
            where id = $1
            "#,
        )
        .bind(id)
        .bind(username_present)
        .bind(username)
        .bind(role_present)
        .bind(role)
        .execute(pool)
        .await?;
    }

    if active_present {
        let active = active.expect("active is present when active_present is true");
        accounts::set_account_active(pool, id, active).await?;
    }

    accounts::admin_user(pool, id)
        .await?
        .ok_or_else(accounts::not_found)
}

pub async fn delete_user(pool: &PgPool, actor_id: Uuid, id: Uuid) -> AppResult<()> {
    if id == actor_id {
        return Err(AppError::BadRequest(
            "Cannot delete your own account".to_owned(),
        ));
    }
    if !accounts::delete_account(pool, id).await? {
        return Err(accounts::not_found());
    }
    Ok(())
}

pub async fn update_role(
    pool: &PgPool,
    actor_id: Uuid,
    id: Uuid,
    body: PatchAdminRole,
) -> AppResult<AdminUserDto> {
    if id == actor_id {
        return Err(AppError::BadRequest(
            "Cannot change your own role".to_owned(),
        ));
    }
    let role = role_patch_alias(body.role, body.user_role)?;
    let current_role = admin_role_for_target(pool, id).await?;
    if accounts::is_super_admin_role(&current_role) {
        return Err(accounts::not_found());
    }
    sqlx::query("insert into user_profiles (id) values ($1) on conflict (id) do nothing")
        .bind(id)
        .execute(pool)
        .await?;
    let affected = sqlx::query(
        r#"
        update user_profiles
        set user_role = $2, updated_at = now()
        where id = $1
        "#,
    )
    .bind(id)
    .bind(&role)
    .execute(pool)
    .await?
    .rows_affected();
    if affected == 0 {
        return Err(accounts::not_found());
    }
    accounts::admin_user(pool, id)
        .await?
        .ok_or_else(accounts::not_found)
}

pub async fn toggle_active(pool: &PgPool, actor_id: Uuid, id: Uuid) -> AppResult<AdminUserDto> {
    if id == actor_id {
        return Err(AppError::BadRequest(
            "Cannot deactivate your own account".to_owned(),
        ));
    }
    accounts::toggle_account_active(pool, id)
        .await?
        .ok_or_else(accounts::not_found)
}

pub async fn user_content_statistics(pool: &PgPool, id: Uuid) -> AppResult<Value> {
    let stats = sqlx::query_as::<_, (i64, i64, i64, i64)>(
        r#"
        select
          (select count(*)::bigint from subjects where user_id = $1),
          (select count(*)::bigint from problems where user_id = $1),
          (select count(*)::bigint from problem_sets where user_id = $1),
          (select count(*)::bigint from attempts where user_id = $1)
        "#,
    )
    .bind(id)
    .fetch_one(pool)
    .await?;
    Ok(json!({
        "subjects": stats.0,
        "problems": stats.1,
        "problem_sets": stats.2,
        "attempts": stats.3,
    }))
}

pub async fn user_storage_usage(pool: &PgPool, id: Uuid) -> AppResult<Value> {
    let usage = sqlx::query_as::<_, (i64, i64)>(
        r#"
        with files as (
          select jsonb_array_elements(coalesce(assets, '[]'::jsonb)) asset
          from problems where user_id = $1
          union all
          select jsonb_array_elements(coalesce(solution_assets, '[]'::jsonb)) asset
          from problems where user_id = $1
        )
        select
          coalesce(sum(nullif(asset->>'size', '')::bigint), 0)::bigint,
          count(*)::bigint
        from files
        "#,
    )
    .bind(id)
    .fetch_one(pool)
    .await?;
    Ok(json!({
        "totalBytes": usage.0,
        "fileCount": usage.1,
    }))
}

pub async fn user_quota(pool: &PgPool, id: Uuid) -> AppResult<Value> {
    Ok(json!({
        "ai_extraction": quota::quota_usage(pool, id, quota::AI_EXTRACTION).await?,
        "ai_categorisation": quota::quota_usage(pool, id, quota::AI_CATEGORISATION).await?,
    }))
}

pub async fn set_user_quota(pool: &PgPool, id: Uuid, body: PatchAdminQuota) -> AppResult<Value> {
    let resource_type = resource_type_patch(body.resource_type, Some(quota::AI_EXTRACTION))?;
    if let Some(daily_limit) = nullable_i64_patch(body.daily_limit)? {
        quota::set_quota_override(pool, id, &resource_type, daily_limit).await?;
    }
    user_quota(pool, id).await
}

pub async fn user_content_limits(pool: &PgPool, id: Uuid) -> AppResult<Value> {
    content_limits::all_content_limits(pool, id).await
}

pub async fn set_user_content_limit(
    pool: &PgPool,
    id: Uuid,
    body: PatchAdminContentLimit,
) -> AppResult<Value> {
    let resource_type = resource_type_patch(body.resource_type, None)?;
    if let Some(limit_value) = nullable_i64_patch(body.limit_value)? {
        content_limits::set_override(pool, id, &resource_type, limit_value).await?;
    }
    user_content_limits(pool, id).await
}

pub async fn list_settings(pool: &PgPool) -> AppResult<Value> {
    let settings = sqlx::query_scalar::<_, SqlJson<Value>>(
        "select to_jsonb(admin_settings) as data from admin_settings order by key asc",
    )
    .fetch_all(pool)
    .await?
    .into_iter()
    .map(|SqlJson(v)| v)
    .collect::<Vec<_>>();
    Ok(json!({ "settings": settings }))
}

pub async fn update_setting(
    pool: &PgPool,
    admin_id: Uuid,
    key: String,
    body: PatchAdminSetting,
) -> AppResult<Value> {
    let (value_present, value) = setting_value_patch(body.value);
    let (description_present, description) = nullable_string_patch(body.description);
    let row = sqlx::query_scalar::<_, SqlJson<Value>>(
        r#"
        insert into admin_settings (key, value, description, updated_by)
        values (
          $1,
          case when $2 then $3 else '{}'::jsonb end,
          case when $4 then $5 else null end,
          $6
        )
        on conflict (key) do update
        set
          value = case when $2 then excluded.value else admin_settings.value end,
          description = case when $4 then excluded.description else admin_settings.description end,
          updated_by = excluded.updated_by,
          updated_at = now()
        returning to_jsonb(admin_settings) as data
        "#,
    )
    .bind(key)
    .bind(value_present)
    .bind(value.map(SqlJson))
    .bind(description_present)
    .bind(description)
    .bind(admin_id)
    .fetch_one(pool)
    .await?;
    Ok(json!({ "setting": row.0 }))
}

pub async fn statistics(pool: &PgPool) -> AppResult<Value> {
    let stats = sqlx::query_as::<_, (i64, i64, i64, i64, i64)>(
        r#"
        select
          (select count(*)::bigint from app_users),
          (select count(*)::bigint from app_users where disabled_at is null),
          (
            select count(*)::bigint
            from app_users au
            left join user_profiles up on up.id = au.id
            where coalesce(up.user_role, 'user') in ('admin','super_admin')
          ),
          (select count(*)::bigint from app_users where created_at >= current_date),
          (select count(*)::bigint from app_users where created_at >= date_trunc('week', now()))
        "#,
    )
    .fetch_one(pool)
    .await?;
    let content = sqlx::query_as::<_, (i64, i64, i64)>(
        r#"
        select
          (select count(*)::bigint from subjects),
          (select count(*)::bigint from problems),
          (select count(*)::bigint from problem_sets)
        "#,
    )
    .fetch_one(pool)
    .await?;
    Ok(json!({
        "statistics": {
            "total_users": stats.0,
            "active_users": stats.1,
            "admin_users": stats.2,
            "new_users_today": stats.3,
            "new_users_this_week": stats.4,
        },
        "contentStats": {
            "subjects": content.0,
            "problems": content.1,
            "problem_sets": content.2,
        },
        "storageStats": {
            "total_bytes": 0,
        }
    }))
}

pub async fn activity(pool: &PgPool, query: AdminQuery) -> AppResult<Value> {
    let page = query.page.unwrap_or(1).max(1);
    let limit = query.limit.unwrap_or(20).clamp(1, 100);
    let offset = (page - 1) * limit;
    let mut qb = QueryBuilder::<Postgres>::new(
        "select to_jsonb(user_activity_log) as data from user_activity_log where true",
    );
    if let Some(user_id) = query.user_id {
        qb.push(" and user_id = ").push_bind(user_id);
    }
    if let Some(action) = query.action.as_deref().filter(|s| !s.is_empty()) {
        qb.push(" and action = ").push_bind(action);
    }
    qb.push(" order by created_at desc limit ")
        .push_bind(limit)
        .push(" offset ")
        .push_bind(offset);
    let activities = qb
        .build_query_scalar::<SqlJson<Value>>()
        .fetch_all(pool)
        .await?
        .into_iter()
        .map(|SqlJson(v)| v)
        .collect::<Vec<_>>();
    let total_count =
        sqlx::query_scalar::<_, i64>("select count(*)::bigint from user_activity_log")
            .fetch_one(pool)
            .await?;
    Ok(json!({
        "activities": activities,
        "total_count": total_count,
        "page": page,
        "limit": limit,
    }))
}

fn push_user_filters<'args>(
    qb: &mut QueryBuilder<'args, Postgres>,
    search: Option<&str>,
    role: Option<&str>,
) {
    if let Some(search) = search {
        let pattern = format!("%{search}%");
        qb.push(" and (au.email ilike ")
            .push_bind(pattern.clone())
            .push(" or up.username ilike ")
            .push_bind(pattern.clone())
            .push(" or up.first_name ilike ")
            .push_bind(pattern.clone())
            .push(" or up.last_name ilike ")
            .push_bind(pattern)
            .push(")");
    }
    if let Some(role) = role {
        qb.push(" and coalesce(up.user_role, 'user') = ")
            .push_bind(role.to_owned());
    }
}

async fn admin_role_for_target(pool: &PgPool, user_id: Uuid) -> AppResult<String> {
    sqlx::query_scalar::<_, Option<String>>(
        r#"
        select coalesce(up.user_role, 'user')
        from app_users au
        left join user_profiles up on up.id = au.id
        where au.id = $1
        "#,
    )
    .bind(user_id)
    .fetch_optional(pool)
    .await?
    .flatten()
    .ok_or_else(accounts::not_found)
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

fn required_patch<T>(field: PatchField<T>, field_name: &str) -> AppResult<(bool, Option<T>)> {
    match field {
        PatchField::Missing => Ok((false, None)),
        PatchField::Null => Err(AppError::BadRequest(format!("{field_name} cannot be null"))),
        PatchField::Value(value) => Ok((true, Some(value))),
    }
}

fn optional_role_patch(field: PatchField<String>) -> AppResult<(bool, Option<String>)> {
    match field {
        PatchField::Missing => Ok((false, None)),
        PatchField::Null => Err(AppError::BadRequest("Invalid role specified".to_owned())),
        PatchField::Value(value) => {
            let value = normalise_role(value)?;
            Ok((true, Some(value)))
        }
    }
}

fn role_patch_alias(role: PatchField<String>, user_role: PatchField<String>) -> AppResult<String> {
    let selected = match role {
        PatchField::Missing => user_role,
        value => value,
    };
    match selected {
        PatchField::Value(value) => normalise_role(value),
        PatchField::Missing | PatchField::Null => {
            Err(AppError::BadRequest("Invalid role specified".to_owned()))
        }
    }
}

fn normalise_role(value: String) -> AppResult<String> {
    let value = value.trim().to_owned();
    if !accounts::is_valid_role(&value) {
        return Err(AppError::BadRequest("Invalid role specified".to_owned()));
    }
    Ok(value)
}

fn resource_type_patch(
    field: PatchField<String>,
    default: Option<&'static str>,
) -> AppResult<String> {
    match field {
        PatchField::Missing => default
            .map(ToOwned::to_owned)
            .ok_or_else(|| AppError::BadRequest("Invalid resource type".to_owned())),
        PatchField::Null => Err(AppError::BadRequest("Invalid resource type".to_owned())),
        PatchField::Value(value) => {
            let value = value.trim().to_owned();
            if value.is_empty() {
                return Err(AppError::BadRequest("Invalid resource type".to_owned()));
            }
            Ok(value)
        }
    }
}

fn nullable_i64_patch(field: PatchField<i64>) -> AppResult<Option<Option<i64>>> {
    match field {
        PatchField::Missing => Ok(None),
        PatchField::Null => Ok(Some(None)),
        PatchField::Value(value) => Ok(Some(Some(value))),
    }
}

fn setting_value_patch(field: PatchField<Value>) -> (bool, Option<Value>) {
    match field {
        PatchField::Missing => (false, None),
        PatchField::Null => (true, Some(Value::Null)),
        PatchField::Value(value) => (true, Some(value)),
    }
}
