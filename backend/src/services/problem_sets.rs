use std::collections::HashMap;

use serde::Deserialize;
use serde_json::{Value, json};
use sqlx::{PgPool, Postgres, QueryBuilder, types::Json as SqlJson};
use uuid::Uuid;
use validator::Validate;

use crate::{
    dto::{
        patch::PatchField,
        problem::{ProblemDto, ProblemRow},
        problem_set::{
            ProblemSetDto, ProblemSetOwnerProfileDto, ProblemSetProblemDto, ProblemSetRow,
            ProblemSetShareDto,
        },
    },
    error::{AppError, AppResult, validation},
    models::{CopyProblemSetBody, CreateProblemSet, ReportProblemSetBody},
    services::{content_limits, problem_filters::ProblemFilter, problems as problem_service},
};

const PROBLEM_SET_ROW_SELECT: &str = r#"
        select
          ps.id,
          ps.user_id,
          ps.subject_id,
          ps.name,
          ps.description,
          ps.sharing_level,
          ps.is_smart,
          ps.filter_config,
          ps.session_config,
          ps.allow_copying,
          ps.is_listed,
          ps.discovery_subject,
          ps.created_at,
          ps.updated_at,
          s.name as subject_name,
          coalesce(pc.problem_count, 0)::bigint as problem_count
        from problem_sets ps
        join subjects s on s.id = ps.subject_id
        left join lateral (
          select count(*)::bigint as problem_count
          from problem_set_problems
          where problem_set_id = ps.id
        ) pc on true
"#;

#[derive(Debug, Deserialize)]
pub struct PatchProblemSet {
    #[serde(default)]
    name: PatchField<String>,
    #[serde(default)]
    description: PatchField<String>,
    #[serde(default)]
    sharing_level: PatchField<String>,
    #[serde(default)]
    shared_with_emails: PatchField<Vec<String>>,
    #[serde(default)]
    is_smart: PatchField<bool>,
    #[serde(default)]
    filter_config: PatchField<Value>,
    #[serde(default)]
    session_config: PatchField<Value>,
    #[serde(default)]
    allow_copying: PatchField<bool>,
    #[serde(default)]
    is_listed: PatchField<bool>,
    #[serde(default)]
    discovery_subject: PatchField<String>,
}

#[derive(Debug, sqlx::FromRow)]
struct ProblemSetFavouriteRow {
    problem_set_id: Uuid,
    favorited_at: chrono::DateTime<chrono::Utc>,
}

#[derive(Debug, sqlx::FromRow)]
struct ProblemSetLinkRow {
    problem_id: Uuid,
    added_at: chrono::DateTime<chrono::Utc>,
}

struct LinkedProblem {
    added_at: Option<chrono::DateTime<chrono::Utc>>,
    problem: ProblemDto,
}

pub async fn list_user_problem_sets(
    pool: &PgPool,
    user_id: Uuid,
    subject_id: Option<Uuid>,
) -> AppResult<Vec<ProblemSetDto>> {
    let mut qb = QueryBuilder::<Postgres>::new(PROBLEM_SET_ROW_SELECT);
    qb.push(" where ps.user_id = ");
    qb.push_bind(user_id);
    if let Some(subject_id) = subject_id {
        qb.push(" and ps.subject_id = ").push_bind(subject_id);
    }
    qb.push(" order by ps.created_at desc");
    let rows = qb.build_query_as::<ProblemSetRow>().fetch_all(pool).await?;
    Ok(rows.into_iter().map(ProblemSetDto::from_row).collect())
}

