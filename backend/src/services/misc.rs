use chrono::{DateTime, Days, NaiveDate, Utc};
use serde::Deserialize;
use serde_json::{Value, json};
use sqlx::{Postgres, QueryBuilder, types::Json as SqlJson};
use uuid::Uuid;

use crate::{
    dto::statistics::{
        ActivityDay, RecentStudyActivity, SessionStatisticsSummary, StatisticsOverview,
        StatisticsResponse, StudyStreaks, SubjectBreakdownRow, WeeklyProgressPoint,
    },
    error::{AppError, AppResult},
    services::insights,
    state::AppState,
};

#[derive(Debug, Deserialize)]
pub struct DiscoverQuery {
    q: Option<String>,
    subject: Option<String>,
    limit: Option<i64>,
    cursor: Option<String>,
    sort: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum DiscoverSort {
    Ranking,
    Newest,
    MostLiked,
    MostCopied,
}

impl DiscoverSort {
    fn parse(value: Option<&str>) -> AppResult<Self> {
        match value.unwrap_or("ranking") {
            "ranking" => Ok(Self::Ranking),
            "newest" => Ok(Self::Newest),
            "most_liked" => Ok(Self::MostLiked),
            "most_copied" => Ok(Self::MostCopied),
            _ => Err(AppError::BadRequest("Invalid discover sort".to_owned())),
        }
    }

    fn as_str(self) -> &'static str {
        match self {
            Self::Ranking => "ranking",
            Self::Newest => "newest",
            Self::MostLiked => "most_liked",
            Self::MostCopied => "most_copied",
        }
    }

    fn order_clause(self) -> &'static str {
        match self {
            Self::Ranking => " order by d.ranking_score desc, d.id desc limit ",
            Self::Newest => " order by d.created_at desc, d.id desc limit ",
            Self::MostLiked => " order by d.like_count desc, d.id desc limit ",
            Self::MostCopied => " order by d.copy_count desc, d.id desc limit ",
        }
    }

    fn metric_column(self) -> Option<&'static str> {
        match self {
            Self::Ranking => Some("d.ranking_score"),
            Self::Newest => None,
            Self::MostLiked => Some("d.like_count"),
            Self::MostCopied => Some("d.copy_count"),
        }
    }

    fn cursor_field(self) -> &'static str {
        match self {
            Self::Ranking => "ranking_score",
            Self::Newest => "created_at",
            Self::MostLiked => "like_count",
            Self::MostCopied => "copy_count",
        }
    }
}

#[derive(Debug)]
struct DiscoverCursor {
    value: DiscoverCursorValue,
    id: Uuid,
}

#[derive(Debug)]
enum DiscoverCursorValue {
    Timestamp(DateTime<Utc>),
    Count(i64),
}

pub async fn announcement(pool: &sqlx::PgPool) -> AppResult<Value> {
    Ok(sqlx::query_scalar::<_, Option<SqlJson<Value>>>(
        "select value from admin_settings where key = 'announcement'",
    )
    .fetch_optional(pool)
    .await?
    .flatten()
    .map(|SqlJson(v)| v)
    .unwrap_or_else(|| json!({ "enabled": false })))
}

/// Whether self-service sign-up is currently allowed. Reads the
/// `user_registration` admin setting; defaults to `true` when the row is
/// absent or malformed, so registration stays open by default and the
/// toggle only takes effect once an admin explicitly disables it.
pub async fn registration_enabled(pool: &sqlx::PgPool) -> AppResult<bool> {
    let row: Option<SqlJson<Value>> =
        sqlx::query_scalar("select value from admin_settings where key = 'user_registration'")
            .fetch_optional(pool)
            .await?;
    Ok(row
        .and_then(|SqlJson(v)| v.get("enabled").cloned())
        .and_then(|v| v.as_bool())
        .unwrap_or(true))
}

