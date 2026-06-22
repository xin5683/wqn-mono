use serde::Deserialize;
use serde_json::{Map, Value, json};
use sqlx::{PgPool, types::Json as SqlJson};
use uuid::Uuid;

use crate::{
    error::{AppResult, validation},
    services::{content_limits, quota},
};

/// Key under which the global limit-defaults override object is stored in the
/// `admin_settings` table. The value is a jsonb object mapping resource-type
/// strings (`ai_extraction`, `ai_categorisation`, `subjects`,
/// `problems_per_subject`, `problem_sets`, `tags_per_subject`,
/// `storage_bytes`) to a non-negative integer. Absent or null fields fall back
/// to the compiled-in [`hardcoded_default`].
pub const LIMIT_DEFAULTS_KEY: &str = "limit_defaults";

/// A snapshot of the globally-configured limit defaults. Each field is
/// `Some(value)` when a super-admin has configured an override for that
/// resource type, `None` otherwise. Cheap to copy — threaded through the
/// effective-limit lookup so the configured default is honoured before the
/// hardcoded baseline.
#[derive(Debug, Clone, Copy, Default, Deserialize)]
pub struct LimitDefaults {
    #[serde(default)]
    pub ai_extraction: Option<i64>,
    #[serde(default)]
    pub ai_categorisation: Option<i64>,
    #[serde(default)]
    pub subjects: Option<i64>,
    #[serde(default)]
    pub problems_per_subject: Option<i64>,
    #[serde(default)]
    pub problem_sets: Option<i64>,
    #[serde(default)]
    pub tags_per_subject: Option<i64>,
    #[serde(default)]
    pub storage_bytes: Option<i64>,
}

impl LimitDefaults {
    /// The configured override for a resource type, if any. Callers fall back
    /// to the hardcoded default when this returns `None`.
    pub fn get(&self, resource_type: &str) -> Option<i64> {
        match resource_type {
            quota::AI_EXTRACTION => self.ai_extraction,
            quota::AI_CATEGORISATION => self.ai_categorisation,
            content_limits::SUBJECTS => self.subjects,
            content_limits::PROBLEMS_PER_SUBJECT => self.problems_per_subject,
            content_limits::PROBLEM_SETS => self.problem_sets,
            content_limits::TAGS_PER_SUBJECT => self.tags_per_subject,
            content_limits::STORAGE_BYTES => self.storage_bytes,
            _ => None,
        }
    }

    /// Serialize as a jsonb object, omitting `None` fields so unsetting a
    /// value (reset) removes it from storage entirely.
    pub fn to_json(self) -> Value {
        let mut map = Map::new();
        let mut insert = |key: &str, v: Option<i64>| {
            if let Some(v) = v {
                map.insert(key.to_owned(), json!(v));
            }
        };
        insert(quota::AI_EXTRACTION, self.ai_extraction);
        insert(quota::AI_CATEGORISATION, self.ai_categorisation);
        insert(content_limits::SUBJECTS, self.subjects);
        insert(
            content_limits::PROBLEMS_PER_SUBJECT,
            self.problems_per_subject,
        );
        insert(content_limits::PROBLEM_SETS, self.problem_sets);
        insert(content_limits::TAGS_PER_SUBJECT, self.tags_per_subject);
        insert(content_limits::STORAGE_BYTES, self.storage_bytes);
        Value::Object(map)
    }
}

/// The compiled-in baseline default for a resource type — the value used when
/// neither a per-user override nor a global configured default exists.
pub fn hardcoded_default(resource_type: &str) -> i64 {
    match resource_type {
        // AI quotas
        quota::AI_CATEGORISATION => 50,
        quota::AI_EXTRACTION => 10,
        // Content limits
        content_limits::STORAGE_BYTES => 50 * 1024 * 1024,
        content_limits::SUBJECTS => 6,
        content_limits::PROBLEMS_PER_SUBJECT => 300,
        content_limits::PROBLEM_SETS => 30,
        content_limits::TAGS_PER_SUBJECT => 50,
        _ => 0,
    }
}