pub async fn create_problem_set(
    pool: &PgPool,
    user_id: Uuid,
    body: CreateProblemSet,
) -> AppResult<ProblemSetDto> {
    body.validate().map_err(|err| {
        validation(
            "Invalid request body",
            serde_json::to_value(err).unwrap_or_default(),
        )
    })?;
    if body.is_smart.unwrap_or(false) && body.filter_config.is_none() {
        return Err(AppError::BadRequest(
            "filter_config is required for smart problem sets".to_owned(),
        ));
    }
    let limit =
        content_limits::check_content_limit(pool, user_id, content_limits::PROBLEM_SETS, None)
            .await?;
    if !limit
        .get("allowed")
        .and_then(Value::as_bool)
        .unwrap_or(false)
    {
        return Err(AppError::Forbidden);
    }

    let set_id = sqlx::query_scalar::<_, Uuid>(
        r#"
        insert into problem_sets (
          user_id, subject_id, name, description, sharing_level, is_smart,
          filter_config, session_config, allow_copying
        )
        values (
          $1, $2, $3, $4, coalesce($5, 'private'), coalesce($6, false),
          $7, $8, coalesce($9, true)
        )
        returning id
        "#,
    )
    .bind(user_id)
    .bind(body.subject_id)
    .bind(body.name)
    .bind(body.description)
    .bind(body.sharing_level)
    .bind(body.is_smart)
    .bind(body.filter_config.map(SqlJson))
    .bind(body.session_config.map(SqlJson))
    .bind(body.allow_copying)
    .fetch_one(pool)
    .await?;

    if let Some(problem_ids) = body.problem_ids {
        insert_problem_set_links(pool, user_id, set_id, problem_ids).await?;
    }
    if let Some(emails) = body.shared_with_emails {
        replace_shares(pool, user_id, set_id, emails).await?;
    }
    fetch_problem_set_full(pool, set_id).await
}

pub async fn update_problem_set(
    pool: &PgPool,
    user_id: Uuid,
    id: Uuid,
    body: PatchProblemSet,
) -> AppResult<ProblemSetDto> {
    if body.is_smart.as_nullable_value() == Some(&true)
        && !matches!(&body.filter_config, PatchField::Value(_))
    {
        return Err(AppError::BadRequest(
            "Smart problem sets must include a valid filter_config".to_owned(),
        ));
    }
    if let Some(name) = body.name.as_nullable_value() {
        validate_string_len("name", name, 1, 50)?;
    }
    if let Some(sharing_level) = body.sharing_level.as_nullable_value() {
        validate_sharing_level(sharing_level)?;
    }

    let should_replace_shares = !matches!(&body.sharing_level, PatchField::Missing)
        || !matches!(&body.shared_with_emails, PatchField::Missing);
    let shared_with_emails = match body.shared_with_emails {
        PatchField::Value(emails) => emails,
        PatchField::Null | PatchField::Missing => Vec::new(),
    };

    let (name_present, name) = required_patch(body.name, "name")?;
    let (description_present, description) = body.description.into_nullable();
    let (sharing_level_present, sharing_level) =
        required_patch(body.sharing_level, "sharing_level")?;
    let (is_smart_present, is_smart) = required_patch(body.is_smart, "is_smart")?;
    let (filter_config_present, filter_config) = body.filter_config.into_nullable();
    let (session_config_present, session_config) = body.session_config.into_nullable();
    let (allow_copying_present, allow_copying) =
        required_patch(body.allow_copying, "allow_copying")?;
    let (is_listed_present, is_listed) = required_patch(body.is_listed, "is_listed")?;
    let (discovery_subject_present, discovery_subject) = body.discovery_subject.into_nullable();

    let updated_id = sqlx::query_scalar::<_, Uuid>(
        r#"
        update problem_sets
        set
          name = case when $3 then $4 else name end,
          description = case when $5 then $6 else description end,
          sharing_level = case when $7 then $8 else sharing_level end,
          is_smart = case when $9 then $10 else is_smart end,
          filter_config = case when $11 then $12 else filter_config end,
          session_config = case when $13 then $14 else session_config end,
          allow_copying = case when $15 then $16 else allow_copying end,
          is_listed = case when $17 then $18 else is_listed end,
          discovery_subject = case when $19 then $20 else discovery_subject end,
          updated_at = now()
        where id = $1 and user_id = $2
        returning id
        "#,
    )
    .bind(id)
    .bind(user_id)
    .bind(name_present)
    .bind(name)
    .bind(description_present)
    .bind(description)
    .bind(sharing_level_present)
    .bind(sharing_level)
    .bind(is_smart_present)
    .bind(is_smart)
    .bind(filter_config_present)
    .bind(filter_config.map(SqlJson))
    .bind(session_config_present)
    .bind(session_config.map(SqlJson))
    .bind(allow_copying_present)
    .bind(allow_copying)
    .bind(is_listed_present)
    .bind(is_listed)
    .bind(discovery_subject_present)
    .bind(discovery_subject)
    .fetch_optional(pool)
    .await?;
    if updated_id.is_none() {
        return Err(AppError::NotFound(
            "Problem set not found or access denied".to_owned(),
        ));
    }
    if should_replace_shares {
        replace_shares(pool, user_id, id, shared_with_emails).await?;
    }
    fetch_problem_set_full(pool, id).await
}

