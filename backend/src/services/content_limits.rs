use serde_json::{Value, json};
use sqlx::PgPool;
use uuid::Uuid;

use crate::{
    error::{AppError, AppResult},
    services::limits_config::{LimitDefaults, configured_defaults, hardcoded_default},
};

pub const SUBJECTS: &str = "subjects";
pub const PROBLEMS_PER_SUBJECT: &str = "problems_per_subject";
pub const PROBLEM_SETS: &str = "problem_sets";
pub const TAGS_PER_SUBJECT: &str = "tags_per_subject";
pub const STORAGE_BYTES: &str = "storage_bytes";

pub fn default_limit(resource_type: &str) -> i64 {
    hardcoded_default(resource_type)
}

pub async fn check_content_limit(
    pool: &PgPool,
    user_id: Uuid,
    resource_type: &str,
    subject_id: Option<Uuid>,
) -> AppResult<Value> {
    let defaults = configured_defaults(pool).await?;
    let limit = effective_limit(pool, user_id, resource_type, defaults).await?;
    let current = match resource_type {
        SUBJECTS => count_rows(pool, "subjects", user_id, None).await?,
        PROBLEMS_PER_SUBJECT => {
            let subject_id = subject_id.ok_or_else(|| {
                AppError::BadRequest("subjectId required for problems_per_subject".to_owned())
            })?;
            count_rows(pool, "problems", user_id, Some(subject_id)).await?
        }
        PROBLEM_SETS => count_rows(pool, "problem_sets", user_id, None).await?,
        TAGS_PER_SUBJECT => {
            let subject_id = subject_id.ok_or_else(|| {
                AppError::BadRequest("subjectId required for tags_per_subject".to_owned())
            })?;
            count_rows(pool, "tags", user_id, Some(subject_id)).await?
        }
        STORAGE_BYTES => get_user_storage_bytes(pool, user_id).await?,
        _ => {
            return Err(AppError::BadRequest(format!(
                "Unknown resource type: {resource_type}"
            )));
        }
    };
    Ok(limit_result(resource_type, current, limit))
}

pub async fn all_content_limits(pool: &PgPool, user_id: Uuid) -> AppResult<Value> {
    let defaults = configured_defaults(pool).await?;
    let subject_count = count_rows(pool, "subjects", user_id, None).await?;
    let problem_set_count = count_rows(pool, "problem_sets", user_id, None).await?;
    let storage_bytes = get_user_storage_bytes(pool, user_id).await?;
    let problems = per_subject_breakdown(pool, user_id, "problems").await?;
    let tags = per_subject_breakdown(pool, user_id, "tags").await?;

    let values = vec![
        limit_result(
            SUBJECTS,
            subject_count,
            effective_limit(pool, user_id, SUBJECTS, defaults).await?,
        ),
        with_breakdown(
            limit_result(
                PROBLEMS_PER_SUBJECT,
                max_current(&problems),
                effective_limit(pool, user_id, PROBLEMS_PER_SUBJECT, defaults).await?,
            ),
            problems,
        ),
        limit_result(
            PROBLEM_SETS,
            problem_set_count,
            effective_limit(pool, user_id, PROBLEM_SETS, defaults).await?,
        ),
        with_breakdown(
            limit_result(
                TAGS_PER_SUBJECT,
                max_current(&tags),
                effective_limit(pool, user_id, TAGS_PER_SUBJECT, defaults).await?,
            ),
            tags,
        ),
        limit_result(
            STORAGE_BYTES,
            storage_bytes,
            effective_limit(pool, user_id, STORAGE_BYTES, defaults).await?,
        ),
    ];

    Ok(Value::Array(values))
}

pub async fn effective_limit(
    pool: &PgPool,
    user_id: Uuid,
    resource_type: &str,
    defaults: LimitDefaults,
) -> AppResult<i64> {
    let override_value = sqlx::query_scalar::<_, Option<i64>>(
        "select limit_value from content_limit_overrides where user_id = $1 and resource_type = $2",
    )
    .bind(user_id)
    .bind(resource_type)
    .fetch_optional(pool)
    .await?
    .flatten();
    Ok(override_value
        .or(defaults.get(resource_type))
        .unwrap_or_else(|| default_limit(resource_type)))
}

pub async fn set_override(
    pool: &PgPool,
    user_id: Uuid,
    resource_type: &str,
    limit_value: Option<i64>,
) -> AppResult<()> {
    if let Some(limit_value) = limit_value {
        if limit_value < 0 {
            return Err(AppError::BadRequest("Invalid limit value".to_owned()));
        }
        sqlx::query(
            r#"
            insert into content_limit_overrides (user_id, resource_type, limit_value)
            values ($1, $2, $3)
            on conflict (user_id, resource_type)
            do update set limit_value = excluded.limit_value, updated_at = now()
            "#,
        )
        .bind(user_id)
        .bind(resource_type)
        .bind(limit_value)
        .execute(pool)
        .await?;
    } else {
        sqlx::query(
            "delete from content_limit_overrides where user_id = $1 and resource_type = $2",
        )
        .bind(user_id)
        .bind(resource_type)
        .execute(pool)
        .await?;
    }
    Ok(())
}

