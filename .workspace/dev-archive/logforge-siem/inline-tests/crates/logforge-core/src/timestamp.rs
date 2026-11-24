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
