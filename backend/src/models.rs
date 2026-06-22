#![allow(dead_code)]

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use utoipa::{IntoParams, ToSchema};
use uuid::Uuid;
use validator::Validate;

#[derive(Debug, Serialize, ToSchema)]
pub struct HealthResponse {
    pub status: String,
    pub timestamp: String,
    pub uptime_seconds: u64,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct ReadyResponse {
    pub status: String,
    pub database: String,
    pub timestamp: String,
}

#[derive(Debug, Deserialize, IntoParams)]
pub struct SubjectQuery {
    pub subject_id: Option<Uuid>,
}

#[derive(Debug, Deserialize, Validate, ToSchema)]
pub struct CreateSubject {
    #[validate(length(min = 1, max = 30))]
    pub name: String,
    pub color: Option<String>,
    pub icon: Option<String>,
}

#[derive(Debug, Deserialize, Validate, ToSchema)]
pub struct UpdateSubject {
    #[validate(length(min = 1, max = 30))]
    pub name: Option<String>,
    pub color: Option<String>,
    pub icon: Option<String>,
}

#[derive(Debug, Deserialize, Validate, ToSchema)]
pub struct CreateTag {
    pub subject_id: Uuid,
    #[validate(length(min = 1, max = 30))]
    pub name: String,
}

#[derive(Debug, Deserialize, Validate, ToSchema)]
pub struct UpdateTag {
    #[validate(length(min = 1, max = 30))]
    pub name: Option<String>,
}

#[derive(Debug, Deserialize, IntoParams)]
pub struct ProblemsQuery {
    pub subject_id: Option<Uuid>,
    pub search_text: Option<String>,
    pub search_title: Option<bool>,
    pub search_content: Option<bool>,
    pub problem_types: Option<String>,
    pub tag_ids: Option<String>,
    pub tag_filter_mode: Option<String>,
    pub statuses: Option<String>,
}

#[derive(Debug, Deserialize, Serialize, ToSchema)]
pub struct Asset {
    pub path: String,
    pub kind: Option<String>,
}

// =====================================================
// Answer Configuration (mirrors frontend AnswerConfigSchema)
// =====================================================
// Typed on the WRITE path (CreateProblem/UpdateProblem/PatchProblem) so the
// backend — not the frontend Zod — is the authoritative guard: structurally
// invalid answer_config (unknown `type`, missing required fields, legacy field
// names like `options`) is rejected at deserialization, before it ever reaches
// the database. The READ path (ProblemRow/ProblemDto) intentionally stays
// `serde_json::Value`: historical rows never break listing, and rows written
// through this typed path always round-trip cleanly.

fn default_true() -> bool {
    true
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct AnswerChoice {
    pub id: String,
    #[serde(default)]
    pub text: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "lowercase")]
pub enum ShortAnswerMode {
    Text,
    Numeric,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct NumericAnswerConfig {
    pub correct_value: f64,
    pub tolerance: f64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub unit: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum AnswerConfig {
    Mcq {
        choices: Vec<AnswerChoice>,
        correct_choice_id: String,
        #[serde(default = "default_true")]
        randomize_choices: bool,
    },
    Short {
        mode: ShortAnswerMode,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        acceptable_answers: Option<Vec<String>>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        numeric_config: Option<NumericAnswerConfig>,
    },
}

#[derive(Debug, Deserialize, Validate, ToSchema)]
pub struct CreateProblem {
    pub id: Option<Uuid>,
    pub subject_id: Uuid,
    #[validate(length(min = 1, max = 50))]
    pub title: String,
    pub content: Option<String>,
    pub problem_type: String,
    pub correct_answer: Option<String>,
    pub answer_config: Option<AnswerConfig>,
    pub auto_mark: Option<bool>,
    pub status: Option<String>,
    pub assets: Option<Vec<Asset>>,
    pub solution_text: Option<String>,
    pub solution_assets: Option<Vec<Asset>>,
    pub last_reviewed_date: Option<DateTime<Utc>>,
    pub tag_ids: Option<Vec<Uuid>>,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct UpdateProblem {
    pub subject_id: Option<Uuid>,
    pub title: Option<String>,
    pub content: Option<String>,
    pub problem_type: Option<String>,
    pub correct_answer: Option<String>,
    pub answer_config: Option<AnswerConfig>,
    pub auto_mark: Option<bool>,
    pub status: Option<String>,
    pub assets: Option<Vec<Asset>>,
    pub solution_text: Option<String>,
    pub solution_assets: Option<Vec<Asset>>,
    pub last_reviewed_date: Option<DateTime<Utc>>,
    pub tag_ids: Option<Vec<Uuid>>,
}

#[derive(Debug, Deserialize, Validate, ToSchema)]
pub struct CreateAttempt {
    pub problem_id: Uuid,
    pub submitted_answer: Value,
    pub is_correct: Option<bool>,
    #[validate(length(max = 1000))]
    pub cause: Option<String>,
    pub is_self_assessed: Option<bool>,
    #[validate(range(min = 1, max = 5))]
    pub confidence: Option<i32>,
    #[validate(length(max = 5000))]
    pub reflection_notes: Option<String>,
    pub selected_status: Option<String>,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct UpdateAttempt {
    pub submitted_answer: Option<Value>,
    pub confidence: Option<i32>,
    pub cause: Option<String>,
    pub reflection_notes: Option<String>,
    pub selected_status: Option<String>,
}

#[derive(Debug, Deserialize, Validate, ToSchema)]
pub struct CreateProblemSet {
    pub subject_id: Uuid,
    #[validate(length(min = 1, max = 50))]
    pub name: String,
    pub description: Option<String>,
    pub sharing_level: Option<String>,
    pub shared_with_emails: Option<Vec<String>>,
    pub problem_ids: Option<Vec<Uuid>>,
    pub is_smart: Option<bool>,
    pub filter_config: Option<Value>,
    pub session_config: Option<Value>,
    pub allow_copying: Option<bool>,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct UpdateProblemSet {
    pub name: Option<String>,
    pub description: Option<String>,
    pub sharing_level: Option<String>,
    pub shared_with_emails: Option<Vec<String>>,
    pub is_smart: Option<bool>,
    pub filter_config: Option<Value>,
    pub session_config: Option<Value>,
    pub allow_copying: Option<bool>,
    pub is_listed: Option<bool>,
    pub discovery_subject: Option<String>,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct ProblemIdsBody {
    pub problem_ids: Vec<Uuid>,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct CopyProblemSetBody {
    pub target_subject_id: Uuid,
    pub name: Option<String>,
    pub copy_tags: Option<bool>,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct ReportProblemSetBody {
    pub reason: String,
    pub details: Option<String>,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct UpdateProfile {
    pub username: Option<String>,
    pub first_name: Option<String>,
    pub last_name: Option<String>,
    pub date_of_birth: Option<String>,
    pub gender: Option<String>,
    pub region: Option<String>,
    pub timezone: Option<String>,
    pub avatar_url: Option<String>,
    pub bio: Option<String>,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct StartSpacedSession {
    pub subject_id: Uuid,
    pub session_size: Option<i64>,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct StartInsightsSession {
    pub subject_id: Uuid,
    pub problem_ids: Vec<Uuid>,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct UpdateSessionProgress {
    pub current_index: Option<i64>,
    pub problem_order: Option<Vec<Uuid>>,
    pub completed_problem_ids: Option<Vec<Uuid>>,
    pub is_completed: Option<bool>,
    pub result: Option<Value>,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct FileDeleteBody {
    pub path: String,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct AdminSettingBody {
    pub key: Option<String>,
    pub value: Value,
    pub description: Option<String>,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct AdminRoleBody {
    pub role: String,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct AdminQuotaBody {
    pub daily_limit: Option<i64>,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct AdminContentLimitBody {
    pub resource_type: String,
    pub limit_value: Option<i64>,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct ExtractProblemBody {
    pub files: Option<Vec<String>>,
    pub text: Option<String>,
    pub subject_id: Option<Uuid>,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct UpdateErrorCategorisation {
    pub broad_category: Option<String>,
    pub granular_tag: Option<String>,
}