/// Read the global configured limit defaults from `admin_settings`. Returns an
/// empty (all-`None`) snapshot when the row is absent or unparseable, so the
/// hardcoded baseline applies — never errors on a missing/malformed row.
pub async fn configured_defaults(pool: &PgPool) -> AppResult<LimitDefaults> {
    let row: Option<SqlJson<Value>> =
        sqlx::query_scalar("select value from admin_settings where key = $1")
            .bind(LIMIT_DEFAULTS_KEY)
            .fetch_optional(pool)
            .await?;
    Ok(row
        .and_then(|SqlJson(v)| parse_defaults(&v))
        .unwrap_or_default())
}

fn parse_defaults(v: &Value) -> Option<LimitDefaults> {
    serde_json::from_value::<LimitDefaults>(v.clone()).ok()
}

/// All resource types that can have a configured default, paired with their
/// category for display. Used by the admin listing endpoint.
pub fn all_resources() -> &'static [(&'static str, &'static str)] {
    &[
        (quota::AI_EXTRACTION, "daily_quota"),
        (quota::AI_CATEGORISATION, "daily_quota"),
        (content_limits::SUBJECTS, "content_limit"),
        (content_limits::PROBLEMS_PER_SUBJECT, "content_limit"),
        (content_limits::PROBLEM_SETS, "content_limit"),
        (content_limits::TAGS_PER_SUBJECT, "content_limit"),
        (content_limits::STORAGE_BYTES, "content_limit"),
    ]
}

/// Validate that every present default is a non-negative integer. Storage is
/// allowed to be large (e.g. multiple GB) but still must fit in `i64`.
fn validate(defaults: &LimitDefaults) -> AppResult<()> {
    let fields = [
        (quota::AI_EXTRACTION, defaults.ai_extraction),
        (quota::AI_CATEGORISATION, defaults.ai_categorisation),
        (content_limits::SUBJECTS, defaults.subjects),
        (
            content_limits::PROBLEMS_PER_SUBJECT,
            defaults.problems_per_subject,
        ),
        (content_limits::PROBLEM_SETS, defaults.problem_sets),
        (content_limits::TAGS_PER_SUBJECT, defaults.tags_per_subject),
        (content_limits::STORAGE_BYTES, defaults.storage_bytes),
    ];
    let bad: Vec<&str> = fields
        .into_iter()
        .filter_map(|(key, v)| v.is_some_and(|n| n < 0).then_some(key))
        .collect();
    if bad.is_empty() {
        Ok(())
    } else {
        Err(validation(
            "Invalid limit defaults",
            json!({ "fields": bad }),
        ))
    }
}

/// Upsert the configured limit defaults into `admin_settings`. Null fields are
/// dropped from the stored object, which is how a super-admin resets a value
/// back to the hardcoded baseline. Returns the refreshed listing.
pub async fn set_limit_defaults(
    pool: &PgPool,
    admin_id: Uuid,
    defaults: LimitDefaults,
) -> AppResult<Value> {
    validate(&defaults)?;
    let value = defaults.to_json();
    sqlx::query(
        r#"
        insert into admin_settings (key, value, description, updated_by)
        values ($1, $2, 'Global default usage limits and quotas', $3)
        on conflict (key) do update
          set value = excluded.value,
              updated_by = excluded.updated_by,
              updated_at = now()
        "#,
    )
    .bind(LIMIT_DEFAULTS_KEY)
    .bind(SqlJson(value))
    .bind(admin_id)
    .execute(pool)
    .await?;
    list_limit_defaults(pool).await
}

/// Listing for the admin UI: for each resource, its hardcoded baseline, the
/// currently-configured override (if any), and the effective default that
/// enforcement uses.
pub async fn list_limit_defaults(pool: &PgPool) -> AppResult<Value> {
    let defaults = configured_defaults(pool).await?;
    let entries: Vec<Value> = all_resources()
        .iter()
        .map(|&(resource_type, category)| {
            let configured = defaults.get(resource_type);
            let hardcoded = hardcoded_default(resource_type);
            json!({
                "resource_type": resource_type,
                "category": category,
                "hardcoded": hardcoded,
                "configured": configured,
                "effective": configured.unwrap_or(hardcoded),
            })
        })
        .collect();
    Ok(json!({ "defaults": entries }))
}
