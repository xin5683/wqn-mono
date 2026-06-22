use std::collections::{BTreeMap, BTreeSet};

use serde_json::{Map, Value, json};
use sqlx::types::Json as SqlJson;
use uuid::Uuid;

use crate::{error::AppResult, services::gemini, state::AppState};

const MIN_ACTIVITY_FOR_FULL_DIGEST: i64 = 5;

#[derive(Debug, Clone)]
struct SubjectSummary {
    id: String,
    name: String,
    color: Option<String>,
    total: i64,
    wrong: i64,
    needs_review: i64,
    mastered: i64,
}

#[derive(Debug, Clone)]
struct TopicAccumulator {
    subject_id: String,
    subject_name: String,
    subject_color: Option<String>,
    label: String,
    dominant_error_type: String,
    problem_ids: BTreeSet<String>,
    wrong_count: i64,
    needs_review_count: i64,
    mastered_count: i64,
}

impl TopicAccumulator {
    fn problem_count(&self) -> i64 {
        self.problem_ids.len() as i64
    }

    fn unresolved_count(&self) -> i64 {
        self.wrong_count + self.needs_review_count
    }
}

pub async fn generate_and_store_digest(state: &AppState, user_id: Uuid) -> AppResult<Value> {
    let fallback = build_digest(state, user_id).await?;
    let digest = generate_with_gemini(state, &fallback)
        .await
        .unwrap_or_else(|| fallback.clone());
    let row = sqlx::query_scalar::<_, SqlJson<Value>>(
        r#"
        insert into insight_digests (user_id, status, digest, generated_at)
        values ($1, 'completed', $2, now())
        returning to_jsonb(insight_digests) as data
        "#,
    )
    .bind(user_id)
    .bind(SqlJson(digest))
    .fetch_one(&state.pool)
    .await?;
    Ok(flatten_digest_row(row.0))
}

pub fn flatten_digest_row(mut row: Value) -> Value {
    let Some(object) = row.as_object_mut() else {
        return row;
    };
    let digest = object
        .remove("digest")
        .and_then(|value| value.as_object().cloned())
        .unwrap_or_default();
    for (key, value) in digest {
        object.entry(key).or_insert(value);
    }
    row
}

async fn generate_with_gemini(state: &AppState, fallback: &Value) -> Option<Value> {
    state.config.gemini_api_key.as_ref()?;

    let generated = gemini::generate_json(
        state,
        "Generate a wrong-question notebook insight digest. Return JSON only. Required fields: headline, error_pattern_summary, subject_error_patterns, subject_health, weak_spots, topic_clusters, progress_narratives, digest_tier. Preserve IDs exactly.",
        &format!(
            "Use this aggregation as source data and return the same schema with concise student-facing prose:\n\n{}",
            fallback
        ),
    )
    .await
    .ok()?;

    Some(merge_digest_defaults(generated, fallback))
}

async fn build_digest(state: &AppState, user_id: Uuid) -> AppResult<Value> {
    let subjects = subject_summaries(state, user_id).await?;
    let attempts = recent_attempt_context(state, user_id).await?;
    let mut clusters = topic_clusters_from_attempts(&attempts);

    if clusters.is_empty() {
        clusters = topic_clusters_from_subjects(&subjects);
    }

    let total_problems = subjects.iter().map(|subject| subject.total).sum::<i64>();
    let total_attempt_rows = attempts.len() as i64;
    let total_unresolved = subjects
        .iter()
        .map(|subject| subject.wrong + subject.needs_review)
        .sum::<i64>();
    let mastered = subjects.iter().map(|subject| subject.mastered).sum::<i64>();

    let weak_spots = weak_spots(&clusters);
    let topic_clusters = topic_cluster_json(&clusters);
    let subject_health = subject_health(&subjects, &clusters);
    let subject_error_patterns = subject_error_patterns(&clusters);
    let progress_narratives = progress_narratives(&subjects);
    let digest_tier = digest_tier(total_attempt_rows, total_unresolved);
    let headline = headline(total_problems, mastered, weak_spots.first());
    let error_pattern_summary = error_pattern_summary(&weak_spots, total_unresolved);

    Ok(json!({
        "headline": headline,
        "error_pattern_summary": error_pattern_summary,
        "subject_error_patterns": subject_error_patterns,
        "subject_health": subject_health,
        "weak_spots": weak_spots,
        "topic_clusters": topic_clusters,
        "progress_narratives": progress_narratives,
        "digest_tier": digest_tier,
        "raw_aggregation_data": {
            "total_problems": total_problems,
            "total_attempt_rows": total_attempt_rows,
            "total_unresolved": total_unresolved,
            "subject_count": subjects.len(),
            "generation_source": "local_aggregation",
        },
    }))
}