pub async fn delete_problem_set(pool: &PgPool, user_id: Uuid, id: Uuid) -> AppResult<()> {
    let affected = sqlx::query("delete from problem_sets where id = $1 and user_id = $2")
        .bind(id)
        .bind(user_id)
        .execute(pool)
        .await?
        .rows_affected();
    if affected == 0 {
        return Err(AppError::NotFound(
            "Problem set not found or access denied".to_owned(),
        ));
    }
    Ok(())
}

pub async fn list_problem_set_problems(
    pool: &PgPool,
    user_id: Uuid,
    id: Uuid,
) -> AppResult<Vec<ProblemSetProblemDto>> {
    ensure_problem_set_owner(pool, user_id, id).await?;
    problem_set_problem_rows(pool, id).await
}

pub async fn add_problems_to_set(
    pool: &PgPool,
    user_id: Uuid,
    id: Uuid,
    problem_ids: Vec<Uuid>,
) -> AppResult<u64> {
    ensure_problem_set_owner(pool, user_id, id).await?;
    insert_problem_set_links(pool, user_id, id, problem_ids).await
}

pub async fn remove_problems_from_set(
    pool: &PgPool,
    user_id: Uuid,
    id: Uuid,
    problem_ids: &[Uuid],
) -> AppResult<u64> {
    ensure_problem_set_owner(pool, user_id, id).await?;
    let affected = sqlx::query(
        r#"
        delete from problem_set_problems
        where problem_set_id = $1 and user_id = $2 and problem_id = any($3)
        "#,
    )
    .bind(id)
    .bind(user_id)
    .bind(problem_ids)
    .execute(pool)
    .await?
    .rows_affected();
    Ok(affected)
}

pub async fn report_problem_set(
    pool: &PgPool,
    reporter_id: Uuid,
    id: Uuid,
    body: ReportProblemSetBody,
) -> AppResult<()> {
    sqlx::query(
        r#"
        insert into problem_set_reports (problem_set_id, reported_by_user_id, reason, details)
        values ($1, $2, $3, $4)
        "#,
    )
    .bind(id)
    .bind(reporter_id)
    .bind(body.reason)
    .bind(body.details)
    .execute(pool)
    .await?;
    Ok(())
}

pub async fn problem_set_progress(
    pool: &PgPool,
    id: Uuid,
    user_id: Option<Uuid>,
) -> AppResult<Value> {
    let problems = problem_set_problem_rows(pool, id).await?;
    let total = problems.len() as i64;
    let completed = if let Some(user_id) = user_id {
        sqlx::query_scalar::<_, i64>(
            r#"
            select count(distinct a.problem_id)::bigint
            from attempts a
            join problem_set_problems psp on psp.problem_id = a.problem_id
            where psp.problem_set_id = $1 and a.user_id = $2
            "#,
        )
        .bind(id)
        .bind(user_id)
        .fetch_one(pool)
        .await?
    } else {
        0
    };
    Ok(json!({
        "total": total,
        "completed": completed,
        "remaining": (total - completed).max(0),
    }))
}