pub async fn discover(pool: &sqlx::PgPool, query: DiscoverQuery) -> AppResult<Value> {
    let limit = query.limit.unwrap_or(20).clamp(1, 100);
    let fetch_limit = limit + 1;
    let sort = DiscoverSort::parse(query.sort.as_deref())?;
    let cursor = parse_discover_cursor(query.cursor.as_deref(), sort)?;
    let mut qb = QueryBuilder::<Postgres>::new(
        r#"
        with discoverable as (
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
            coalesce(s.name, 'Unknown') as subject_name,
            s.color as subject_color,
            s.icon as subject_icon,
            up.username as owner_username,
            up.avatar_url as owner_avatar_url,
            coalesce(nullif(concat_ws(' ', up.first_name, up.last_name), ''), up.username, 'Anonymous') as owner_display_name,
            coalesce(problem_counts.problem_count, 0)::bigint as problem_count,
            coalesce(view_stats.view_count, 0)::bigint as view_count,
            coalesce(view_stats.unique_view_count, 0)::bigint as unique_view_count,
            coalesce(like_stats.like_count, 0)::bigint as like_count,
            coalesce(copy_stats.copy_count, 0)::bigint as copy_count,
            (
              coalesce(like_stats.like_count, 0) * 3
              + coalesce(copy_stats.copy_count, 0) * 5
              + coalesce(view_stats.view_count, 0)
            )::bigint as ranking_score
        from problem_sets ps
        left join subjects s on s.id = ps.subject_id
        left join user_profiles up on up.id = ps.user_id
        left join lateral (
          select count(*)::bigint as problem_count
          from problem_set_problems
          where problem_set_id = ps.id
        ) problem_counts on true
        left join lateral (
          select count(*)::bigint as view_count, count(*)::bigint as unique_view_count
          from problem_set_views
          where problem_set_id = ps.id
        ) view_stats on true
        left join lateral (
          select count(*)::bigint as like_count
          from problem_set_likes
          where problem_set_id = ps.id
        ) like_stats on true
        left join lateral (
          select count(*)::bigint as copy_count
          from problem_set_copies
          where source_problem_set_id = ps.id
        ) copy_stats on true
        where ps.sharing_level = 'public' and coalesce(ps.is_listed, true) = true
        "#,
    );
    if let Some(subject) = query.subject.as_deref().filter(|s| !s.is_empty()) {
        qb.push(" and ps.discovery_subject = ").push_bind(subject);
    }
    if let Some(search) = query.q.as_deref().filter(|s| !s.is_empty()) {
        let pattern = format!("%{search}%");
        qb.push(" and (ps.name ilike ")
            .push_bind(pattern.clone())
            .push(" or ps.description ilike ")
            .push_bind(pattern)
            .push(")");
    }

    qb.push(
        r#"
        )
        select jsonb_build_object(
          'id', d.id,
          'user_id', d.user_id,
          'subject_id', d.subject_id,
          'name', d.name,
          'description', d.description,
          'sharing_level', d.sharing_level,
          'is_smart', d.is_smart,
          'filter_config', d.filter_config,
          'session_config', d.session_config,
          'allow_copying', d.allow_copying,
          'is_listed', d.is_listed,
          'discovery_subject', d.discovery_subject,
          'subject_name', d.subject_name,
          'subject_color', d.subject_color,
          'subject_icon', d.subject_icon,
          'owner_username', d.owner_username,
          'owner_avatar_url', d.owner_avatar_url,
          'problem_count', d.problem_count,
          'like_count', d.like_count,
          'view_count', d.view_count,
          'copy_count', d.copy_count,
          'ranking_score', d.ranking_score,
          'owner', jsonb_build_object(
            'username', d.owner_username,
            'display_name', d.owner_display_name,
            'avatar_url', d.owner_avatar_url
          ),
          'stats', jsonb_build_object(
            'view_count', d.view_count,
            'unique_view_count', d.unique_view_count,
            'like_count', d.like_count,
            'copy_count', d.copy_count,
            'problem_count', d.problem_count,
            'ranking_score', d.ranking_score
          ),
          'created_at', d.created_at,
          'updated_at', d.updated_at
        ) as data
        from discoverable d
        where true
        "#,
    );

    if let Some(cursor) = &cursor {
        match &cursor.value {
            DiscoverCursorValue::Timestamp(created_at) => {
                qb.push(" and (d.created_at, d.id) < (")
                    .push_bind(created_at.to_owned())
                    .push(", ")
                    .push_bind(cursor.id)
                    .push(")");
            }
            DiscoverCursorValue::Count(count) => {
                let metric = sort
                    .metric_column()
                    .expect("count cursors are only used with metric sorts");
                qb.push(" and (")
                    .push(metric)
                    .push(" < ")
                    .push_bind(*count)
                    .push(" or (")
                    .push(metric)
                    .push(" = ")
                    .push_bind(*count)
                    .push(" and d.id < ")
                    .push_bind(cursor.id)
                    .push("))");
            }
        }
    }

    qb.push(sort.order_clause()).push_bind(fetch_limit);

    let mut items = qb
        .build_query_scalar::<SqlJson<Value>>()
        .fetch_all(pool)
        .await?
        .into_iter()
        .map(|SqlJson(v)| v)
        .collect::<Vec<_>>();

    let has_next = items.len() as i64 > limit;
    if has_next {
        items.truncate(limit as usize);
    }
    let next_cursor = if has_next {
        items
            .last()
            .map(|item| encode_discover_cursor(sort, item))
            .transpose()?
    } else {
        None
    };

    let data = items.clone();
    Ok(json!({
        "items": items,
        "data": data,
        "next_cursor": next_cursor,
        "cursor": query.cursor,
        "sort": sort.as_str(),
    }))
}