async fn subject_summaries(state: &AppState, user_id: Uuid) -> AppResult<Vec<SubjectSummary>> {
    let rows = sqlx::query_scalar::<_, SqlJson<Value>>(
        r#"
        select jsonb_build_object(
          'id', s.id,
          'name', s.name,
          'color', s.color,
          'total', count(p.id)::bigint,
          'wrong', count(p.id) filter (where p.status = 'wrong')::bigint,
          'needs_review', count(p.id) filter (where p.status = 'needs_review')::bigint,
          'mastered', count(p.id) filter (where p.status = 'mastered')::bigint
        ) as data
        from subjects s
        left join problems p on p.subject_id = s.id and p.user_id = s.user_id
        where s.user_id = $1
        group by s.id
        order by s.name asc
        "#,
    )
    .bind(user_id)
    .fetch_all(&state.pool)
    .await?;

    Ok(rows
        .into_iter()
        .filter_map(|SqlJson(row)| {
            Some(SubjectSummary {
                id: row.get("id")?.as_str()?.to_owned(),
                name: row.get("name")?.as_str()?.to_owned(),
                color: row
                    .get("color")
                    .and_then(Value::as_str)
                    .map(ToOwned::to_owned),
                total: row.get("total").and_then(Value::as_i64).unwrap_or(0),
                wrong: row.get("wrong").and_then(Value::as_i64).unwrap_or(0),
                needs_review: row.get("needs_review").and_then(Value::as_i64).unwrap_or(0),
                mastered: row.get("mastered").and_then(Value::as_i64).unwrap_or(0),
            })
        })
        .collect())
}

async fn recent_attempt_context(state: &AppState, user_id: Uuid) -> AppResult<Vec<Value>> {
    let rows = sqlx::query_scalar::<_, SqlJson<Value>>(
        r#"
        select jsonb_build_object(
          'subject_id', s.id,
          'subject_name', s.name,
          'subject_color', s.color,
          'topic_label', coalesce(nullif(ec.topic_label, ''), nullif(ec.granular_tag, ''), p.title, 'Uncategorised'),
          'broad_category', coalesce(nullif(ec.broad_category, ''), nullif(ec.granular_tag, ''), coalesce(a.selected_status, p.status, 'needs_review')),
          'problem_id', p.id,
          'problem_status', p.status,
          'selected_status', a.selected_status,
          'is_correct', a.is_correct,
          'created_at', a.created_at
        ) as data
        from attempts a
        join problems p on p.id = a.problem_id
        join subjects s on s.id = p.subject_id
        left join error_categorisations ec on ec.attempt_id = a.id and ec.user_id = a.user_id
        where a.user_id = $1
          and a.created_at >= now() - interval '90 days'
        order by a.created_at desc
        limit 100
        "#,
    )
    .bind(user_id)
    .fetch_all(&state.pool)
    .await?;

    Ok(rows.into_iter().map(|SqlJson(row)| row).collect())
}

fn topic_clusters_from_attempts(rows: &[Value]) -> Vec<TopicAccumulator> {
    let mut grouped = BTreeMap::<(String, String), TopicAccumulator>::new();
    for row in rows {
        let subject_id = string_field(row, "subject_id", "");
        if subject_id.is_empty() {
            continue;
        }
        let label = string_field(row, "topic_label", "Uncategorised");
        let key = (subject_id.clone(), label.clone());
        let status = string_field(row, "selected_status", "");
        let status = if status.is_empty() {
            string_field(row, "problem_status", "needs_review")
        } else {
            status
        };
        let problem_id = string_field(row, "problem_id", "");
        let broad_category = string_field(row, "broad_category", "needs_review");
        let entry = grouped.entry(key).or_insert_with(|| TopicAccumulator {
            subject_id,
            subject_name: string_field(row, "subject_name", "Subject"),
            subject_color: row
                .get("subject_color")
                .and_then(Value::as_str)
                .map(ToOwned::to_owned),
            label,
            dominant_error_type: broad_category,
            problem_ids: BTreeSet::new(),
            wrong_count: 0,
            needs_review_count: 0,
            mastered_count: 0,
        });
        if !problem_id.is_empty() {
            entry.problem_ids.insert(problem_id);
        }
        match status.as_str() {
            "wrong" => entry.wrong_count += 1,
            "mastered" => entry.mastered_count += 1,
            _ => {
                if row.get("is_correct").and_then(Value::as_bool) == Some(false) {
                    entry.wrong_count += 1;
                } else {
                    entry.needs_review_count += 1;
                }
            }
        }
    }
    grouped.into_values().collect()
}

