use std::collections::HashMap;

use serde::Deserialize;
use serde_json::Value;
use sqlx::{Postgres, QueryBuilder, types::Json as SqlJson};
use uuid::Uuid;
use validator::Validate;

use crate::{
    dto::patch::PatchField,
    dto::problem::{ProblemDto, ProblemRow, ProblemTagDto, ProblemTagRow},
    error::{AppError, AppResult, validation},
    models::{AnswerConfig, Asset, CreateProblem, ShortAnswerMode},
    services::{
        content_limits,
        problem_filters::{
            ProblemFilter, parse_csv, parse_uuid_csv, validate_problem_types, validate_statuses,
        },
        review, storage,
    },
};

pub const PROBLEM_ROW_SELECT: &str = r#"
        select
          p.id,
          p.user_id,
          p.subject_id,
          p.title,
          p.content,
          p.problem_type,
          p.correct_answer,
          p.answer_config,
          p.auto_mark,
          p.status,
          coalesce(p.assets, '[]'::jsonb) as assets,
          p.solution_text,
          coalesce(p.solution_assets, '[]'::jsonb) as solution_assets,
          p.last_reviewed_date,
          p.created_at,
          p.updated_at
        from problems p
"#;

#[derive(Debug, Deserialize)]
pub struct ProblemsQuery {
    subject_id: Option<Uuid>,
    search_text: Option<String>,
    search_title: Option<bool>,
    search_content: Option<bool>,
    problem_types: Option<String>,
    tag_ids: Option<String>,
    tag_filter_mode: Option<String>,
    statuses: Option<String>,
    days_since_review: Option<i64>,
    include_never_reviewed: Option<bool>,
}

#[derive(Debug, Deserialize)]
pub struct PatchProblem {
    #[serde(default)]
    subject_id: PatchField<Uuid>,
    #[serde(default)]
    title: PatchField<String>,
    #[serde(default)]
    content: PatchField<String>,
    #[serde(default)]
    problem_type: PatchField<String>,
    #[serde(default)]
    correct_answer: PatchField<String>,
    #[serde(default)]
    answer_config: PatchField<AnswerConfig>,
    #[serde(default)]
    auto_mark: PatchField<bool>,
    #[serde(default)]
    status: PatchField<String>,
    #[serde(default)]
    assets: PatchField<Vec<Asset>>,
    #[serde(default)]
    solution_text: PatchField<String>,
    #[serde(default)]
    solution_assets: PatchField<Vec<Asset>>,
    #[serde(default)]
    last_reviewed_date: PatchField<chrono::DateTime<chrono::Utc>>,
    #[serde(default)]
    tag_ids: PatchField<Vec<Uuid>>,
}

pub async fn list_user_problems(
    pool: &sqlx::PgPool,
    user_id: Uuid,
    query: ProblemsQuery,
) -> AppResult<Vec<ProblemDto>> {
    let tag_ids = parse_uuid_csv(query.tag_ids.as_deref(), "tag ID")?;
    let problem_types = parse_csv(query.problem_types.as_deref());
    let statuses = parse_csv(query.statuses.as_deref());
    let filter = ProblemFilter {
        subject_id: query.subject_id,
        search_text: query.search_text,
        search_title: query.search_title,
        search_content: query.search_content,
        problem_types,
        tag_ids,
        tag_filter_mode: query.tag_filter_mode,
        statuses,
        days_since_review: query.days_since_review,
        include_never_reviewed: query.include_never_reviewed,
    };
    filter.validate()?;

    let mut qb = QueryBuilder::<Postgres>::new(PROBLEM_ROW_SELECT);
    qb.push(" where p.user_id = ");
    qb.push_bind(user_id);
    filter.push_sql(&mut qb, user_id);
    qb.push(" order by p.created_at desc");

    let rows = qb.build_query_as::<ProblemRow>().fetch_all(pool).await?;
    tracing::debug!(
        user_id = %user_id,
        subject_id = ?query.subject_id,
        count = rows.len(),
        "listed problems"
    );
    problem_dtos_from_rows(pool, user_id, rows).await
}

