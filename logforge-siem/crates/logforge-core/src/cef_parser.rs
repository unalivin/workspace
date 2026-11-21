use crate::ingest::{compute_dedup_hash, normalize_timestamp, redact_secrets, IngestError, NormalizedEvent};
use crate::model::Severity;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CefHeader {
    pub version: u8,
    pub device_vendor: String,
    pub device_product: String,
    pub device_version: String,
    pub signature_id: String,
    pub name: String,
    pub severity_raw: String,
}

pub fn parse_cef_line(raw: &str) -> Result<NormalizedEvent, IngestError> {
    let trimmed = raw.trim();
    if !trimmed.starts_with("CEF:") {
        return Err(IngestError::Malformed("missing CEF: prefix".to_string()));
    }
    let (clean, redacted) = redact_secrets(trimmed);
    let parts: Vec<&str> = clean.splitn(8, '|').collect();
    if parts.len() < 8 {
        return Err(IngestError::Malformed("CEF requires 7 pipe-delimited header fields".to_string()));
    }

    let version_str = parts[0].trim_start_matches("CEF:");
    let _version: u8 = version_str
        .parse()
        .map_err(|_| IngestError::Malformed("invalid CEF version".to_string()))?;

    let vendor = parts[1].to_string();
    let product = parts[2].to_string();
    let signature_id = parts[4].to_string();
    let name = parts[5].to_string();
    let sev_num: u8 = parts[6].parse().unwrap_or(3);

    let severity = match sev_num {
        0..=2 => Severity::Info,
        3..=5 => Severity::Warn,
        6..=8 => Severity::Error,
        _ => Severity::Critical,
    };

    let timestamp_utc = normalize_timestamp(None)?;
    let message = format!("[{signature_id}] {name} {}", parts[7]);
    let dedup_hash = compute_dedup_hash(&timestamp_utc, &vendor, &product, &message);

    Ok(NormalizedEvent {
        timestamp_utc,
        severity,
        host: vendor,
        service: product,
        message,
        raw_line: clean,
        dedup_hash,
        redacted,
        fields_json: "{}".to_string(),
    })
}
