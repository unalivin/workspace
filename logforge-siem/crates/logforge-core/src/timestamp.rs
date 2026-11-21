use chrono::{DateTime, Datelike, LocalResult, NaiveDate, NaiveDateTime, TimeZone, Utc};
use thiserror::Error;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TimestampContext {
    pub reference_utc: DateTime<Utc>,
    pub local_timezone: chrono_tz::Tz,
    pub max_clock_skew_seconds: i64,
}

impl Default for TimestampContext {
    fn default() -> Self {
        Self {
            reference_utc: Utc::now(),
            local_timezone: chrono_tz::UTC,
            max_clock_skew_seconds: 300,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParsedTimestamp {
    pub timestamp_utc: DateTime<Utc>,
    pub clock_skewed: bool,
    pub inferred_year: bool,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum TimestampError {
    #[error("timestamp is not a supported RFC3339, syslog, or local date-time value")]
    Invalid,
    #[error("local timestamp falls in a daylight-saving time gap")]
    NonexistentLocalTime,
    #[error("local timestamp is ambiguous at a daylight-saving transition")]
    AmbiguousLocalTime,
}

pub fn parse_timestamp(
    raw: &str,
    context: &TimestampContext,
) -> Result<ParsedTimestamp, TimestampError> {
    let text = raw.trim();
    let (parsed, inferred_year) = if let Ok(offset) = DateTime::parse_from_rfc3339(text) {
        (offset.with_timezone(&Utc), false)
    } else if let Ok(naive) = NaiveDateTime::parse_from_str(text, "%Y-%m-%d %H:%M:%S") {
        (
            resolve_local(context.local_timezone.from_local_datetime(&naive))?,
            false,
        )
    } else if text.contains(':') {
        let mut pieces = text.split_whitespace();
        let month = pieces.next().ok_or(TimestampError::Invalid)?;
        let day = pieces.next().ok_or(TimestampError::Invalid)?;
        let time_text = pieces.next().ok_or(TimestampError::Invalid)?;
        if pieces.next().is_some() {
            return Err(TimestampError::Invalid);
        }
        let (month, day) =
            parse_syslog_day(&format!("{month} {day}")).ok_or(TimestampError::Invalid)?;
        let time = chrono::NaiveTime::parse_from_str(time_text, "%H:%M:%S")
            .map_err(|_| TimestampError::Invalid)?;
        let reference_date = context.reference_utc.with_timezone(&context.local_timezone);
        let candidate = nearest_plausible_date(
            reference_date.year(),
            month,
            day,
            time,
            context.local_timezone,
            context.reference_utc,
        )?;
        (candidate, true)
    } else {
        return Err(TimestampError::Invalid);
    };
    let skew = (parsed - context.reference_utc).num_seconds().abs();
    Ok(ParsedTimestamp {
        timestamp_utc: parsed,
        clock_skewed: skew > context.max_clock_skew_seconds.max(0),
        inferred_year,
    })
}

fn resolve_local(
    value: LocalResult<chrono::DateTime<chrono_tz::Tz>>,
) -> Result<DateTime<Utc>, TimestampError> {
    match value {
        LocalResult::Single(v) => Ok(v.with_timezone(&Utc)),
        LocalResult::None => Err(TimestampError::NonexistentLocalTime),
        LocalResult::Ambiguous(_, _) => Err(TimestampError::AmbiguousLocalTime),
    }
}

fn parse_syslog_day(raw: &str) -> Option<(u32, u32)> {
    let mut pieces = raw.split_whitespace();
    let month = match pieces.next()? {
        "Jan" => 1,
        "Feb" => 2,
        "Mar" => 3,
        "Apr" => 4,
        "May" => 5,
        "Jun" => 6,
        "Jul" => 7,
        "Aug" => 8,
        "Sep" => 9,
        "Oct" => 10,
        "Nov" => 11,
        "Dec" => 12,
        _ => return None,
    };
    let day = pieces.next()?.parse().ok()?;
    if pieces.next().is_some() {
        return None;
    }
    Some((month, day))
}

fn nearest_plausible_date(
    year: i32,
    month: u32,
    day: u32,
    time: chrono::NaiveTime,
    tz: chrono_tz::Tz,
    reference: DateTime<Utc>,
) -> Result<DateTime<Utc>, TimestampError> {
    let mut candidates = Vec::new();
    for candidate_year in [year - 1, year, year + 1] {
        if let Some(date) = NaiveDate::from_ymd_opt(candidate_year, month, day) {
            let naive = date.and_time(time);
            if let Ok(utc) = resolve_local(tz.from_local_datetime(&naive)) {
                candidates.push(utc);
            }
        }
    }
    candidates
        .into_iter()
        .min_by_key(|value| (*value - reference).num_seconds().abs())
        .ok_or(TimestampError::NonexistentLocalTime)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn context(reference: &str, tz: chrono_tz::Tz) -> TimestampContext {
        TimestampContext {
            reference_utc: DateTime::parse_from_rfc3339(reference)
                .unwrap()
                .with_timezone(&Utc),
            local_timezone: tz,
            max_clock_skew_seconds: 300,
        }
    }

    #[test]
    fn offsets_are_normalized_and_skew_is_flagged() {
        let ctx = context("2026-01-01T00:00:00Z", chrono_tz::UTC);
        let parsed = parse_timestamp("2026-01-01T02:00:00+02:00", &ctx).unwrap();
        assert_eq!(
            parsed.timestamp_utc.to_rfc3339(),
            "2026-01-01T00:00:00+00:00"
        );
        assert!(!parsed.clock_skewed);
        assert!(
            parse_timestamp("2026-01-01T01:00:00Z", &ctx)
                .unwrap()
                .clock_skewed
        );
    }

    #[test]
    fn infers_year_for_syslog_and_detects_dst_gaps() {
        let ctx = context("2026-01-01T00:00:00Z", chrono_tz::America::New_York);
        assert!(
            parse_timestamp("Dec 31 23:00:00", &ctx)
                .unwrap()
                .inferred_year
        );
        assert_eq!(
            parse_timestamp("2026-03-08 02:30:00", &ctx),
            Err(TimestampError::NonexistentLocalTime)
        );
        assert_eq!(
            parse_timestamp("2026-11-01 01:30:00", &ctx),
            Err(TimestampError::AmbiguousLocalTime)
        );
    }
}