const MCQ_MIN_CHOICES: usize = 2;
const MCQ_MAX_CHOICES: usize = 10;
const MCQ_CHOICE_ID_MAX_LEN: usize = 10;
const MCQ_CHOICE_TEXT_MAX_LEN: usize = 500;
const SHORT_MAX_ANSWERS: usize = 20;
const SHORT_ANSWER_MAX_LEN: usize = 200;
const NUMERIC_UNIT_MAX_LEN: usize = 50;

/// Enforce `answer_config` business rules that serde cannot express on its own
/// (cross-field and mode-conditional constraints). Mirrors the frontend
/// `AnswerConfigSchema` refines so the backend is the authoritative write-side
/// guard, not the frontend Zod. Bounds match `ANSWER_CONFIG_CONSTANTS`.
pub fn validate_answer_config(config: &AnswerConfig) -> AppResult<()> {
    match config {
        AnswerConfig::Mcq {
            choices,
            correct_choice_id,
            ..
        } => {
            if choices.len() < MCQ_MIN_CHOICES || choices.len() > MCQ_MAX_CHOICES {
                return Err(validation(
                    format!(
                        "choices must contain between {MCQ_MIN_CHOICES} and {MCQ_MAX_CHOICES} items"
                    ),
                    serde_json::json!({ "field": "choices", "count": choices.len() }),
                ));
            }
            for (index, choice) in choices.iter().enumerate() {
                if choice.id.is_empty() || choice.id.chars().count() > MCQ_CHOICE_ID_MAX_LEN {
                    return Err(validation(
                        format!("choice id must be 1-{MCQ_CHOICE_ID_MAX_LEN} characters"),
                        serde_json::json!({ "field": "choices", "index": index }),
                    ));
                }
                if choice.text.chars().count() > MCQ_CHOICE_TEXT_MAX_LEN {
                    return Err(validation(
                        format!("choice text must be at most {MCQ_CHOICE_TEXT_MAX_LEN} characters"),
                        serde_json::json!({ "field": "choices", "index": index }),
                    ));
                }
            }
            if !choices.iter().any(|choice| choice.id == *correct_choice_id) {
                return Err(validation(
                    "correct_choice_id must match one of the choice IDs",
                    serde_json::json!({
                        "field": "correct_choice_id",
                        "value": correct_choice_id,
                    }),
                ));
            }
        }
        AnswerConfig::Short {
            mode,
            acceptable_answers,
            numeric_config,
        } => match mode {
            ShortAnswerMode::Text => {
                let answers = acceptable_answers.as_ref().ok_or_else(|| {
                    validation(
                        "acceptable_answers is required for short text mode",
                        serde_json::json!({ "field": "acceptable_answers" }),
                    )
                })?;
                if answers.is_empty() || answers.len() > SHORT_MAX_ANSWERS {
                    return Err(validation(
                        format!("acceptable_answers must contain 1-{SHORT_MAX_ANSWERS} items"),
                        serde_json::json!({
                            "field": "acceptable_answers",
                            "count": answers.len(),
                        }),
                    ));
                }
                for (index, answer) in answers.iter().enumerate() {
                    if answer.is_empty() || answer.chars().count() > SHORT_ANSWER_MAX_LEN {
                        return Err(validation(
                            format!(
                                "acceptable answer must be 1-{SHORT_ANSWER_MAX_LEN} characters"
                            ),
                            serde_json::json!({ "field": "acceptable_answers", "index": index }),
                        ));
                    }
                }
            }
            ShortAnswerMode::Numeric => {
                let numeric = numeric_config.as_ref().ok_or_else(|| {
                    validation(
                        "numeric_config is required for short numeric mode",
                        serde_json::json!({ "field": "numeric_config" }),
                    )
                })?;
                if numeric.tolerance < 0.0 {
                    return Err(validation(
                        "tolerance must be non-negative",
                        serde_json::json!({
                            "field": "tolerance",
                            "value": numeric.tolerance,
                        }),
                    ));
                }
                if let Some(unit) = &numeric.unit
                    && unit.chars().count() > NUMERIC_UNIT_MAX_LEN
                {
                    return Err(validation(
                        format!("unit must be at most {NUMERIC_UNIT_MAX_LEN} characters"),
                        serde_json::json!({ "field": "unit" }),
                    ));
                }
            }
        },
    }
    Ok(())
}