fn parse_discover_cursor(
    raw: Option<&str>,
    sort: DiscoverSort,
) -> AppResult<Option<DiscoverCursor>> {
    let Some(raw) = raw.filter(|value| !value.is_empty()) else {
        return Ok(None);
    };
    let separator = raw
        .rfind(':')
        .ok_or_else(|| AppError::BadRequest("Invalid discover cursor".to_owned()))?;
    let (value, id) = raw.split_at(separator);
    let id = id
        .strip_prefix(':')
        .ok_or_else(|| AppError::BadRequest("Invalid discover cursor".to_owned()))?;
    if value.is_empty() || id.is_empty() {
        return Err(AppError::BadRequest("Invalid discover cursor".to_owned()));
    }
    let id = Uuid::parse_str(id)
        .map_err(|_| AppError::BadRequest("Invalid discover cursor".to_owned()))?;
    let value = match sort {
        DiscoverSort::Newest => DiscoverCursorValue::Timestamp(
            DateTime::parse_from_rfc3339(value)
                .map_err(|_| AppError::BadRequest("Invalid discover cursor".to_owned()))?
                .with_timezone(&Utc),
        ),
        DiscoverSort::Ranking | DiscoverSort::MostLiked | DiscoverSort::MostCopied => {
            let count = value
                .parse::<i64>()
                .map_err(|_| AppError::BadRequest("Invalid discover cursor".to_owned()))?;
            if count < 0 {
                return Err(AppError::BadRequest("Invalid discover cursor".to_owned()));
            }
            DiscoverCursorValue::Count(count)
        }
    };
    Ok(Some(DiscoverCursor { value, id }))
}

fn encode_discover_cursor(sort: DiscoverSort, item: &Value) -> AppResult<String> {
    let id = item
        .get("id")
        .and_then(Value::as_str)
        .ok_or_else(|| AppError::Internal("Discover item missing id".to_owned()))?;
    let value = match sort {
        DiscoverSort::Newest => item
            .get(sort.cursor_field())
            .and_then(Value::as_str)
            .map(str::to_owned)
            .ok_or_else(|| AppError::Internal("Discover item missing cursor value".to_owned()))?,
        DiscoverSort::Ranking | DiscoverSort::MostLiked | DiscoverSort::MostCopied => item
            .get(sort.cursor_field())
            .and_then(Value::as_i64)
            .map(|value| value.to_string())
            .ok_or_else(|| AppError::Internal("Discover item missing cursor value".to_owned()))?,
    };
    Ok(format!("{value}:{id}"))
}

