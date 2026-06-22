use chrono::{Duration, Utc};
use serde_json::Value;
use sqlx::{Postgres, QueryBuilder};
use uuid::Uuid;

use crate::error::{AppError, AppResult};

#[derive(Debug, Clone, Default)]
pub struct ProblemFilter {
    pub subject_id: Option<Uuid>,
    pub search_text: Option<String>,
    pub search_title: Option<bool>,
    pub search_content: Option<bool>,
    pub problem_types: Vec<String>,
    pub tag_ids: Vec<Uuid>,
    pub tag_filter_mode: Option<String>,
    pub statuses: Vec<String>,
    pub days_since_review: Option<i64>,
    pub include_never_reviewed: Option<bool>,
}

impl ProblemFilter {
    pub fn validate(&self) -> AppResult<()> {
        validate_problem_types(&self.problem_types)?;
        validate_statuses(&self.statuses)
    }

    pub fn from_value(subject_id: Option<Uuid>, value: &Value) -> AppResult<Self> {
        let filter = Self {
            subject_id,
            search_text: value
                .get("search_text")
                .and_then(Value::as_str)
                .map(ToOwned::to_owned),
            search_title: value.get("search_title").and_then(Value::as_bool),
            search_content: value.get("search_content").and_then(Value::as_bool),
            problem_types: strings_from_value(value.get("problem_types")),
            tag_ids: uuids_from_value(value.get("tag_ids"), "tag_ids")?,
            tag_filter_mode: value
                .get("tag_filter_mode")
                .and_then(Value::as_str)
                .map(ToOwned::to_owned),
            statuses: strings_from_value(value.get("statuses")),
            days_since_review: value.get("days_since_review").and_then(Value::as_i64),
            include_never_reviewed: value.get("include_never_reviewed").and_then(Value::as_bool),
        };
        filter.validate()?;
        Ok(filter)
    }

    pub fn push_sql(&self, qb: &mut QueryBuilder<'_, Postgres>, user_id: Uuid) {
        if let Some(subject_id) = self.subject_id {
            qb.push(" and p.subject_id = ").push_bind(subject_id);
        }

        if !self.tag_ids.is_empty() {
            if self.tag_filter_mode.as_deref() == Some("all") {
                qb.push(" and p.id in (select problem_id from problem_tag where user_id = ")
                    .push_bind(user_id)
                    .push(" and tag_id = any(")
                    .push_bind(self.tag_ids.clone())
                    .push(") group by problem_id having count(distinct tag_id) >= ")
                    .push_bind(self.tag_ids.len() as i64)
                    .push(")");
            } else {
                qb.push(" and p.id in (select problem_id from problem_tag where user_id = ")
                    .push_bind(user_id)
                    .push(" and tag_id = any(")
                    .push_bind(self.tag_ids.clone())
                    .push("))");
            }
        }

        if let Some(search_text) = self
            .search_text
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
        {
            let search_title = self.search_title.unwrap_or(false);
            let search_content = self.search_content.unwrap_or(false);
            if search_title || search_content {
                qb.push(" and (");
                let mut separated = qb.separated(" or ");
                let pattern = format!("%{search_text}%");
                if search_title {
                    separated.push("p.title ilike ").push_bind(pattern.clone());
                }
                if search_content {
                    separated
                        .push("p.content ilike ")
                        .push_bind(pattern.clone())
                        .push(" or p.solution_text ilike ")
                        .push_bind(pattern);
                }
                qb.push(")");
            }
        }

        if !self.problem_types.is_empty() {
            qb.push(" and p.problem_type = any(")
                .push_bind(self.problem_types.clone())
                .push(")");
        }
        if !self.statuses.is_empty() {
            qb.push(" and p.status = any(")
                .push_bind(self.statuses.clone())
                .push(")");
        }
        if let Some(days) = self.days_since_review {
            let cutoff = Utc::now() - Duration::days(days.max(0));
            if self.include_never_reviewed.unwrap_or(true) {
                qb.push(" and (p.last_reviewed_date < ")
                    .push_bind(cutoff)
                    .push(" or p.last_reviewed_date is null)");
            } else {
                qb.push(" and p.last_reviewed_date < ").push_bind(cutoff);
            }
        }
    }
}

pub fn parse_csv(value: Option<&str>) -> Vec<String> {
    value
        .unwrap_or_default()
        .split(',')
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned)
        .collect()
}

pub fn parse_uuid_csv(value: Option<&str>, label: &str) -> AppResult<Vec<Uuid>> {
    parse_csv(value)
        .into_iter()
        .map(|id| {
            Uuid::parse_str(&id)
                .map_err(|_| AppError::BadRequest(format!("Invalid {label} format")))
        })
        .collect()
}

pub fn validate_problem_types(values: &[String]) -> AppResult<()> {
    validate_filter_values("problem types", values, &["mcq", "short", "extended"])
}

pub fn validate_statuses(values: &[String]) -> AppResult<()> {
    validate_filter_values("statuses", values, &["wrong", "needs_review", "mastered"])
}

fn strings_from_value(value: Option<&Value>) -> Vec<String> {
    match value {
        Some(Value::Array(values)) => values
            .iter()
            .filter_map(Value::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(ToOwned::to_owned)
            .collect(),
        Some(Value::String(value)) => parse_csv(Some(value)),
        _ => Vec::new(),
    }
}

fn uuids_from_value(value: Option<&Value>, key: &str) -> AppResult<Vec<Uuid>> {
    match value {
        Some(Value::Array(values)) => values
            .iter()
            .filter_map(Value::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(|id| {
                Uuid::parse_str(id)
                    .map_err(|_| AppError::BadRequest(format!("Invalid {key} value")))
            })
            .collect(),
        Some(Value::String(value)) => parse_uuid_csv(Some(value), key),
        _ => Ok(Vec::new()),
    }
}

fn validate_filter_values(label: &str, values: &[String], allowed: &[&str]) -> AppResult<()> {
    let invalid: Vec<&str> = values
        .iter()
        .map(String::as_str)
        .filter(|value| !allowed.contains(value))
        .collect();
    if invalid.is_empty() {
        Ok(())
    } else {
        Err(AppError::BadRequest(format!(
            "Invalid {label}: {}",
            invalid.join(", ")
        )))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_filter_value_arrays_and_csv_strings() {
        let tag_id = Uuid::new_v4();
        let filter = ProblemFilter::from_value(
            None,
            &serde_json::json!({
                "statuses": "wrong,mastered",
                "problem_types": ["short"],
                "tag_ids": [tag_id.to_string()],
                "tag_filter_mode": "all",
                "days_since_review": 3,
                "include_never_reviewed": false
            }),
        )
        .expect("valid filter");

        assert_eq!(filter.statuses, vec!["wrong", "mastered"]);
        assert_eq!(filter.problem_types, vec!["short"]);
        assert_eq!(filter.tag_ids, vec![tag_id]);
        assert_eq!(filter.tag_filter_mode.as_deref(), Some("all"));
        assert_eq!(filter.days_since_review, Some(3));
        assert_eq!(filter.include_never_reviewed, Some(false));
    }

    #[test]
    fn rejects_invalid_filter_values() {
        let err = ProblemFilter::from_value(
            None,
            &serde_json::json!({
                "statuses": ["done"],
                "problem_types": ["essay"]
            }),
        )
        .expect_err("invalid values should fail");

        assert!(err.public_message().contains("Invalid problem types"));
    }
}