pub async fn create_problem(
    pool: &sqlx::PgPool,
    user_id: Uuid,
    body: CreateProblem,
) -> AppResult<ProblemDto> {
    body.validate().map_err(|err| {
        validation(
            "Invalid request body",
            serde_json::to_value(err).unwrap_or_default(),
        )
    })?;
    validate_problem_types(std::slice::from_ref(&body.problem_type))?;
    if let Some(status) = &body.status {
        validate_statuses(std::slice::from_ref(status))?;
    }
    if let Some(config) = &body.answer_config {
        validate_answer_config(config)?;
    }

    let assets = body.assets.unwrap_or_default();
    let solution_assets = body.solution_assets.unwrap_or_default();
    if !storage::all_assets_owned_by_user(user_id, &assets, &solution_assets) {
        return Err(AppError::Forbidden);
    }

    let limit = content_limits::check_content_limit(
        pool,
        user_id,
        content_limits::PROBLEMS_PER_SUBJECT,
        Some(body.subject_id),
    )
    .await?;
    if !limit
        .get("allowed")
        .and_then(Value::as_bool)
        .unwrap_or(false)
    {
        return Err(AppError::Forbidden);
    }

    let problem_id = sqlx::query_scalar::<_, Uuid>(
        r#"
        insert into problems (
          id, user_id, subject_id, title, content, problem_type, correct_answer,
          answer_config, auto_mark, status, assets, solution_text,
          solution_assets, last_reviewed_date
        )
        values (
          coalesce($1, gen_random_uuid()), $2, $3, $4, $5, $6, $7,
          $8, coalesce($9, false), coalesce($10, 'needs_review'), $11, $12,
          $13, $14
        )
        returning id
        "#,
    )
    .bind(body.id)
    .bind(user_id)
    .bind(body.subject_id)
    .bind(body.title)
    .bind(body.content)
    .bind(body.problem_type)
    .bind(body.correct_answer)
    .bind(body.answer_config.map(SqlJson))
    .bind(body.auto_mark)
    .bind(body.status)
    .bind(SqlJson(assets))
    .bind(body.solution_text)
    .bind(SqlJson(solution_assets))
    .bind(body.last_reviewed_date)
    .fetch_one(pool)
    .await?;

    replace_problem_tags(pool, user_id, problem_id, body.tag_ids.unwrap_or_default()).await?;
    review::ensure_initial_schedule(pool, user_id, problem_id).await?;
    tracing::info!(
        problem_id = %problem_id,
        subject_id = %body.subject_id,
        user_id = %user_id,
        "problem created"
    );
    fetch_problem(pool, user_id, problem_id).await
}

pub async fn fetch_problem(pool: &sqlx::PgPool, user_id: Uuid, id: Uuid) -> AppResult<ProblemDto> {
    let sql = format!("{PROBLEM_ROW_SELECT} where p.id = $1 and p.user_id = $2");
    let row = sqlx::query_as::<_, ProblemRow>(&sql)
        .bind(id)
        .bind(user_id)
        .fetch_optional(pool)
        .await?
        .ok_or_else(|| AppError::NotFound("Not found".to_owned()))?;
    problem_dto_from_row(pool, user_id, row).await
}

/// Whether a problem row owned by `user_id` exists with this id. Used by the
/// draft-cleanup path to ensure we only ever reclaim files for a problem that
/// was *never* persisted — never one that has been saved.
pub async fn problem_exists(
    pool: &sqlx::PgPool,
    user_id: Uuid,
    problem_id: Uuid,
) -> AppResult<bool> {
    let exists = sqlx::query_scalar::<_, bool>(
        "select exists(select 1 from problems where id = $1 and user_id = $2)",
    )
    .bind(problem_id)
    .bind(user_id)
    .fetch_one(pool)
    .await?;
    Ok(exists)
}

