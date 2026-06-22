use chrono::{DateTime, Duration, LocalResult, NaiveTime, TimeZone, Utc};
use chrono_tz::Tz;
use sqlx::PgPool;
use uuid::Uuid;

use crate::error::AppResult;

const DEFAULT_TIMEZONE: &str = "UTC";
const DEFAULT_EASE_FACTOR: f64 = 2.5;
const MIN_EASE_FACTOR: f64 = 1.3;
const DEFAULT_INTERVAL_DAYS: i32 = 1;
const FIRST_CORRECT_INTERVAL_DAYS: i32 = 1;
const SECOND_CORRECT_INTERVAL_DAYS: i32 = 3;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ReviewScheduleState {
    pub repetition_number: i32,
    pub ease_factor: f64,
    pub interval_days: i32,
    pub last_reviewed_at: Option<DateTime<Utc>>,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ReviewScheduleUpdate {
    pub repetition_number: i32,
    pub ease_factor: f64,
    pub interval_days: i32,
    pub next_review_at: DateTime<Utc>,
    pub last_reviewed_at: DateTime<Utc>,
    pub advanced_sm2: bool,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Sm2Input {
    pub repetition_number: i32,
    pub ease_factor: f64,
    pub interval_days: i32,
    pub quality: f64,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Sm2Result {
    pub repetition_number: i32,
    pub ease_factor: f64,
    pub interval_days: i32,
}

pub async fn ensure_initial_schedule(
    pool: &PgPool,
    user_id: Uuid,
    problem_id: Uuid,
) -> AppResult<()> {
    sqlx::query(
        r#"
        insert into review_schedule (user_id, problem_id, next_review_at, interval_days)
        values ($1, $2, now(), 1)
        on conflict (user_id, problem_id) do nothing
        "#,
    )
    .bind(user_id)
    .bind(problem_id)
    .execute(pool)
    .await?;
    Ok(())
}

pub async fn update_review_schedule(
    pool: &PgPool,
    user_id: Uuid,
    problem_id: Uuid,
    selected_status: &str,
) -> AppResult<()> {
    let existing = sqlx::query_as::<_, (i32, f64, i32, Option<DateTime<Utc>>)>(
        r#"
        select repetition_number, ease_factor, interval_days, last_reviewed_at
        from review_schedule
        where user_id = $1 and problem_id = $2
        "#,
    )
    .bind(user_id)
    .bind(problem_id)
    .fetch_optional(pool)
    .await?
    .map(|row| ReviewScheduleState {
        repetition_number: row.0,
        ease_factor: row.1,
        interval_days: row.2,
        last_reviewed_at: row.3,
    });

    let timezone =
        sqlx::query_scalar::<_, Option<String>>("select timezone from user_profiles where id = $1")
            .bind(user_id)
            .fetch_optional(pool)
            .await?
            .flatten()
            .unwrap_or_else(|| DEFAULT_TIMEZONE.to_owned());

    let update = calculate_review_schedule_update(existing, selected_status, Utc::now(), &timezone);

    sqlx::query(
        r#"
        insert into review_schedule (
          user_id, problem_id, next_review_at, interval_days, ease_factor,
          repetition_number, last_reviewed_at
        )
        values ($1, $2, $3, $4, $5, $6, $7)
        on conflict (user_id, problem_id)
        do update set
          next_review_at = excluded.next_review_at,
          interval_days = excluded.interval_days,
          ease_factor = excluded.ease_factor,
          repetition_number = excluded.repetition_number,
          last_reviewed_at = excluded.last_reviewed_at,
          updated_at = now()
        "#,
    )
    .bind(user_id)
    .bind(problem_id)
    .bind(update.next_review_at)
    .bind(update.interval_days)
    .bind(update.ease_factor)
    .bind(update.repetition_number)
    .bind(update.last_reviewed_at)
    .execute(pool)
    .await?;

    Ok(())
}

pub fn calculate_review_schedule_update(
    existing: Option<ReviewScheduleState>,
    selected_status: &str,
    now: DateTime<Utc>,
    timezone: &str,
) -> ReviewScheduleUpdate {
    if let Some(existing) = existing
        && existing.last_reviewed_at.is_some_and(|last_reviewed_at| {
            is_same_day_in_timezone(last_reviewed_at, now, timezone)
        })
    {
        return ReviewScheduleUpdate {
            repetition_number: existing.repetition_number,
            ease_factor: existing.ease_factor,
            interval_days: existing.interval_days,
            next_review_at: local_midnight_after_days(existing.interval_days, now, timezone),
            last_reviewed_at: now,
            advanced_sm2: false,
        };
    }

    let state = existing.unwrap_or(ReviewScheduleState {
        repetition_number: 0,
        ease_factor: DEFAULT_EASE_FACTOR,
        interval_days: DEFAULT_INTERVAL_DAYS,
        last_reviewed_at: None,
    });
    let sm2 = calculate_sm2_next_review(Sm2Input {
        repetition_number: state.repetition_number,
        ease_factor: state.ease_factor,
        interval_days: state.interval_days,
        quality: map_status_to_quality(selected_status),
    });

    ReviewScheduleUpdate {
        repetition_number: sm2.repetition_number,
        ease_factor: sm2.ease_factor,
        interval_days: sm2.interval_days,
        next_review_at: local_midnight_after_days(sm2.interval_days, now, timezone),
        last_reviewed_at: now,
        advanced_sm2: true,
    }
}

pub fn map_status_to_quality(selected_status: &str) -> f64 {
    match selected_status {
        "mastered" => 5.0,
        "needs_review" => 3.0,
        _ => 1.0,
    }
}

pub fn calculate_sm2_next_review(input: Sm2Input) -> Sm2Result {
    if input.quality >= 3.0 {
        let repetition_number = input.repetition_number + 1;
        let interval_days = if repetition_number == 1 {
            FIRST_CORRECT_INTERVAL_DAYS
        } else if repetition_number == 2 {
            SECOND_CORRECT_INTERVAL_DAYS
        } else {
            ((input.interval_days as f64) * input.ease_factor)
                .round()
                .max(1.0) as i32
        };
        let delta = 5.0 - input.quality;
        let ease_factor =
            (input.ease_factor + (0.1 - delta * (0.08 + delta * 0.02))).max(MIN_EASE_FACTOR);

        Sm2Result {
            repetition_number,
            ease_factor,
            interval_days,
        }
    } else {
        Sm2Result {
            repetition_number: 0,
            ease_factor: input.ease_factor,
            interval_days: DEFAULT_INTERVAL_DAYS,
        }
    }
}

pub fn local_midnight_after_days(
    days_from_now: i32,
    now: DateTime<Utc>,
    timezone: &str,
) -> DateTime<Utc> {
    let tz = parse_timezone(timezone);
    let local_today = now.with_timezone(&tz).date_naive();
    let target_date = local_today + Duration::days(i64::from(days_from_now));
    let local_midnight = target_date.and_time(NaiveTime::MIN);

    match tz.from_local_datetime(&local_midnight) {
        LocalResult::Single(value) => value.with_timezone(&Utc),
        LocalResult::Ambiguous(first, second) => first.min(second).with_timezone(&Utc),
        LocalResult::None => {
            let mut probe = local_midnight + Duration::minutes(1);
            for _ in 0..180 {
                if let Some(value) = tz.from_local_datetime(&probe).earliest() {
                    return value.with_timezone(&Utc);
                }
                probe += Duration::minutes(1);
            }
            Utc.from_utc_datetime(&local_midnight)
        }
    }
}

pub fn is_same_day_in_timezone(
    first: DateTime<Utc>,
    second: DateTime<Utc>,
    timezone: &str,
) -> bool {
    let tz = parse_timezone(timezone);
    first.with_timezone(&tz).date_naive() == second.with_timezone(&tz).date_naive()
}

fn parse_timezone(timezone: &str) -> Tz {
    timezone.parse::<Tz>().unwrap_or(chrono_tz::UTC)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dt(value: &str) -> DateTime<Utc> {
        value.parse::<DateTime<Utc>>().expect("valid RFC3339 UTC")
    }

    #[test]
    fn maps_status_to_quality_like_original() {
        assert_eq!(map_status_to_quality("wrong"), 1.0);
        assert_eq!(map_status_to_quality("needs_review"), 3.0);
        assert_eq!(map_status_to_quality("mastered"), 5.0);
    }

    #[test]
    fn calculates_sm2_correct_intervals_and_ease() {
        let first = calculate_sm2_next_review(Sm2Input {
            repetition_number: 0,
            ease_factor: 2.5,
            interval_days: 1,
            quality: 5.0,
        });
        assert_eq!(first.repetition_number, 1);
        assert_eq!(first.interval_days, 1);
        assert!((first.ease_factor - 2.6).abs() < 0.0001);

        let second = calculate_sm2_next_review(Sm2Input {
            repetition_number: 1,
            ease_factor: 2.5,
            interval_days: 1,
            quality: 3.0,
        });
        assert_eq!(second.repetition_number, 2);
        assert_eq!(second.interval_days, 3);
        assert!((second.ease_factor - 2.36).abs() < 0.0001);

        let later = calculate_sm2_next_review(Sm2Input {
            repetition_number: 2,
            ease_factor: 2.5,
            interval_days: 3,
            quality: 4.0,
        });
        assert_eq!(later.repetition_number, 3);
        assert_eq!(later.interval_days, 8);
    }

    #[test]
    fn incorrect_review_resets_repetition_and_interval_only() {
        let result = calculate_sm2_next_review(Sm2Input {
            repetition_number: 5,
            ease_factor: 2.1,
            interval_days: 30,
            quality: 1.0,
        });

        assert_eq!(result.repetition_number, 0);
        assert_eq!(result.interval_days, 1);
        assert_eq!(result.ease_factor, 2.1);
    }

    #[test]
    fn next_review_is_user_local_midnight_across_dst() {
        let now = dt("2026-03-08T15:00:00Z");
        let next = local_midnight_after_days(1, now, "America/New_York");

        assert_eq!(next.to_rfc3339(), "2026-03-09T04:00:00+00:00");
    }

    #[test]
    fn invalid_timezone_falls_back_to_utc_midnight() {
        let now = dt("2026-03-08T15:00:00Z");
        let next = local_midnight_after_days(1, now, "Not/AZone");

        assert_eq!(next.to_rfc3339(), "2026-03-09T00:00:00+00:00");
    }

    #[test]
    fn detects_same_day_in_user_timezone() {
        assert!(is_same_day_in_timezone(
            dt("2026-01-01T16:30:00Z"),
            dt("2026-01-02T15:30:00Z"),
            "Asia/Shanghai"
        ));
        assert!(!is_same_day_in_timezone(
            dt("2026-01-01T16:30:00Z"),
            dt("2026-01-02T15:30:00Z"),
            "UTC"
        ));
    }

    #[test]
    fn same_day_review_refreshes_due_date_without_advancing_sm2() {
        let now = dt("2026-01-02T15:30:00Z");
        let existing = ReviewScheduleState {
            repetition_number: 2,
            ease_factor: 2.36,
            interval_days: 3,
            last_reviewed_at: Some(dt("2026-01-01T16:30:00Z")),
        };
        let update =
            calculate_review_schedule_update(Some(existing), "mastered", now, "Asia/Shanghai");

        assert!(!update.advanced_sm2);
        assert_eq!(update.repetition_number, 2);
        assert_eq!(update.ease_factor, 2.36);
        assert_eq!(update.interval_days, 3);
        assert_eq!(
            update.next_review_at.to_rfc3339(),
            "2026-01-04T16:00:00+00:00"
        );
        assert_eq!(update.last_reviewed_at, now);
    }

    #[test]
    fn first_review_of_day_advances_sm2_and_uses_local_midnight() {
        let now = dt("2026-01-02T01:00:00Z");
        let existing = ReviewScheduleState {
            repetition_number: 1,
            ease_factor: 2.5,
            interval_days: 1,
            last_reviewed_at: Some(dt("2026-01-01T01:00:00Z")),
        };
        let update =
            calculate_review_schedule_update(Some(existing), "needs_review", now, "Asia/Shanghai");

        assert!(update.advanced_sm2);
        assert_eq!(update.repetition_number, 2);
        assert_eq!(update.interval_days, 3);
        assert!((update.ease_factor - 2.36).abs() < 0.0001);
        assert_eq!(
            update.next_review_at.to_rfc3339(),
            "2026-01-04T16:00:00+00:00"
        );
        assert_eq!(update.last_reviewed_at, now);
    }
}