pub async fn list_favourites(pool: &PgPool, user_id: Uuid) -> AppResult<Vec<ProblemSetDto>> {
    let favourite_rows = sqlx::query_as::<_, ProblemSetFavouriteRow>(
        r#"
        select f.problem_set_id, f.created_at as favorited_at
        from problem_set_favourites f
        where f.user_id = $1
        order by f.created_at desc
        "#,
    )
    .bind(user_id)
    .fetch_all(pool)
    .await?;
    let ids = favourite_rows
        .iter()
        .map(|row| row.problem_set_id)
        .collect::<Vec<_>>();
    let mut problem_sets = fetch_problem_sets_by_ids(pool, &ids).await?;
    let rows = favourite_rows
        .into_iter()
        .filter_map(|favourite| {
            let mut problem_set = problem_sets.remove(&favourite.problem_set_id)?;
            problem_set.favorited_at = Some(favourite.favorited_at);
            problem_set.favourited_at = Some(favourite.favorited_at);
            Some(problem_set)
        })
        .collect();
    Ok(rows)
}

pub async fn copy_problem_set(
    pool: &PgPool,
    user_id: Uuid,
    id: Uuid,
    body: CopyProblemSetBody,
) -> AppResult<ProblemSetDto> {
    let source = fetch_problem_set_full(pool, id).await?;
    if !source.allow_copying {
        return Err(AppError::Forbidden);
    }
    let name = body
        .name
        .unwrap_or_else(|| format!("{} (Copy)", source.name));
    let new_set_id = sqlx::query_scalar::<_, Uuid>(
        r#"
        insert into problem_sets (user_id, subject_id, name, description, sharing_level)
        values ($1, $2, $3, $4, 'private')
        returning id
        "#,
    )
    .bind(user_id)
    .bind(body.target_subject_id)
    .bind(name)
    .bind(source.description.as_deref())
    .fetch_one(pool)
    .await?;
    let source_problem_ids = problem_ids_for_set(pool, id).await?;
    for problem_id in source_problem_ids {
        let copied = copy_problem_row(pool, user_id, body.target_subject_id, problem_id).await?;
        insert_problem_set_links(pool, user_id, new_set_id, vec![copied]).await?;
    }
    record_copy(pool, Some(id), None, user_id, Some(new_set_id), None).await?;
    fetch_problem_set_full(pool, new_set_id).await
}

pub async fn copy_problem(
    pool: &PgPool,
    user_id: Uuid,
    problem_set_id: Uuid,
    source_problem_id: Uuid,
    target_subject_id: Uuid,
) -> AppResult<ProblemDto> {
    let copied_id = copy_problem_row(pool, user_id, target_subject_id, source_problem_id).await?;
    record_copy(
        pool,
        Some(problem_set_id),
        Some(source_problem_id),
        user_id,
        None,
        Some(copied_id),
    )
    .await?;
    problem_service::fetch_problem(pool, user_id, copied_id).await
}

pub async fn can_view_problem_set(
    pool: &PgPool,
    problem_set_id: Uuid,
    user_id: Option<Uuid>,
    user_email: Option<&str>,
) -> AppResult<bool> {
    let row = sqlx::query_as::<_, (Uuid, String)>(
        "select user_id, sharing_level from problem_sets where id = $1",
    )
    .bind(problem_set_id)
    .fetch_optional(pool)
    .await?;
    let Some((owner_id, sharing_level)) = row else {
        return Ok(false);
    };
    if user_id == Some(owner_id) || sharing_level == "public" {
        return Ok(true);
    }
    if sharing_level == "limited"
        && let Some(email) = user_email
    {
        let shared = sqlx::query_scalar::<_, bool>(
            r#"
                select exists(
                  select 1 from problem_set_shares
                  where problem_set_id = $1 and lower(shared_with_email) = lower($2)
                )
                "#,
        )
        .bind(problem_set_id)
        .bind(email)
        .fetch_one(pool)
        .await?;
        return Ok(shared);
    }
    Ok(false)
}

pub async fn record_problem_set_view(
    pool: &PgPool,
    problem_set_id: Uuid,
    viewer_user_id: Option<Uuid>,
    ip_hash: Option<&str>,
) -> AppResult<()> {
    sqlx::query(
        r#"
        insert into problem_set_views (problem_set_id, viewer_user_id, ip_hash)
        values ($1, $2, $3)
        on conflict (
          problem_set_id,
          (coalesce(viewer_user_id, '00000000-0000-0000-0000-000000000000'::uuid)),
          (coalesce(ip_hash, ''))
        )
        do nothing
        "#,
    )
    .bind(problem_set_id)
    .bind(viewer_user_id)
    .bind(ip_hash)
    .execute(pool)
    .await?;
    Ok(())
}