pub async fn update_problem(
    pool: &sqlx::PgPool,
    user_id: Uuid,
    id: Uuid,
    body: PatchProblem,
) -> AppResult<ProblemDto> {
    if let Some(problem_type) = required_patch_value(&body.problem_type, "problem_type")? {
        validate_problem_types(std::slice::from_ref(problem_type))?;
    }
    if let Some(status) = required_patch_value(&body.status, "status")? {
        validate_statuses(std::slice::from_ref(status))?;
    }
    required_patch_value(&body.subject_id, "subject_id")?;
    required_patch_value(&body.title, "title")?;
    required_patch_value(&body.auto_mark, "auto_mark")?;

    if let PatchField::Value(config) = &body.answer_config {
        validate_answer_config(config)?;
    }

    if !storage::all_assets_owned_by_user(
        user_id,
        body.assets.as_slice_or_empty(),
        body.solution_assets.as_slice_or_empty(),
    ) {
        return Err(AppError::Forbidden);
    }

    update_problem_row(pool, user_id, id, body).await?;
    fetch_problem(pool, user_id, id).await
}

pub async fn delete_problem(pool: &sqlx::PgPool, user_id: Uuid, id: Uuid) -> AppResult<()> {
    let affected = sqlx::query("delete from problems where id = $1 and user_id = $2")
        .bind(id)
        .bind(user_id)
        .execute(pool)
        .await?
        .rows_affected();
    if affected == 0 {
        return Err(AppError::NotFound("Not found".to_owned()));
    }
    Ok(())
}

pub async fn update_problem_assets(
    pool: &sqlx::PgPool,
    user_id: Uuid,
    id: Uuid,
    body: &Value,
) -> AppResult<ProblemDto> {
    let assets = asset_patch_from_value(body, "assets")?;
    let solution_assets = asset_patch_from_value(body, "solution_assets")?;
    if !storage::all_assets_owned_by_user(
        user_id,
        assets.as_deref().unwrap_or_default(),
        solution_assets.as_deref().unwrap_or_default(),
    ) {
        return Err(AppError::Forbidden);
    }

    let affected = sqlx::query(
        r#"
        update problems
        set
          assets = coalesce($3, assets),
          solution_assets = coalesce($4, solution_assets),
          updated_at = now()
        where id = $1 and user_id = $2
        "#,
    )
    .bind(id)
    .bind(user_id)
    .bind(assets.map(SqlJson))
    .bind(solution_assets.map(SqlJson))
    .execute(pool)
    .await?
    .rows_affected();
    if affected == 0 {
        return Err(AppError::NotFound("Not found".to_owned()));
    }
    fetch_problem(pool, user_id, id).await
}

pub async fn filter_count(pool: &sqlx::PgPool, user_id: Uuid, body: &Value) -> AppResult<i64> {
    let filter_config = body.get("filter_config").unwrap_or(body);
    let filter = ProblemFilter::from_value(
        body.get("subject_id")
            .and_then(Value::as_str)
            .and_then(|id| Uuid::parse_str(id).ok()),
        filter_config,
    )?;

    let mut qb =
        QueryBuilder::<Postgres>::new("select count(*)::bigint from problems p where p.user_id = ");
    qb.push_bind(user_id);
    filter.push_sql(&mut qb, user_id);
    Ok(qb.build_query_scalar::<i64>().fetch_one(pool).await?)
}

pub async fn fetch_problems_by_ids(
    pool: &sqlx::PgPool,
    user_id: Uuid,
    problem_ids: &[Uuid],
) -> AppResult<HashMap<Uuid, ProblemDto>> {
    if problem_ids.is_empty() {
        return Ok(HashMap::new());
    }

    let mut qb = QueryBuilder::<Postgres>::new(PROBLEM_ROW_SELECT);
    qb.push(" where p.user_id = ")
        .push_bind(user_id)
        .push(" and p.id = any(")
        .push_bind(problem_ids)
        .push(")");

    let rows = qb.build_query_as::<ProblemRow>().fetch_all(pool).await?;
    let problems = problem_dtos_from_rows(pool, user_id, rows).await?;
    Ok(problems
        .into_iter()
        .map(|problem| (problem.id, problem))
        .collect())
}

