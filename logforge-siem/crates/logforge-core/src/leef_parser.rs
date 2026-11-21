use crate::ingest::{
    compute_dedup_hash, normalize_timestamp, redact_secrets, IngestError, NormalizedEvent,
};
use crate::model::Severity;

pub fn parse_leef_line(raw: &str) -> Result<NormalizedEvent, IngestError> {
    let trimmed = raw.trim();
    if !trimmed.starts_with("LEEF:") {
        return Err(IngestError::Malformed("missing LEEF: prefix".to_string()));
    }
    let (clean, redacted) = redact_secrets(trimmed);
    let parts: Vec<&str> = clean.splitn(6, '|').collect();
    if parts.len() < 6 {
        return Err(IngestError::Malformed(
            "LEEF requires 5 pipe-delimited header fields".to_string(),
        ));
    }
    let vendor = parts[1].to_string();
    let product = parts[2].to_string();
    let event_id = parts[4].to_string();
    let timestamp_utc = normalize_timestamp(None)?;
    let message = format!("[{event_id}] {}", parts[5]);
    let dedup_hash = compute_dedup_hash(&timestamp_utc, &vendor, &product, &message);

    Ok(NormalizedEvent {
        timestamp_utc,
        severity: Severity::Info,
        host: vendor,
        service: product,
        message,
        raw_line: clean,
        dedup_hash,
        redacted,
        fields_json: "{}".to_string(),
    })
}