pub async fn toggle_like(pool: &PgPool, problem_set_id: Uuid, user_id: Uuid) -> AppResult<Value> {
    let mut tx = pool.begin().await?;
    let deleted = sqlx::query_scalar::<_, Option<Uuid>>(
        r#"
        delete from problem_set_likes
        where problem_set_id = $1 and user_id = $2
        returning id
        "#,
    )
    .bind(problem_set_id)
    .bind(user_id)
    .fetch_optional(&mut *tx)
    .await?
    .flatten();

    let liked = if deleted.is_some() {
        false
    } else {
        sqlx::query(
            r#"
            insert into problem_set_likes (problem_set_id, user_id)
            values ($1, $2)
            on conflict (problem_set_id, user_id) do nothing
            "#,
        )
        .bind(problem_set_id)
        .bind(user_id)
        .execute(&mut *tx)
        .await?;
        true
    };

    let count = sqlx::query_scalar::<_, i64>(
        "select count(*)::bigint from problem_set_likes where problem_set_id = $1",
    )
    .bind(problem_set_id)
    .fetch_one(&mut *tx)
    .await?;
    tx.commit().await?;
    Ok(json!({ "liked": liked, "like_count": count }))
}

pub async fn toggle_favourite(
    pool: &PgPool,
    problem_set_id: Uuid,
    user_id: Uuid,
) -> AppResult<Value> {
    let mut tx = pool.begin().await?;
    let deleted = sqlx::query_scalar::<_, Option<Uuid>>(
        r#"
        delete from problem_set_favourites
        where problem_set_id = $1 and user_id = $2
        returning id
        "#,
    )
    .bind(problem_set_id)
    .bind(user_id)
    .fetch_optional(&mut *tx)
    .await?
    .flatten();

    let favourited = if deleted.is_some() {
        false
    } else {
        sqlx::query(
            r#"
            insert into problem_set_favourites (problem_set_id, user_id)
            values ($1, $2)
            on conflict (problem_set_id, user_id) do nothing
            "#,
        )
        .bind(problem_set_id)
        .bind(user_id)
        .execute(&mut *tx)
        .await?;
        true
    };
    tx.commit().await?;
    Ok(json!({ "favourited": favourited, "favorited": favourited }))
}

pub async fn record_copy(
    pool: &PgPool,
    source_problem_set_id: Option<Uuid>,
    source_problem_id: Option<Uuid>,
    copied_by_user_id: Uuid,
    new_problem_set_id: Option<Uuid>,
    new_problem_id: Option<Uuid>,
) -> AppResult<()> {
    sqlx::query(
        r#"
        insert into problem_set_copies (
          source_problem_set_id, source_problem_id, copied_by_user_id,
          new_problem_set_id, new_problem_id
        )
        values ($1, $2, $3, $4, $5)
        "#,
    )
    .bind(source_problem_set_id)
    .bind(source_problem_id)
    .bind(copied_by_user_id)
    .bind(new_problem_set_id)
    .bind(new_problem_id)
    .execute(pool)
    .await?;
    Ok(())
}