pub async fn problem_dtos_from_rows(
    pool: &sqlx::PgPool,
    user_id: Uuid,
    rows: Vec<ProblemRow>,
) -> AppResult<Vec<ProblemDto>> {
    let problem_ids = rows.iter().map(|row| row.id).collect::<Vec<_>>();
    let mut tags_by_problem = fetch_problem_tags(pool, user_id, &problem_ids).await?;
    Ok(rows
        .into_iter()
        .map(|row| {
            let tags = tags_by_problem.remove(&row.id).unwrap_or_default();
            ProblemDto::from_row(row, tags)
        })
        .collect())
}

async fn problem_dto_from_row(
    pool: &sqlx::PgPool,
    user_id: Uuid,
    row: ProblemRow,
) -> AppResult<ProblemDto> {
    let mut tags_by_problem = fetch_problem_tags(pool, user_id, &[row.id]).await?;
    let tags = tags_by_problem.remove(&row.id).unwrap_or_default();
    Ok(ProblemDto::from_row(row, tags))
}

async fn fetch_problem_tags(
    pool: &sqlx::PgPool,
    user_id: Uuid,
    problem_ids: &[Uuid],
) -> AppResult<HashMap<Uuid, Vec<ProblemTagDto>>> {
    if problem_ids.is_empty() {
        return Ok(HashMap::new());
    }

    let rows = sqlx::query_as::<_, ProblemTagRow>(
        r#"
        select
          pt.problem_id,
          t.id,
          t.user_id,
          t.subject_id,
          t.name,
          t.created_at,
          t.updated_at
        from problem_tag pt
        join tags t on t.id = pt.tag_id and t.user_id = pt.user_id
        where pt.user_id = $1 and pt.problem_id = any($2)
        order by pt.problem_id, t.name
        "#,
    )
    .bind(user_id)
    .bind(problem_ids)
    .fetch_all(pool)
    .await?;

    let mut tags_by_problem: HashMap<Uuid, Vec<ProblemTagDto>> = HashMap::new();
    for row in rows {
        tags_by_problem
            .entry(row.problem_id)
            .or_default()
            .push(row.into());
    }
    Ok(tags_by_problem)
}

