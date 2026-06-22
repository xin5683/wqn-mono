use chrono::Utc;
use serde_json::{Value, json};
use sqlx::PgPool;
use uuid::Uuid;

use crate::{
    error::{AppError, AppResult},
    services::limits_config::{LimitDefaults, configured_defaults, hardcoded_default},
};

pub const AI_EXTRACTION: &str = "ai_extraction";
pub const AI_CATEGORISATION: &str = "ai_categorisation";

pub fn default_limit(resource_type: &str) -> i64 {
    hardcoded_default(resource_type)
}

pub async fn quota_usage(pool: &PgPool, user_id: Uuid, resource_type: &str) -> AppResult<Value> {
    let defaults = configured_defaults(pool).await?;
    let daily_limit = effective_daily_limit(pool, user_id, resource_type, defaults).await?;
    let today = Utc::now().date_naive();
    let current = sqlx::query_scalar::<_, Option<i64>>(
        r#"
        select usage_count from usage_quotas
        where user_id = $1 and resource_type = $2 and period_start = $3
        "#,
    )
    .bind(user_id)
    .bind(resource_type)
    .bind(today)
    .fetch_optional(pool)
    .await?
    .flatten()
    .unwrap_or(0);

    Ok(quota_result(current, daily_limit))
}

pub async fn check_and_increment_quota(
    pool: &PgPool,
    user_id: Uuid,
    resource_type: &str,
) -> AppResult<Value> {
    let defaults = configured_defaults(pool).await?;
    let mut tx = pool.begin().await?;
    let daily_limit = effective_daily_limit_tx(&mut tx, user_id, resource_type, defaults).await?;
    let today = Utc::now().date_naive();

    let current = sqlx::query_scalar::<_, i64>(
        r#"
        insert into usage_quotas (user_id, resource_type, period_start, usage_count)
        values ($1, $2, $3, 0)
        on conflict (user_id, resource_type, period_start)
        do update set updated_at = now()
        returning usage_count
        "#,
    )
    .bind(user_id)
    .bind(resource_type)
    .bind(today)
    .fetch_one(&mut *tx)
    .await?;

    if current >= daily_limit {
        tx.commit().await?;
        return Ok(quota_result(current, daily_limit));
    }

    let updated = sqlx::query_scalar::<_, i64>(
        r#"
        update usage_quotas
        set usage_count = usage_count + 1, updated_at = now()
        where user_id = $1 and resource_type = $2 and period_start = $3
        returning usage_count
        "#,
    )
    .bind(user_id)
    .bind(resource_type)
    .bind(today)
    .fetch_one(&mut *tx)
    .await?;
    tx.commit().await?;

    Ok(quota_result(updated, daily_limit))
}

pub async fn set_quota_override(
    pool: &PgPool,
    user_id: Uuid,
    resource_type: &str,
    daily_limit: Option<i64>,
) -> AppResult<()> {
    if let Some(daily_limit) = daily_limit {
        if daily_limit < 0 {
            return Err(AppError::BadRequest("Invalid daily limit".to_owned()));
        }
        sqlx::query(
            r#"
            insert into user_quota_overrides (user_id, resource_type, daily_limit)
            values ($1, $2, $3)
            on conflict (user_id, resource_type)
            do update set daily_limit = excluded.daily_limit, updated_at = now()
            "#,
        )
        .bind(user_id)
        .bind(resource_type)
        .bind(daily_limit)
        .execute(pool)
        .await?;
    } else {
        sqlx::query("delete from user_quota_overrides where user_id = $1 and resource_type = $2")
            .bind(user_id)
            .bind(resource_type)
            .execute(pool)
            .await?;
    }
    Ok(())
}

async fn effective_daily_limit(
    pool: &PgPool,
    user_id: Uuid,
    resource_type: &str,
    defaults: LimitDefaults,
) -> AppResult<i64> {
    let value = sqlx::query_scalar::<_, Option<i64>>(
        "select daily_limit from user_quota_overrides where user_id = $1 and resource_type = $2",
    )
    .bind(user_id)
    .bind(resource_type)
    .fetch_optional(pool)
    .await?
    .flatten();
    Ok(value
        .or(defaults.get(resource_type))
        .unwrap_or_else(|| default_limit(resource_type)))
}

async fn effective_daily_limit_tx(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    user_id: Uuid,
    resource_type: &str,
    defaults: LimitDefaults,
) -> AppResult<i64> {
    let value = sqlx::query_scalar::<_, Option<i64>>(
        "select daily_limit from user_quota_overrides where user_id = $1 and resource_type = $2",
    )
    .bind(user_id)
    .bind(resource_type)
    .fetch_optional(&mut **tx)
    .await?
    .flatten();
    Ok(value
        .or(defaults.get(resource_type))
        .unwrap_or_else(|| default_limit(resource_type)))
}

fn quota_result(current: i64, daily_limit: i64) -> Value {
    json!({
        "allowed": current < daily_limit,
        "current_usage": current,
        "daily_limit": daily_limit,
        "remaining": (daily_limit - current).max(0),
    })
}
