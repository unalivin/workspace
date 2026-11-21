use crate::ingest::{
    compute_dedup_hash, normalize_timestamp, redact_secrets, IngestError, NormalizedEvent,
};
use crate::model::Severity;

pub fn parse_rfc5424(raw: &str) -> Result<NormalizedEvent, IngestError> {
    let trimmed = raw.trim();
    if !trimmed.starts_with('<') {
        return Err(IngestError::Malformed("missing PRI <...>".to_string()));
    }
    let end_pri = trimmed
        .find('>')
        .ok_or_else(|| IngestError::Malformed("unclosed PRI".to_string()))?;
    let pri: u16 = trimmed[1..end_pri]
        .parse()
        .map_err(|_| IngestError::Malformed("invalid PRI number".to_string()))?;
    let sev_code = pri % 8;
    let severity = match sev_code {
        0..=2 => Severity::Critical,
        3 => Severity::Error,
        4 => Severity::Warn,
        5..=6 => Severity::Info,
        _ => Severity::Debug,
    };

    let rest = trimmed[end_pri + 1..].trim_start();
    let parts: Vec<&str> = rest.splitn(6, ' ').collect();
    if parts.len() < 6 {
        return Err(IngestError::Malformed(
            "incomplete RFC5424 header".to_string(),
        ));
    }

    let ts = if parts[1] == "-" {
        None
    } else {
        Some(parts[1])
    };
    let timestamp_utc = normalize_timestamp(ts)?;
    let host = parts[2].to_string();
    let service = parts[3].to_string();
    let (clean_msg, redacted) = redact_secrets(parts[5]);
    let dedup_hash = compute_dedup_hash(&timestamp_utc, &host, &service, &clean_msg);

    Ok(NormalizedEvent {
        timestamp_utc,
        severity,
        host,
        service,
        message: clean_msg,
        raw_line: trimmed.to_string(),
        dedup_hash,
        redacted,
        fields_json: "{}".to_string(),
    })
}