async fn update_problem_row(
    pool: &sqlx::PgPool,
    user_id: Uuid,
    id: Uuid,
    body: PatchProblem,
) -> AppResult<()> {
    let (subject_id_present, subject_id) = into_required_patch(body.subject_id, "subject_id")?;
    let (title_present, title) = into_required_patch(body.title, "title")?;
    let (content_present, content) = body.content.into_nullable();
    let (problem_type_present, problem_type) =
        into_required_patch(body.problem_type, "problem_type")?;
    let (correct_answer_present, correct_answer) = body.correct_answer.into_nullable();
    let (answer_config_present, answer_config) = body.answer_config.into_nullable();
    let (auto_mark_present, auto_mark) = into_required_patch(body.auto_mark, "auto_mark")?;
    let (status_present, status) = into_required_patch(body.status, "status")?;
    let (assets_present, assets) = body.assets.into_vec_or_empty();
    let (solution_text_present, solution_text) = body.solution_text.into_nullable();
    let (solution_assets_present, solution_assets) = body.solution_assets.into_vec_or_empty();
    let (last_reviewed_date_present, last_reviewed_date) = body.last_reviewed_date.into_nullable();
    let (tag_ids_present, tag_ids) = body.tag_ids.into_vec_or_empty();

    let affected = sqlx::query(
        r#"
        update problems
        set
          subject_id = case when $3 then $4 else subject_id end,
          title = case when $5 then $6 else title end,
          content = case when $7 then $8 else content end,
          problem_type = case when $9 then $10 else problem_type end,
          correct_answer = case when $11 then $12 else correct_answer end,
          answer_config = case when $13 then $14 else answer_config end,
          auto_mark = case when $15 then $16 else auto_mark end,
          status = case when $17 then $18 else status end,
          assets = case when $19 then $20 else assets end,
          solution_text = case when $21 then $22 else solution_text end,
          solution_assets = case when $23 then $24 else solution_assets end,
          last_reviewed_date = case when $25 then $26 else last_reviewed_date end,
          updated_at = now()
        where id = $1 and user_id = $2
        "#,
    )
    .bind(id)
    .bind(user_id)
    .bind(subject_id_present)
    .bind(subject_id)
    .bind(title_present)
    .bind(title)
    .bind(content_present)
    .bind(content)
    .bind(problem_type_present)
    .bind(problem_type)
    .bind(correct_answer_present)
    .bind(correct_answer)
    .bind(answer_config_present)
    .bind(answer_config.map(SqlJson))
    .bind(auto_mark_present)
    .bind(auto_mark)
    .bind(status_present)
    .bind(status)
    .bind(assets_present)
    .bind(assets.map(SqlJson))
    .bind(solution_text_present)
    .bind(solution_text)
    .bind(solution_assets_present)
    .bind(solution_assets.map(SqlJson))
    .bind(last_reviewed_date_present)
    .bind(last_reviewed_date)
    .execute(pool)
    .await?
    .rows_affected();
    if affected == 0 {
        return Err(AppError::NotFound("Not found".to_owned()));
    }
    if tag_ids_present {
        replace_problem_tags(pool, user_id, id, tag_ids.unwrap_or_default()).await?;
    }
    Ok(())
}

fn required_patch_value<'a, T>(
    field: &'a PatchField<T>,
    field_name: &str,
) -> AppResult<Option<&'a T>> {
    match field {
        PatchField::Missing => Ok(None),
        PatchField::Null => Err(AppError::BadRequest(format!("{field_name} cannot be null"))),
        PatchField::Value(value) => Ok(Some(value)),
    }
}

fn into_required_patch<T>(field: PatchField<T>, field_name: &str) -> AppResult<(bool, Option<T>)> {
    match field {
        PatchField::Missing => Ok((false, None)),
        PatchField::Null => Err(AppError::BadRequest(format!("{field_name} cannot be null"))),
        PatchField::Value(value) => Ok((true, Some(value))),
    }
}

fn asset_patch_from_value(body: &Value, key: &str) -> AppResult<Option<Vec<Asset>>> {
    match body.get(key) {
        None => Ok(None),
        Some(Value::Null) => Ok(Some(Vec::new())),
        Some(value) => serde_json::from_value(value.clone())
            .map(Some)
            .map_err(|err| AppError::BadRequest(err.to_string())),
    }
}