fn topic_clusters_from_subjects(subjects: &[SubjectSummary]) -> Vec<TopicAccumulator> {
    subjects
        .iter()
        .filter(|subject| subject.total > 0)
        .map(|subject| TopicAccumulator {
            subject_id: subject.id.clone(),
            subject_name: subject.name.clone(),
            subject_color: subject.color.clone(),
            label: format!("{} review queue", subject.name),
            dominant_error_type: if subject.wrong > 0 {
                "wrong".to_owned()
            } else if subject.needs_review > 0 {
                "needs_review".to_owned()
            } else {
                "mastered".to_owned()
            },
            problem_ids: BTreeSet::new(),
            wrong_count: subject.wrong,
            needs_review_count: subject.needs_review,
            mastered_count: subject.mastered,
        })
        .collect()
}

fn weak_spots(clusters: &[TopicAccumulator]) -> Vec<Value> {
    let mut clusters = clusters
        .iter()
        .filter(|cluster| cluster.unresolved_count() > 0)
        .collect::<Vec<_>>();
    clusters.sort_by_key(|cluster| -cluster.unresolved_count());
    clusters
        .into_iter()
        .take(7)
        .map(|cluster| {
            json!({
                "topic_label": cluster.label,
                "subject_id": cluster.subject_id,
                "subject_name": cluster.subject_name,
                "subject_color": cluster.subject_color,
                "problem_count": cluster.problem_count().max(cluster.unresolved_count()),
                "trend_phrase": format!("{} unresolved item(s) need another pass.", cluster.unresolved_count()),
                "dominant_error_type": cluster.dominant_error_type,
                "problem_ids": cluster.problem_ids.iter().cloned().collect::<Vec<_>>(),
            })
        })
        .collect()
}

fn topic_cluster_json(clusters: &[TopicAccumulator]) -> Value {
    let mut by_subject = Map::new();
    for cluster in clusters {
        by_subject
            .entry(cluster.subject_id.clone())
            .or_insert_with(|| Value::Array(Vec::new()));
        if let Some(Value::Array(items)) = by_subject.get_mut(&cluster.subject_id) {
            items.push(json!({
                "label": cluster.label,
                "problem_count": cluster.problem_count().max(cluster.wrong_count + cluster.needs_review_count + cluster.mastered_count),
                "wrong_count": cluster.wrong_count,
                "needs_review_count": cluster.needs_review_count,
                "mastered_count": cluster.mastered_count,
                "narrative": cluster_narrative(cluster),
                "problem_ids": cluster.problem_ids.iter().cloned().collect::<Vec<_>>(),
            }));
        }
    }
    Value::Object(by_subject)
}

fn subject_health(subjects: &[SubjectSummary], clusters: &[TopicAccumulator]) -> Value {
    let mut by_subject = Map::new();
    for subject in subjects {
        let unresolved = subject.wrong + subject.needs_review;
        let cluster_count = clusters
            .iter()
            .filter(|cluster| cluster.subject_id == subject.id && cluster.unresolved_count() > 0)
            .count();
        let summary = if subject.total == 0 {
            "No problems recorded yet.".to_owned()
        } else if unresolved == 0 {
            "All tracked problems are currently mastered.".to_owned()
        } else {
            format!(
                "{} unresolved problem(s) across {} active topic area(s).",
                unresolved, cluster_count
            )
        };
        by_subject.insert(subject.id.clone(), Value::String(summary));
    }
    Value::Object(by_subject)
}

fn subject_error_patterns(clusters: &[TopicAccumulator]) -> Value {
    let mut by_subject: BTreeMap<String, Vec<&TopicAccumulator>> = BTreeMap::new();
    for cluster in clusters {
        if cluster.unresolved_count() > 0 {
            by_subject
                .entry(cluster.subject_id.clone())
                .or_default()
                .push(cluster);
        }
    }
    let mut result = Map::new();
    for (subject_id, mut items) in by_subject {
        items.sort_by_key(|cluster| -cluster.unresolved_count());
        let labels = items
            .iter()
            .take(3)
            .map(|cluster| cluster.label.as_str())
            .collect::<Vec<_>>()
            .join(", ");
        result.insert(
            subject_id,
            Value::String(format!("Most unresolved work is concentrated in {labels}.")),
        );
    }
    Value::Object(result)
}

fn progress_narratives(subjects: &[SubjectSummary]) -> Value {
    let mut result = Map::new();
    for subject in subjects {
        let narrative = if subject.total == 0 {
            "Add problems to start tracking progress.".to_owned()
        } else {
            format!(
                "{} of {} problem(s) are mastered; {} still need review.",
                subject.mastered,
                subject.total,
                subject.wrong + subject.needs_review
            )
        };
        result.insert(subject.id.clone(), Value::String(narrative));
    }
    Value::Object(result)
}

