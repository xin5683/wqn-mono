use chrono::{DateTime, Utc};
use serde::Serialize;
use serde_json::Value;
use sqlx::{FromRow, types::Json as SqlJson};
use uuid::Uuid;

use crate::models::Asset;

#[derive(Debug, FromRow)]
pub struct ProblemRow {
    pub id: Uuid,
    pub user_id: Uuid,
    pub subject_id: Uuid,
    pub title: String,
    pub content: Option<String>,
    pub problem_type: String,
    pub correct_answer: Option<String>,
    pub answer_config: Option<SqlJson<Value>>,
    pub auto_mark: bool,
    pub status: String,
    pub assets: SqlJson<Vec<Asset>>,
    pub solution_text: Option<String>,
    pub solution_assets: SqlJson<Vec<Asset>>,
    pub last_reviewed_date: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Serialize)]
pub struct ProblemDto {
    pub id: Uuid,
    pub user_id: Uuid,
    pub subject_id: Uuid,
    pub title: String,
    pub content: Option<String>,
    pub problem_type: String,
    pub correct_answer: Option<String>,
    pub answer_config: Option<Value>,
    pub auto_mark: bool,
    pub status: String,
    pub assets: Vec<Asset>,
    pub solution_text: Option<String>,
    pub solution_assets: Vec<Asset>,
    pub last_reviewed_date: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub tags: Vec<ProblemTagDto>,
}

impl ProblemDto {
    pub fn from_row(row: ProblemRow, tags: Vec<ProblemTagDto>) -> Self {
        Self {
            id: row.id,
            user_id: row.user_id,
            subject_id: row.subject_id,
            title: row.title,
            content: row.content,
            problem_type: row.problem_type,
            correct_answer: row.correct_answer,
            answer_config: row.answer_config.map(|SqlJson(value)| value),
            auto_mark: row.auto_mark,
            status: row.status,
            assets: row.assets.0,
            solution_text: row.solution_text,
            solution_assets: row.solution_assets.0,
            last_reviewed_date: row.last_reviewed_date,
            created_at: row.created_at,
            updated_at: row.updated_at,
            tags,
        }
    }
}

#[derive(Debug, FromRow)]
pub struct ProblemTagRow {
    pub problem_id: Uuid,
    pub id: Uuid,
    pub user_id: Uuid,
    pub subject_id: Uuid,
    pub name: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Serialize)]
pub struct ProblemTagDto {
    pub id: Uuid,
    pub user_id: Uuid,
    pub subject_id: Uuid,
    pub name: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl From<ProblemTagRow> for ProblemTagDto {
    fn from(row: ProblemTagRow) -> Self {
        Self {
            id: row.id,
            user_id: row.user_id,
            subject_id: row.subject_id,
            name: row.name,
            created_at: row.created_at,
            updated_at: row.updated_at,
        }
    }
}