pub async fn statistics(pool: &sqlx::PgPool, user_id: Uuid) -> AppResult<StatisticsResponse> {
    let (total, mastered, needs_review, wrong) = sqlx::query_as::<_, (i64, i64, i64, i64)>(
        r#"
        select
          count(*)::bigint,
          count(*) filter (where status = 'mastered')::bigint,
          count(*) filter (where status = 'needs_review')::bigint,
          count(*) filter (where status = 'wrong')::bigint
        from problems
        where user_id = $1
        "#,
    )
    .bind(user_id)
    .fetch_one(pool)
    .await?;
    let mastery_rate = if total > 0 {
        (mastered as f64 / total as f64) * 100.0
    } else {
        0.0
    };

    let timezone =
        sqlx::query_scalar::<_, Option<String>>("select timezone from user_profiles where id = $1")
            .bind(user_id)
            .fetch_optional(pool)
            .await?
            .flatten()
            .unwrap_or_else(|| "UTC".to_owned());

    let subject_breakdown = sqlx::query_as::<_, SubjectBreakdownRow>(
        r#"
        select
          s.id as subject_id,
          s.name as subject_name,
          count(p.id)::bigint as total,
          count(p.id) filter (where p.status = 'mastered')::bigint as mastered,
          count(p.id) filter (where p.status = 'needs_review')::bigint as needs_review,
          count(p.id) filter (where p.status = 'wrong')::bigint as wrong,
          (
            case when count(p.id) = 0 then 0
                 else round((count(p.id) filter (where p.status = 'mastered'))::numeric * 100 / count(p.id), 2)
            end
          )::double precision as mastery_pct
        from subjects s
        left join problems p on p.subject_id = s.id and p.user_id = s.user_id
        where s.user_id = $1
        group by s.id
        order by s.name asc
        "#,
    )
    .bind(user_id)
    .fetch_all(pool)
    .await?;

    let weekly_progress = sqlx::query_as::<_, WeeklyProgressPoint>(
        r#"
        with weeks as (
          select date_trunc('week', coalesce(last_reviewed_date, updated_at))::date as week_start,
                 count(*) filter (where status = 'mastered')::bigint as mastered_count
          from problems
          where user_id = $1
          group by 1
        )
        select
          week_start,
          (sum(mastered_count) over (order by week_start))::bigint as cumulative_mastered
        from weeks
        order by week_start asc
        "#,
    )
    .bind(user_id)
    .fetch_all(pool)
    .await?;

    let activity_heatmap = sqlx::query_as::<_, ActivityDay>(
        r#"
        with selected_tz as (
          select case
            when exists(select 1 from pg_timezone_names where name = $2) then $2
            else 'UTC'
          end as name
        ),
        activity as (
          select timezone((select name from selected_tz), created_at)::date as activity_date
          from attempts
          where user_id = $1
        )
        select
          activity_date,
          count(*)::bigint as activity_count
        from activity
        group by activity_date
        order by activity_date asc
        "#,
    )
    .bind(user_id)
    .bind(&timezone)
    .fetch_all(pool)
    .await?;

    let recent_activity = sqlx::query_as::<_, RecentStudyActivity>(
        r#"
        select
          p.id as problem_id,
          p.title as problem_title,
          s.name as subject_name,
          null::text as old_status,
          coalesce(a.selected_status, p.status) as new_status,
          a.created_at as changed_at
        from attempts a
        join problems p on p.id = a.problem_id
        join subjects s on s.id = p.subject_id
        where a.user_id = $1
        order by a.created_at desc
        limit 20
        "#,
    )
    .bind(user_id)
    .fetch_all(pool)
    .await?;

    let (current_streak, longest_streak) = study_streaks(pool, user_id, &timezone).await?;
    let session_stats = session_statistics(pool, user_id).await?;

    Ok(StatisticsResponse {
        overview: StatisticsOverview {
            total_problems: total,
            mastered_count: mastered,
            needs_review_count: needs_review,
            wrong_count: wrong,
            mastery_rate,
        },
        streaks: StudyStreaks {
            current_streak,
            longest_streak,
        },
        session_stats: SessionStatisticsSummary {
            total_sessions: session_stats.total_sessions,
            avg_duration_ms: session_stats.avg_duration_ms,
            avg_problems_per_session: session_stats.avg_problems_per_session(),
            total_review_time_ms: session_stats.total_review_time_ms,
        },
        subject_breakdown,
        weekly_progress,
        activity_heatmap,
        recent_activity,
        timezone,
    })
}