pub async fn stats(pool: &PgPool, problem_set_id: Uuid, user_id: Option<Uuid>) -> AppResult<Value> {
    let (view_count, like_count, favourite_count, copy_count) =
        sqlx::query_as::<_, (i64, i64, i64, i64)>(
            r#"
        select
          (select count(*)::bigint from problem_set_views where problem_set_id = $1),
          (select count(*)::bigint from problem_set_likes where problem_set_id = $1),
          (select count(*)::bigint from problem_set_favourites where problem_set_id = $1),
          (select count(*)::bigint from problem_set_copies where source_problem_set_id = $1)
        "#,
        )
        .bind(problem_set_id)
        .fetch_one(pool)
        .await?;

    let (liked, favourited) = if let Some(user_id) = user_id {
        let liked = sqlx::query_scalar::<_, bool>(
            "select exists(select 1 from problem_set_likes where problem_set_id = $1 and user_id = $2)",
        )
        .bind(problem_set_id)
        .bind(user_id)
        .fetch_one(pool)
        .await?;
        let favourited = sqlx::query_scalar::<_, bool>(
            "select exists(select 1 from problem_set_favourites where problem_set_id = $1 and user_id = $2)",
        )
        .bind(problem_set_id)
        .bind(user_id)
        .fetch_one(pool)
        .await?;
        (liked, favourited)
    } else {
        (false, false)
    };

    Ok(json!({
        "view_count": view_count,
        "like_count": like_count,
        "favourite_count": favourite_count,
        "favorite_count": favourite_count,
        "copy_count": copy_count,
        "liked": liked,
        "favourited": favourited,
        "favorited": favourited,
    }))
}

async fn ensure_problem_set_owner(pool: &PgPool, user_id: Uuid, id: Uuid) -> AppResult<()> {
    let exists = sqlx::query_scalar::<_, bool>(
        "select exists(select 1 from problem_sets where id = $1 and user_id = $2)",
    )
    .bind(id)
    .bind(user_id)
    .fetch_one(pool)
    .await?;
    if exists {
        Ok(())
    } else {
        Err(AppError::NotFound(
            "Problem set not found or access denied".to_owned(),
        ))
    }
}

pub async fn fetch_problem_set_full(pool: &PgPool, id: Uuid) -> AppResult<ProblemSetDto> {
    let mut data = fetch_problem_set_base(pool, id).await?;
    let problems = problem_set_problem_rows(pool, id).await?;
    data.problem_count = problems.len() as i64;
    data.shared_with_emails = Some(fetch_shared_emails(pool, id).await?);
    data.problem_set_shares = Some(fetch_problem_set_shares(pool, id).await?);
    data.owner_profile = fetch_owner_profile(pool, data.user_id).await?;
    data.problems = Some(problems);
    Ok(data)
}

async fn fetch_problem_set_base(pool: &PgPool, id: Uuid) -> AppResult<ProblemSetDto> {
    let sql = format!("{PROBLEM_SET_ROW_SELECT} where ps.id = $1");
    let row = sqlx::query_as::<_, ProblemSetRow>(&sql)
        .bind(id)
        .fetch_optional(pool)
        .await?
        .ok_or_else(|| AppError::NotFound("Problem set not found or access denied".to_owned()))?;
    Ok(ProblemSetDto::from_row(row))
}

async fn fetch_problem_sets_by_ids(
    pool: &PgPool,
    ids: &[Uuid],
) -> AppResult<HashMap<Uuid, ProblemSetDto>> {
    if ids.is_empty() {
        return Ok(HashMap::new());
    }

    let mut qb = QueryBuilder::<Postgres>::new(PROBLEM_SET_ROW_SELECT);
    qb.push(" where ps.id = any(").push_bind(ids).push(")");
    let rows = qb.build_query_as::<ProblemSetRow>().fetch_all(pool).await?;
    Ok(rows
        .into_iter()
        .map(|row| {
            let problem_set = ProblemSetDto::from_row(row);
            (problem_set.id, problem_set)
        })
        .collect())
}

async fn fetch_shared_emails(pool: &PgPool, id: Uuid) -> AppResult<Vec<String>> {
    Ok(sqlx::query_scalar::<_, String>(
        r#"
        select shared_with_email
        from problem_set_shares
        where problem_set_id = $1
        order by shared_with_email
        "#,
    )
    .bind(id)
    .fetch_all(pool)
    .await?)
}

async fn fetch_problem_set_shares(pool: &PgPool, id: Uuid) -> AppResult<Vec<ProblemSetShareDto>> {
    Ok(sqlx::query_as::<_, ProblemSetShareDto>(
        r#"
        select id, shared_with_email, created_at
        from problem_set_shares
        where problem_set_id = $1
        order by shared_with_email
        "#,
    )
    .bind(id)
    .fetch_all(pool)
    .await?)
}

