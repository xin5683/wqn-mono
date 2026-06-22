use chrono::{DateTime, NaiveDate, Utc};
use serde::Serialize;
use sqlx::FromRow;
use uuid::Uuid;

#[derive(Debug, Serialize)]
pub struct StatisticsResponse {
    pub overview: StatisticsOverview,
    pub streaks: StudyStreaks,
    #[serde(rename = "sessionStats")]
    pub session_stats: SessionStatisticsSummary,
    #[serde(rename = "subjectBreakdown")]
    pub subject_breakdown: Vec<SubjectBreakdownRow>,
    #[serde(rename = "weeklyProgress")]
    pub weekly_progress: Vec<WeeklyProgressPoint>,
    #[serde(rename = "activityHeatmap")]
    pub activity_heatmap: Vec<ActivityDay>,
    #[serde(rename = "recentActivity")]
    pub recent_activity: Vec<RecentStudyActivity>,
    pub timezone: String,
}

#[derive(Debug, Serialize)]
pub struct StatisticsOverview {
    pub total_problems: i64,
    pub mastered_count: i64,
    pub needs_review_count: i64,
    pub wrong_count: i64,
    pub mastery_rate: f64,
}

#[derive(Debug, Serialize)]
pub struct StudyStreaks {
    pub current_streak: i64,
    pub longest_streak: i64,
}

#[derive(Debug, Serialize)]
pub struct SessionStatisticsSummary {
    pub total_sessions: i64,
    pub avg_duration_ms: i64,
    pub avg_problems_per_session: f64,
    pub total_review_time_ms: i64,
}

#[derive(Debug, FromRow, Serialize)]
pub struct SubjectBreakdownRow {
    pub subject_id: Uuid,
    pub subject_name: String,
    pub total: i64,
    pub mastered: i64,
    pub needs_review: i64,
    pub wrong: i64,
    pub mastery_pct: f64,
}

#[derive(Debug, FromRow, Serialize)]
pub struct WeeklyProgressPoint {
    pub week_start: NaiveDate,
    pub cumulative_mastered: i64,
}

#[derive(Debug, FromRow, Serialize)]
pub struct ActivityDay {
    pub activity_date: NaiveDate,
    pub activity_count: i64,
}

#[derive(Debug, FromRow, Serialize)]
pub struct RecentStudyActivity {
    pub problem_id: Uuid,
    pub problem_title: String,
    pub subject_name: String,
    pub old_status: Option<String>,
    pub new_status: String,
    pub changed_at: DateTime<Utc>,
}