struct SessionStatistics {
    total_sessions: i64,
    total_completed: i64,
    avg_duration_ms: i64,
    total_review_time_ms: i64,
}

impl SessionStatistics {
    fn avg_problems_per_session(&self) -> f64 {
        if self.total_sessions > 0 {
            self.total_completed as f64 / self.total_sessions as f64
        } else {
            0.0
        }
    }
}

async fn session_statistics(pool: &sqlx::PgPool, user_id: Uuid) -> AppResult<SessionStatistics> {
    let (total_sessions, total_completed, avg_duration_ms, total_review_time_ms) =
        sqlx::query_as::<_, (i64, i64, i64, i64)>(
            r#"
            with sessions as (
              select
                case
                  when jsonb_typeof(session_state->'completed_problem_ids') = 'array'
                    then jsonb_array_length(session_state->'completed_problem_ids')::bigint
                  else 0
                end as completed_count,
                case
                  when jsonb_typeof(session_state->'elapsed_ms') = 'number'
                    then greatest((session_state->>'elapsed_ms')::bigint, 0)
                  else 0
                end as elapsed_ms
              from review_session_state
              where user_id = $1
            )
            select
              count(*)::bigint,
              coalesce(sum(completed_count), 0)::bigint,
              coalesce(round(avg(elapsed_ms))::bigint, 0)::bigint,
              coalesce(sum(elapsed_ms), 0)::bigint
            from sessions
            "#,
        )
        .bind(user_id)
        .fetch_one(pool)
        .await?;

    Ok(SessionStatistics {
        total_sessions,
        total_completed,
        avg_duration_ms,
        total_review_time_ms,
    })
}

async fn study_streaks(
    pool: &sqlx::PgPool,
    user_id: Uuid,
    timezone: &str,
) -> AppResult<(i64, i64)> {
    let (today, activity_dates) = sqlx::query_as::<_, (NaiveDate, Vec<NaiveDate>)>(
        r#"
        with selected_tz as (
          select case
            when exists(select 1 from pg_timezone_names where name = $2) then $2
            else 'UTC'
          end as name
        ),
        activity_dates as (
          select timezone((select name from selected_tz), created_at)::date as activity_date
          from problems
          where user_id = $1
          union
          select timezone((select name from selected_tz), updated_at)::date as activity_date
          from problems
          where user_id = $1
          union
          select timezone((select name from selected_tz), last_reviewed_date)::date as activity_date
          from problems
          where user_id = $1 and last_reviewed_date is not null
          union
          select timezone((select name from selected_tz), created_at)::date as activity_date
          from attempts
          where user_id = $1
          union
          select timezone((select name from selected_tz), created_at)::date as activity_date
          from review_session_state
          where user_id = $1
          union
          select timezone((select name from selected_tz), updated_at)::date as activity_date
          from review_session_state
          where user_id = $1
          union
          select timezone((select name from selected_tz), last_activity_at)::date as activity_date
          from review_session_state
          where user_id = $1
        )
        select
          timezone((select name from selected_tz), now())::date as today,
          coalesce(array_agg(activity_date order by activity_date), array[]::date[]) as activity_dates
        from activity_dates
        "#,
    )
    .bind(user_id)
    .bind(timezone)
    .fetch_one(pool)
    .await?;

    Ok(calculate_streaks(activity_dates, today))
}