pub async fn get_user_storage_bytes(pool: &PgPool, user_id: Uuid) -> AppResult<i64> {
    let total = sqlx::query_scalar::<_, Option<i64>>(
        r#"
        with files as (
          select jsonb_array_elements(coalesce(assets, '[]'::jsonb)) asset
          from problems where user_id = $1
          union all
          select jsonb_array_elements(coalesce(solution_assets, '[]'::jsonb)) asset
          from problems where user_id = $1
        )
        select coalesce(sum(nullif(asset->>'size', '')::bigint), 0)::bigint from files
        "#,
    )
    .bind(user_id)
    .fetch_one(pool)
    .await?
    .unwrap_or(0);
    Ok(total)
}

async fn count_rows(
    pool: &PgPool,
    table: &str,
    user_id: Uuid,
    subject_id: Option<Uuid>,
) -> AppResult<i64> {
    let sql = if subject_id.is_some() {
        format!("select count(*)::bigint from {table} where user_id = $1 and subject_id = $2")
    } else {
        format!("select count(*)::bigint from {table} where user_id = $1")
    };
    let mut query = sqlx::query_scalar::<_, i64>(&sql).bind(user_id);
    if let Some(subject_id) = subject_id {
        query = query.bind(subject_id);
    }
    Ok(query.fetch_one(pool).await?)
}

async fn per_subject_breakdown(pool: &PgPool, user_id: Uuid, table: &str) -> AppResult<Vec<Value>> {
    let sql = format!(
        r#"
        select jsonb_build_object(
            'subject_id', s.id,
            'subject_name', s.name,
            'current', count(t.id)
        ) as data
        from subjects s
        left join {table} t on t.subject_id = s.id and t.user_id = s.user_id
        where s.user_id = $1
        group by s.id, s.name
        order by s.name asc
        "#
    );
    let rows = sqlx::query_scalar::<_, sqlx::types::Json<Value>>(&sql)
        .bind(user_id)
        .fetch_all(pool)
        .await?;
    Ok(rows.into_iter().map(|sqlx::types::Json(v)| v).collect())
}

fn limit_result(resource_type: &str, current: i64, limit: i64) -> Value {
    json!({
        "allowed": current < limit,
        "current": current,
        "limit": limit,
        "remaining": (limit - current).max(0),
        "resource_type": resource_type,
    })
}

fn with_breakdown(mut value: Value, breakdown: Vec<Value>) -> Value {
    if let Value::Object(map) = &mut value {
        map.insert("per_subject".to_owned(), Value::Array(breakdown));
    }
    value
}

fn max_current(values: &[Value]) -> i64 {
    values
        .iter()
        .filter_map(|v| v.get("current").and_then(Value::as_i64))
        .max()
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn default_limit_returns_known_thresholds() {
        assert_eq!(default_limit(SUBJECTS), 6);
        assert_eq!(default_limit(PROBLEMS_PER_SUBJECT), 300);
        assert_eq!(default_limit(PROBLEM_SETS), 30);
        assert_eq!(default_limit(TAGS_PER_SUBJECT), 50);
        assert_eq!(default_limit(STORAGE_BYTES), 50 * 1024 * 1024);
    }

    #[test]
    fn default_limit_for_unknown_resource_is_zero() {
        assert_eq!(default_limit("nonsense"), 0);
    }

    #[test]
    fn limit_result_under_limit_is_allowed() {
        let result = limit_result(SUBJECTS, 2, 6);
        assert_eq!(result["allowed"], json!(true));
        assert_eq!(result["current"], json!(2));
        assert_eq!(result["limit"], json!(6));
        assert_eq!(result["remaining"], json!(4));
        assert_eq!(result["resource_type"], json!(SUBJECTS));
    }

    #[test]
    fn limit_result_at_limit_is_not_allowed_with_zero_remaining() {
        let result = limit_result(PROBLEM_SETS, 30, 30);
        assert_eq!(result["allowed"], json!(false));
        assert_eq!(result["remaining"], json!(0));
    }

    #[test]
    fn limit_result_over_limit_clamps_remaining_to_zero() {
        let result = limit_result(SUBJECTS, 8, 6);
        // (6 - 8).max(0) == 0
        assert_eq!(result["allowed"], json!(false));
        assert_eq!(result["remaining"], json!(0));
        assert_eq!(result["current"], json!(8));
    }

    #[test]
    fn with_breakdown_inserts_per_subject_array() {
        let result = with_breakdown(
            limit_result(SUBJECTS, 0, 6),
            vec![json!({ "subject_id": "s1", "current": 4 })],
        );
        let breakdown = result.get("per_subject").and_then(Value::as_array).unwrap();
        assert_eq!(breakdown.len(), 1);
        assert_eq!(breakdown[0]["subject_id"], json!("s1"));
    }

    #[test]
    fn max_current_picks_largest_current() {
        let values = vec![
            json!({ "current": 3 }),
            json!({ "current": 7 }),
            json!({ "current": 2 }),
        ];
        assert_eq!(max_current(&values), 7);
    }

    #[test]
    fn max_current_ignores_entries_without_numeric_current() {
        let values = vec![
            json!({ "subject_id": "s1" }),
            json!({ "current": "not-a-number" }),
        ];
        assert_eq!(max_current(&values), 0);
        assert_eq!(max_current(&[]), 0);
    }
}