async fn replace_problem_tags(
    pool: &sqlx::PgPool,
    user_id: Uuid,
    problem_id: Uuid,
    tag_ids: Vec<Uuid>,
) -> AppResult<()> {
    let mut tx = pool.begin().await?;
    sqlx::query("delete from problem_tag where user_id = $1 and problem_id = $2")
        .bind(user_id)
        .bind(problem_id)
        .execute(&mut *tx)
        .await?;
    for tag_id in tag_ids {
        sqlx::query(
            r#"
            insert into problem_tag (user_id, problem_id, tag_id)
            values ($1, $2, $3)
            on conflict do nothing
            "#,
        )
        .bind(user_id)
        .bind(problem_id)
        .bind(tag_id)
        .execute(&mut *tx)
        .await?;
    }
    tx.commit().await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::validate_answer_config;
    use crate::models::{AnswerChoice, AnswerConfig, NumericAnswerConfig, ShortAnswerMode};

    fn valid_mcq() -> AnswerConfig {
        AnswerConfig::Mcq {
            choices: vec![
                AnswerChoice {
                    id: "a".into(),
                    text: "A".into(),
                },
                AnswerChoice {
                    id: "b".into(),
                    text: "B".into(),
                },
            ],
            correct_choice_id: "a".into(),
            randomize_choices: true,
        }
    }

    #[test]
    fn accepts_valid_answer_configs() {
        assert!(validate_answer_config(&valid_mcq()).is_ok());

        let short_text = AnswerConfig::Short {
            mode: ShortAnswerMode::Text,
            acceptable_answers: Some(vec!["42".into()]),
            numeric_config: None,
        };
        assert!(validate_answer_config(&short_text).is_ok());

        let short_numeric = AnswerConfig::Short {
            mode: ShortAnswerMode::Numeric,
            acceptable_answers: None,
            numeric_config: Some(NumericAnswerConfig {
                correct_value: 9.8,
                tolerance: 0.1,
                unit: Some("m/s²".into()),
            }),
        };
        assert!(validate_answer_config(&short_numeric).is_ok());
    }

    #[test]
    fn rejects_mcq_with_correct_choice_not_in_choices() {
        let mut mcq = valid_mcq();
        if let AnswerConfig::Mcq {
            correct_choice_id, ..
        } = &mut mcq
        {
            *correct_choice_id = "zzz".into();
        }
        assert!(validate_answer_config(&mcq).is_err());
    }

    #[test]
    fn rejects_mcq_with_too_few_choices() {
        let mcq = AnswerConfig::Mcq {
            choices: vec![AnswerChoice {
                id: "a".into(),
                text: "A".into(),
            }],
            correct_choice_id: "a".into(),
            randomize_choices: false,
        };
        assert!(validate_answer_config(&mcq).is_err());
    }

    #[test]
    fn rejects_short_text_with_empty_answers() {
        let cfg = AnswerConfig::Short {
            mode: ShortAnswerMode::Text,
            acceptable_answers: Some(vec![]),
            numeric_config: None,
        };
        assert!(validate_answer_config(&cfg).is_err());
    }

    #[test]
    fn rejects_short_numeric_with_negative_tolerance() {
        let cfg = AnswerConfig::Short {
            mode: ShortAnswerMode::Numeric,
            acceptable_answers: None,
            numeric_config: Some(NumericAnswerConfig {
                correct_value: 1.0,
                tolerance: -0.5,
                unit: None,
            }),
        };
        assert!(validate_answer_config(&cfg).is_err());
    }

    #[test]
    fn serde_round_trips_mcq_through_internal_type_tag() {
        let json = serde_json::json!({
            "type": "mcq",
            "choices": [{ "id": "a", "text": "A" }, { "id": "b", "text": "B" }],
            "correct_choice_id": "a",
            "randomize_choices": true
        });
        let parsed: AnswerConfig = serde_json::from_value(json.clone()).expect("parses valid mcq");
        assert!(validate_answer_config(&parsed).is_ok());
        let reparsed: AnswerConfig =
            serde_json::from_value(serde_json::to_value(&parsed).unwrap()).unwrap();
        assert!(matches!(reparsed, AnswerConfig::Mcq { .. }));
    }

    #[test]
    fn serde_parses_short_numeric() {
        let json = serde_json::json!({
            "type": "short",
            "mode": "numeric",
            "numeric_config": { "correct_value": 9.8, "tolerance": 0.1, "unit": "m/s²" }
        });
        let parsed: AnswerConfig = serde_json::from_value(json).expect("parses short numeric");
        assert!(validate_answer_config(&parsed).is_ok());
    }

    #[test]
    fn serde_rejects_structurally_invalid_answer_config() {
        // Legacy shape using `options` instead of `choices` — must be refused
        // at the write boundary so it can never be persisted.
        let legacy = serde_json::json!({
            "type": "mcq",
            "options": [{ "id": "a", "text": "A" }]
        });
        assert!(serde_json::from_value::<AnswerConfig>(legacy).is_err());

        let unknown = serde_json::json!({ "type": "bogus" });
        assert!(serde_json::from_value::<AnswerConfig>(unknown).is_err());
    }
}