fn calculate_streaks(mut dates: Vec<NaiveDate>, today: NaiveDate) -> (i64, i64) {
    if dates.is_empty() {
        return (0, 0);
    }

    dates.sort_unstable();
    dates.dedup();

    let mut longest_streak = 1_i64;
    let mut run_length = 1_i64;
    for pair in dates.windows(2) {
        if pair[0].checked_add_days(Days::new(1)) == Some(pair[1]) {
            run_length += 1;
        } else {
            longest_streak = longest_streak.max(run_length);
            run_length = 1;
        }
    }
    longest_streak = longest_streak.max(run_length);

    let latest_activity = *dates.last().expect("dates is not empty");
    let streak_can_continue =
        latest_activity == today || latest_activity.checked_add_days(Days::new(1)) == Some(today);
    if !streak_can_continue {
        return (0, longest_streak);
    }

    let mut current_streak = 1_i64;
    let mut expected = latest_activity;
    for date in dates.iter().rev().skip(1) {
        if date.checked_add_days(Days::new(1)) == Some(expected) {
            current_streak += 1;
            expected = *date;
        } else {
            break;
        }
    }

    (current_streak, longest_streak)
}

pub async fn creator(pool: &sqlx::PgPool, username: String) -> AppResult<Value> {
    let profile = sqlx::query_scalar::<_, SqlJson<Value>>(
        r#"
        select jsonb_build_object(
          'id', id,
          'username', username,
          'first_name', first_name,
          'last_name', last_name,
          'avatar_url', avatar_url,
          'bio', bio
        ) as data
        from user_profiles
        where username = $1
        "#,
    )
    .bind(&username)
    .fetch_optional(pool)
    .await?
    .map(|SqlJson(v)| v)
    .ok_or_else(|| AppError::NotFound("Creator not found".to_owned()))?;
    let profile_id = profile
        .get("id")
        .and_then(Value::as_str)
        .and_then(|id| Uuid::parse_str(id).ok())
        .ok_or_else(|| AppError::Internal("Creator profile missing id".to_owned()))?;
    let display_name = display_name(&profile, &username);

    let sets = sqlx::query_scalar::<_, SqlJson<Value>>(
        r#"
        select jsonb_build_object(
          'id', ps.id,
          'name', ps.name,
          'description', ps.description,
          'subject_name', coalesce(s.name, 'Unknown'),
          'subject_color', s.color,
          'subject_icon', s.icon,
          'problem_count', count(psp.problem_id),
          'is_smart', ps.is_smart,
          'owner', jsonb_build_object(
            'username', up.username,
            'display_name',
              coalesce(nullif(concat_ws(' ', up.first_name, up.last_name), ''), up.username, 'Anonymous'),
            'avatar_url', up.avatar_url
          ),
          'stats', jsonb_build_object(
            'view_count', (select count(*)::bigint from problem_set_views where problem_set_id = ps.id),
            'unique_view_count', (select count(*)::bigint from problem_set_views where problem_set_id = ps.id),
            'like_count', (select count(*)::bigint from problem_set_likes where problem_set_id = ps.id),
            'copy_count', (select count(*)::bigint from problem_set_copies where source_problem_set_id = ps.id),
            'problem_count', count(psp.problem_id),
            'ranking_score', 0
          ),
          'created_at', ps.created_at
        ) as data
        from problem_sets ps
        left join subjects s on s.id = ps.subject_id
        left join user_profiles up on up.id = ps.user_id
        left join problem_set_problems psp on psp.problem_set_id = ps.id
        where ps.user_id = $1
          and ps.sharing_level = 'public'
          and coalesce(ps.is_listed, true) = true
        group by ps.id, s.name, s.color, s.icon, up.username, up.first_name, up.last_name, up.avatar_url
        order by ps.created_at desc
        "#,
    )
    .bind(profile_id)
    .fetch_all(pool)
    .await?
    .into_iter()
    .map(|SqlJson(v)| v)
    .collect::<Vec<_>>();

    let aggregate_stats = sets.iter().fold(
        json!({ "total_views": 0_i64, "total_likes": 0_i64, "total_copies": 0_i64 }),
        |mut acc, set| {
            let stats = set.get("stats").unwrap_or(&Value::Null);
            acc["total_views"] = json!(
                acc["total_views"].as_i64().unwrap_or(0)
                    + stats.get("view_count").and_then(Value::as_i64).unwrap_or(0)
            );
            acc["total_likes"] = json!(
                acc["total_likes"].as_i64().unwrap_or(0)
                    + stats.get("like_count").and_then(Value::as_i64).unwrap_or(0)
            );
            acc["total_copies"] = json!(
                acc["total_copies"].as_i64().unwrap_or(0)
                    + stats.get("copy_count").and_then(Value::as_i64).unwrap_or(0)
            );
            acc
        },
    );

    Ok(json!({
        "profile": {
            "username": username,
            "display_name": display_name,
            "avatar_url": profile.get("avatar_url"),
            "bio": profile.get("bio"),
        },
        "sets": sets,
        "aggregateStats": aggregate_stats,
    }))
}

