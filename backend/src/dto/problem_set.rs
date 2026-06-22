use chrono::{DateTime, Utc};
use serde::Serialize;
use serde_json::Value;
use sqlx::{FromRow, types::Json as SqlJson};
use uuid::Uuid;

use crate::dto::problem::ProblemDto;

#[derive(Debug, FromRow)]
pub struct ProblemSetRow {
    pub id: Uuid,
    pub user_id: Uuid,
    pub subject_id: Uuid,
    pub name: String,
    pub description: Option<String>,
    pub sharing_level: String,
    pub is_smart: bool,
    pub filter_config: Option<SqlJson<Value>>,
    pub session_config: Option<SqlJson<Value>>,
    pub allow_copying: bool,
    pub is_listed: bool,
    pub discovery_subject: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub subject_name: String,
    pub problem_count: i64,
}

#[derive(Debug, Serialize)]
pub struct ProblemSetDto {
    pub id: Uuid,
    pub user_id: Uuid,
    pub subject_id: Uuid,
    pub name: String,
    pub description: Option<String>,
    pub sharing_level: String,
    pub is_smart: bool,
    pub filter_config: Option<Value>,
    pub session_config: Option<Value>,
    pub allow_copying: bool,
    pub is_listed: bool,
    pub discovery_subject: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub subject_name: String,
    pub problem_count: i64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub shared_with_emails: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub problem_set_shares: Option<Vec<ProblemSetShareDto>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub owner_profile: Option<ProblemSetOwnerProfileDto>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub problems: Option<Vec<ProblemSetProblemDto>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub favorited_at: Option<DateTime<Utc>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub favourited_at: Option<DateTime<Utc>>,
}

impl ProblemSetDto {
    pub fn from_row(row: ProblemSetRow) -> Self {
        Self {
            id: row.id,
            user_id: row.user_id,
            subject_id: row.subject_id,
            name: row.name,
            description: row.description,
            sharing_level: row.sharing_level,
            is_smart: row.is_smart,
            filter_config: row.filter_config.map(|SqlJson(value)| value),
            session_config: row.session_config.map(|SqlJson(value)| value),
            allow_copying: row.allow_copying,
            is_listed: row.is_listed,
            discovery_subject: row.discovery_subject,
            created_at: row.created_at,
            updated_at: row.updated_at,
            subject_name: row.subject_name,
            problem_count: row.problem_count,
            shared_with_emails: None,
            problem_set_shares: None,
            owner_profile: None,
            problems: None,
            favorited_at: None,
            favourited_at: None,
        }
    }
}

#[derive(Debug, FromRow, Serialize)]
pub struct ProblemSetShareDto {
    pub id: Uuid,
    pub shared_with_email: String,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, FromRow, Serialize)]
pub struct ProblemSetOwnerProfileDto {
    pub id: Uuid,
    pub username: Option<String>,
    pub first_name: Option<String>,
    pub last_name: Option<String>,
    pub avatar_url: Option<String>,
    pub bio: Option<String>,
    pub gender: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct ProblemSetProblemDto {
    pub problem_id: Uuid,
    pub added_at: Option<DateTime<Utc>>,
    pub problems: ProblemDto,
}