fn digest_tier(total_attempt_rows: i64, total_unresolved: i64) -> &'static str {
    if total_unresolved == 0 {
        "mastery"
    } else if total_attempt_rows < MIN_ACTIVITY_FOR_FULL_DIGEST {
        "narrow"
    } else {
        "full"
    }
}

fn headline(total_problems: i64, mastered: i64, top_weak_spot: Option<&Value>) -> String {
    if let Some(topic) = top_weak_spot.and_then(|spot| spot.get("topic_label")) {
        return format!(
            "Focus next on {}.",
            topic.as_str().unwrap_or("your main weak spot")
        );
    }
    if total_problems == 0 {
        "Add problems to unlock learning insights.".to_owned()
    } else {
        format!("{mastered} of {total_problems} tracked problem(s) are mastered.")
    }
}

fn error_pattern_summary(weak_spots: &[Value], total_unresolved: i64) -> String {
    if weak_spots.is_empty() {
        return "No recurring error pattern is visible in the current data.".to_owned();
    }
    let labels = weak_spots
        .iter()
        .take(3)
        .filter_map(|spot| spot.get("topic_label").and_then(Value::as_str))
        .collect::<Vec<_>>()
        .join(", ");
    format!("{total_unresolved} unresolved item(s) are currently concentrated around {labels}.")
}

fn cluster_narrative(cluster: &TopicAccumulator) -> String {
    format!(
        "{} wrong, {} needs review, {} mastered.",
        cluster.wrong_count, cluster.needs_review_count, cluster.mastered_count
    )
}

fn merge_digest_defaults(mut generated: Value, fallback: &Value) -> Value {
    let required = [
        "headline",
        "error_pattern_summary",
        "subject_error_patterns",
        "subject_health",
        "weak_spots",
        "topic_clusters",
        "progress_narratives",
        "digest_tier",
        "raw_aggregation_data",
    ];
    let Some(generated_object) = generated.as_object_mut() else {
        return fallback.clone();
    };
    for key in required {
        let missing = generated_object
            .get(key)
            .is_none_or(|value| value.is_null() || value == "");
        if missing && let Some(default_value) = fallback.get(key) {
            generated_object.insert(key.to_owned(), default_value.clone());
        }
    }
    generated
}

fn string_field(row: &Value, key: &str, fallback: &str) -> String {
    row.get(key)
        .and_then(Value::as_str)
        .filter(|value| !value.is_empty())
        .unwrap_or(fallback)
        .to_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn flattens_digest_metadata_into_digest_payload() {
        let flattened = flatten_digest_row(json!({
            "id": "digest-id",
            "user_id": "user-id",
            "status": "completed",
            "generated_at": "2026-06-19T00:00:00Z",
            "digest": {
                "headline": "Review algebra",
                "weak_spots": [],
            }
        }));

        assert_eq!(flattened["id"], "digest-id");
        assert_eq!(flattened["headline"], "Review algebra");
        assert!(flattened.get("digest").is_none());
    }

    #[test]
    fn local_digest_has_required_frontend_fields() {
        let subject = SubjectSummary {
            id: "subject-1".to_owned(),
            name: "Math".to_owned(),
            color: Some("blue".to_owned()),
            total: 3,
            wrong: 1,
            needs_review: 1,
            mastered: 1,
        };
        let clusters = topic_clusters_from_subjects(std::slice::from_ref(&subject));
        let weak_spots = weak_spots(&clusters);
        let digest = json!({
            "headline": headline(3, 1, weak_spots.first()),
            "error_pattern_summary": error_pattern_summary(&weak_spots, 2),
            "subject_error_patterns": subject_error_patterns(&clusters),
            "subject_health": subject_health(&[subject], &clusters),
            "weak_spots": weak_spots,
            "topic_clusters": topic_cluster_json(&clusters),
            "progress_narratives": progress_narratives(&[SubjectSummary {
                id: "subject-1".to_owned(),
                name: "Math".to_owned(),
                color: Some("blue".to_owned()),
                total: 3,
                wrong: 1,
                needs_review: 1,
                mastered: 1,
            }]),
            "digest_tier": digest_tier(0, 2),
        });

        assert!(
            digest["headline"]
                .as_str()
                .is_some_and(|value| !value.is_empty())
        );
        assert!(
            digest["error_pattern_summary"]
                .as_str()
                .is_some_and(|value| !value.is_empty())
        );
        assert!(digest["subject_health"]["subject-1"].is_string());
        assert!(digest["topic_clusters"]["subject-1"].is_array());
        assert_eq!(digest["digest_tier"], "narrow");
    }
}
