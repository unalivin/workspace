use crate::ingest::{normalize_timestamp, IngestError};
use chrono::{DateTime, Datelike, FixedOffset, NaiveDateTime, TimeZone, Utc};

pub fn normalize_source_timestamp(
    raw: Option<&str>,
    source_offset_seconds: i32,
    now: DateTime<Utc>,
) -> Result<String, IngestError> {
    let Some(raw) = raw.filter(|value| !value.trim().is_empty() && *value != "-") else {
        return Ok(now.to_rfc3339());
    };
    if let Ok(result) = normalize_timestamp(Some(raw)) {
        return Ok(result);
    }
    let local = NaiveDateTime::parse_from_str(
        &format!("{} {raw}", now.year()),
        "%Y %b %e %H:%M:%S",
    )
    .or_else(|_| NaiveDateTime::parse_from_str(&format!("{} {raw}", now.year()), "%Y %b %d %H:%M:%S"))
    .map_err(|_| IngestError::Malformed(format!("invalid timestamp: {raw}")))?;
    let offset = FixedOffset::east_opt(source_offset_seconds).ok_or_else(|| {
        IngestError::Malformed("source timezone offset is out of range".to_string())
    })?;
    let mut year = now.year();
    let mut candidate = offset
        .from_local_datetime(
            &local
                .with_year(year)
                .ok_or_else(|| IngestError::Malformed("invalid syslog timestamp".to_string()))?,
        )
        .single()
        .ok_or_else(|| IngestError::Malformed("ambiguous local timestamp".to_string()))?;
    if candidate.with_timezone(&Utc) > now + chrono::Duration::days(1) {
        year -= 1;
        candidate =
            offset
                .from_local_datetime(&local.with_year(year).ok_or_else(|| {
                    IngestError::Malformed("invalid syslog timestamp".to_string())
                })?)
                .single()
                .ok_or_else(|| IngestError::Malformed("ambiguous local timestamp".to_string()))?;
    }
    Ok(candidate.with_timezone(&Utc).to_rfc3339())
}

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