async fn fetch_owner_profile(
    pool: &PgPool,
    user_id: Uuid,
) -> AppResult<Option<ProblemSetOwnerProfileDto>> {
    Ok(sqlx::query_as::<_, ProblemSetOwnerProfileDto>(
        r#"
        select id, username, first_name, last_name, avatar_url, bio, gender
        from user_profiles
        where id = $1
        "#,
    )
    .bind(user_id)
    .fetch_optional(pool)
    .await?)
}

async fn problem_set_problem_rows(pool: &PgPool, id: Uuid) -> AppResult<Vec<ProblemSetProblemDto>> {
    Ok(linked_problems_for_set(pool, id)
        .await?
        .into_iter()
        .map(|linked| ProblemSetProblemDto {
            problem_id: linked.problem.id,
            added_at: linked.added_at,
            problems: linked.problem,
        })
        .collect())
}

async fn linked_problems_for_set(pool: &PgPool, id: Uuid) -> AppResult<Vec<LinkedProblem>> {
    let set = problem_set_filter_context(pool, id).await?;
    if set.is_smart {
        return Ok(
            smart_problem_rows(pool, set.user_id, set.subject_id, set.filter_config)
                .await?
                .into_iter()
                .map(|problem| LinkedProblem {
                    added_at: None,
                    problem,
                })
                .collect(),
        );
    }

    let links = sqlx::query_as::<_, ProblemSetLinkRow>(
        r#"
        select psp.problem_id, psp.added_at
        from problem_set_problems psp
        where psp.problem_set_id = $1
        order by psp.added_at asc
        "#,
    )
    .bind(id)
    .fetch_all(pool)
    .await?;
    let problem_ids = links.iter().map(|link| link.problem_id).collect::<Vec<_>>();
    let mut problems_by_id =
        problem_service::fetch_problems_by_ids(pool, set.user_id, &problem_ids).await?;
    Ok(links
        .into_iter()
        .filter_map(|link| {
            let problem = problems_by_id.remove(&link.problem_id)?;
            Some(LinkedProblem {
                added_at: Some(link.added_at),
                problem,
            })
        })
        .collect())
}

pub async fn problem_ids_for_set(pool: &PgPool, id: Uuid) -> AppResult<Vec<Uuid>> {
    let set = problem_set_filter_context(pool, id).await?;
    if set.is_smart {
        return smart_problem_ids(pool, set.user_id, set.subject_id, set.filter_config).await;
    }

    Ok(sqlx::query_scalar::<_, Uuid>(
        "select problem_id from problem_set_problems where problem_set_id = $1 order by added_at",
    )
    .bind(id)
    .fetch_all(pool)
    .await?)
}

struct ProblemSetFilterContext {
    user_id: Uuid,
    subject_id: Uuid,
    is_smart: bool,
    filter_config: Option<Value>,
}

async fn problem_set_filter_context(pool: &PgPool, id: Uuid) -> AppResult<ProblemSetFilterContext> {
    let row = sqlx::query_as::<_, (Uuid, Uuid, bool, Option<SqlJson<Value>>)>(
        "select user_id, subject_id, is_smart, filter_config from problem_sets where id = $1",
    )
    .bind(id)
    .fetch_optional(pool)
    .await?
    .ok_or_else(|| AppError::NotFound("Problem set not found or access denied".to_owned()))?;

    Ok(ProblemSetFilterContext {
        user_id: row.0,
        subject_id: row.1,
        is_smart: row.2,
        filter_config: row.3.map(|SqlJson(v)| v),
    })
}

async fn smart_problem_rows(
    pool: &PgPool,
    user_id: Uuid,
    subject_id: Uuid,
    filter_config: Option<Value>,
) -> AppResult<Vec<ProblemDto>> {
    let filter_value = filter_config.unwrap_or_else(|| json!({}));
    let filter = ProblemFilter::from_value(Some(subject_id), &filter_value)?;
    let mut qb = QueryBuilder::<Postgres>::new(problem_service::PROBLEM_ROW_SELECT);
    qb.push(" where p.user_id = ");
    qb.push_bind(user_id);
    filter.push_sql(&mut qb, user_id);
    qb.push(" order by p.created_at desc");

    let rows = qb.build_query_as::<ProblemRow>().fetch_all(pool).await?;
    problem_service::problem_dtos_from_rows(pool, user_id, rows).await
}

