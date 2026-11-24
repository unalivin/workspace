#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolves_missing_year_using_reference_clock_and_offset() {
        let now = Utc.with_ymd_and_hms(2027, 1, 1, 0, 0, 0).unwrap();
        let value = normalize_source_timestamp(Some("Dec 31 23:30:00"), 3600, now).unwrap();
        assert!(value.starts_with("2026-12-31T22:30:00"));
    }

    #[test]
    fn rejects_invalid_offset_and_malformed_time() {
        let now = Utc::now();
        assert!(normalize_source_timestamp(Some("not a date"), 0, now).is_err());
        assert!(normalize_source_timestamp(Some("Jan 01 00:00:00"), 100_000, now).is_err());
    }
}