pub async fn generate_digests(state: &AppState) -> AppResult<Value> {
    let users = sqlx::query_scalar::<_, Uuid>(
        "select distinct user_id from attempts where created_at >= now() - interval '7 days'",
    )
    .fetch_all(&state.pool)
    .await?;
    let mut completed = 0_usize;
    let mut failed = 0_usize;
    for user_id in &users {
        if insights::generate_and_store_digest(state, *user_id)
            .await
            .is_ok()
        {
            completed += 1;
        } else {
            failed += 1;
        }
    }
    Ok(json!({
        "status": 200_u16,
        "processed": users.len(),
        "completed": completed,
        "failed": failed,
    }))
}

fn display_name(profile: &Value, fallback: &str) -> String {
    let full_name = [
        profile.get("first_name").and_then(Value::as_str),
        profile.get("last_name").and_then(Value::as_str),
    ]
    .into_iter()
    .flatten()
    .filter(|value| !value.is_empty())
    .collect::<Vec<_>>()
    .join(" ");
    if full_name.is_empty() {
        profile
            .get("username")
            .and_then(Value::as_str)
            .unwrap_or(fallback)
            .to_owned()
    } else {
        full_name
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn date(value: &str) -> NaiveDate {
        NaiveDate::parse_from_str(value, "%Y-%m-%d").expect("valid date")
    }

    #[test]
    fn streaks_are_zero_without_activity() {
        assert_eq!(calculate_streaks(Vec::new(), date("2026-06-19")), (0, 0));
    }

    #[test]
    fn current_streak_counts_consecutive_days_through_today() {
        let dates = vec![
            date("2026-06-16"),
            date("2026-06-17"),
            date("2026-06-18"),
            date("2026-06-19"),
        ];

        assert_eq!(calculate_streaks(dates, date("2026-06-19")), (4, 4));
    }

    #[test]
    fn current_streak_remains_active_when_latest_activity_was_yesterday() {
        let dates = vec![date("2026-06-15"), date("2026-06-16"), date("2026-06-18")];

        assert_eq!(calculate_streaks(dates, date("2026-06-19")), (1, 2));
    }

    #[test]
    fn current_streak_resets_after_a_full_missed_day() {
        let dates = vec![date("2026-06-14"), date("2026-06-15"), date("2026-06-17")];

        assert_eq!(calculate_streaks(dates, date("2026-06-19")), (0, 2));
    }

    #[test]
    fn longest_streak_uses_deduplicated_dates() {
        let dates = vec![
            date("2026-06-10"),
            date("2026-06-10"),
            date("2026-06-11"),
            date("2026-06-14"),
            date("2026-06-15"),
            date("2026-06-16"),
        ];

        assert_eq!(calculate_streaks(dates, date("2026-06-16")), (3, 3));
    }
}