async fn smart_problem_ids(
    pool: &PgPool,
    user_id: Uuid,
    subject_id: Uuid,
    filter_config: Option<Value>,
) -> AppResult<Vec<Uuid>> {
    let filter_value = filter_config.unwrap_or_else(|| json!({}));
    let filter = ProblemFilter::from_value(Some(subject_id), &filter_value)?;
    let mut qb = QueryBuilder::<Postgres>::new("select p.id from problems p where p.user_id = ");
    qb.push_bind(user_id);
    filter.push_sql(&mut qb, user_id);
    qb.push(" order by p.created_at desc");
    Ok(qb.build_query_scalar::<Uuid>().fetch_all(pool).await?)
}

async fn insert_problem_set_links(
    pool: &PgPool,
    user_id: Uuid,
    set_id: Uuid,
    problem_ids: Vec<Uuid>,
) -> AppResult<u64> {
    let mut affected = 0;
    for problem_id in problem_ids {
        affected += sqlx::query(
            r#"
            insert into problem_set_problems (problem_set_id, problem_id, user_id)
            values ($1, $2, $3)
            on conflict do nothing
            "#,
        )
        .bind(set_id)
        .bind(problem_id)
        .bind(user_id)
        .execute(pool)
        .await?
        .rows_affected();
    }
    Ok(affected)
}

async fn replace_shares(
    pool: &PgPool,
    user_id: Uuid,
    set_id: Uuid,
    emails: Vec<String>,
) -> AppResult<()> {
    let mut tx = pool.begin().await?;
    sqlx::query(
        "delete from problem_set_shares where problem_set_id = $1 and shared_by_user_id = $2",
    )
    .bind(set_id)
    .bind(user_id)
    .execute(&mut *tx)
    .await?;
    for email in emails {
        sqlx::query(
            r#"
            insert into problem_set_shares (problem_set_id, shared_with_email, shared_by_user_id)
            values ($1, $2, $3)
            on conflict do nothing
            "#,
        )
        .bind(set_id)
        .bind(email)
        .bind(user_id)
        .execute(&mut *tx)
        .await?;
    }
    tx.commit().await?;
    Ok(())
}

async fn copy_problem_row(
    pool: &PgPool,
    user_id: Uuid,
    target_subject_id: Uuid,
    source_problem_id: Uuid,
) -> AppResult<Uuid> {
    let id = sqlx::query_scalar::<_, Uuid>(
        r#"
        insert into problems (
          user_id, subject_id, title, content, problem_type, correct_answer,
          answer_config, auto_mark, status, assets, solution_text, solution_assets
        )
        select
          $1, $2, title, content, problem_type, correct_answer, answer_config,
          auto_mark, status, assets, solution_text, solution_assets
        from problems where id = $3
        returning id
        "#,
    )
    .bind(user_id)
    .bind(target_subject_id)
    .bind(source_problem_id)
    .fetch_one(pool)
    .await?;
    Ok(id)
}

fn required_patch<T>(field: PatchField<T>, field_name: &str) -> AppResult<(bool, Option<T>)> {
    match field {
        PatchField::Missing => Ok((false, None)),
        PatchField::Null => Err(AppError::BadRequest(format!("{field_name} cannot be null"))),
        PatchField::Value(value) => Ok((true, Some(value))),
    }
}

fn validate_string_len(field_name: &str, value: &str, min: usize, max: usize) -> AppResult<()> {
    let len = value.chars().count();
    if len < min || len > max {
        return Err(validation(
            "Invalid request body",
            json!({ field_name: format!("length must be between {min} and {max}") }),
        ));
    }
    Ok(())
}

fn validate_sharing_level(value: &str) -> AppResult<()> {
    if matches!(value, "private" | "limited" | "public") {
        Ok(())
    } else {
        Err(AppError::BadRequest("Invalid sharing_level".to_owned()))
    }
}
