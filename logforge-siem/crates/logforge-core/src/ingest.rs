use crate::model::Severity;
use chrono::Utc;
use serde_json::Value;
use sha2::{Digest, Sha256};
use thiserror::Error;

pub const MAX_LINE_BYTES: usize = 65_536;

#[derive(Debug, Error, PartialEq, Eq)]
pub enum IngestError {
    #[error("empty log line")]
    Empty,
    #[error("log line exceeds maximum size of {MAX_LINE_BYTES} bytes")]
    Oversized,
    #[error("malformed log line: {0}")]
    Malformed(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NormalizedEvent {
    pub timestamp_utc: String,
    pub severity: Severity,
    pub host: String,
    pub service: String,
    pub message: String,
    pub raw_line: String,
    pub dedup_hash: String,
    pub redacted: bool,
    pub fields_json: String,
}

pub fn redact_secrets(input: &str) -> (String, bool) {
    let mut out = input.to_string();
    let mut redacted = false;

    for prefix in ["password=", "api_key=", "token=", "secret="] {
        let mut search_from = 0;
        while let Some(pos) = out[search_from..].to_ascii_lowercase().find(prefix) {
            let abs_pos = search_from + pos + prefix.len();
            let end = out[abs_pos..]
                .find(|c: char| c.is_whitespace() || c == ',' || c == ';' || c == '"' || c == '\'')
                .map(|i| abs_pos + i)
                .unwrap_or(out.len());
            if end > abs_pos && &out[abs_pos..end] != "[REDACTED]" {
                out.replace_range(abs_pos..end, "[REDACTED]");
                redacted = true;
            }
            search_from = abs_pos.min(out.len());
        }
    }

    let mut search_from = 0;
    while let Some(pos) = out[search_from..].find("Bearer ") {
        let abs_pos = search_from + pos + "Bearer ".len();
        let end = out[abs_pos..]
            .find(|c: char| c.is_whitespace() || c == '"' || c == '\'')
            .map(|i| abs_pos + i)
            .unwrap_or(out.len());
        if end > abs_pos && &out[abs_pos..end] != "[REDACTED]" {
            out.replace_range(abs_pos..end, "[REDACTED]");
            redacted = true;
        }
        search_from = abs_pos.min(out.len());
    }

    (out, redacted)
}

pub fn normalize_timestamp(ts: Option<&str>) -> Result<String, IngestError> {
    match ts {
        None | Some("") => Ok(Utc::now().to_rfc3339()),
        Some(raw) => crate::timestamp::parse_timestamp(raw, &crate::timestamp::TimestampContext::default())
            .map(|parsed| parsed.timestamp_utc.to_rfc3339())
            .map_err(|_| IngestError::Malformed(format!("invalid timestamp: {raw}"))),
    }
}

pub fn compute_dedup_hash(timestamp_utc: &str, host: &str, service: &str, message: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(timestamp_utc.as_bytes());
    hasher.update(b"|");
    hasher.update(host.as_bytes());
    hasher.update(b"|");
    hasher.update(service.as_bytes());
    hasher.update(b"|");
    hasher.update(message.as_bytes());
    let digest = hasher.finalize();
    format!("{digest:x}")
}

pub fn parse_line(raw: &str) -> Result<NormalizedEvent, IngestError> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return Err(IngestError::Empty);
    }
    if trimmed.len() > MAX_LINE_BYTES {
        return Err(IngestError::Oversized);
    }

    let (clean_raw, redacted) = redact_secrets(trimmed);

    if clean_raw.starts_with('{') {
        let val: Value = serde_json::from_str(&clean_raw)
            .map_err(|e| IngestError::Malformed(format!("invalid json: {e}")))?;
        let obj = val
            .as_object()
            .ok_or_else(|| IngestError::Malformed("json root must be object".to_string()))?;

        let timestamp_utc = normalize_timestamp(obj.get("timestamp").and_then(|v| v.as_str()))?;
        let severity = obj
            .get("severity")
            .and_then(|v| v.as_str())
            .and_then(Severity::parse)
            .unwrap_or(Severity::Info);
        let host = obj
            .get("host")
            .and_then(|v| v.as_str())
            .unwrap_or("unknown")
            .to_string();
        let service = obj
            .get("service")
            .and_then(|v| v.as_str())
            .unwrap_or("unknown")
            .to_string();
        let message = obj
            .get("message")
            .and_then(|v| v.as_str())
            .ok_or_else(|| IngestError::Malformed("missing message field".to_string()))?
            .to_string();

        let dedup_hash = compute_dedup_hash(&timestamp_utc, &host, &service, &message);
        return Ok(NormalizedEvent {
            timestamp_utc,
            severity,
            host,
            service,
            message,
            raw_line: clean_raw.clone(),
            dedup_hash,
            redacted,
            fields_json: clean_raw,
        });
    }

    let parts: Vec<&str> = clean_raw.splitn(4, ' ').collect();
    if parts.len() < 4 {
        return Err(IngestError::Malformed(
            "expected '<rfc3339> <host> <service> <message>'".to_string(),
        ));
    }

    let timestamp_utc = normalize_timestamp(Some(parts[0]))?;
    let host = parts[1].to_string();
    let service = parts[2].trim_end_matches(':').to_string();
    let message = parts[3].to_string();
    let severity = if message.to_ascii_lowercase().contains("critical") {
        Severity::Critical
    } else if message.to_ascii_lowercase().contains("error") {
        Severity::Error
    } else if message.to_ascii_lowercase().contains("fail")
        || message.to_ascii_lowercase().contains("warn")
    {
        Severity::Warn
    } else {
        Severity::Info
    };

    let dedup_hash = compute_dedup_hash(&timestamp_utc, &host, &service, &message);
    Ok(NormalizedEvent {
        timestamp_utc,
        severity,
        host,
        service,
        message,
        raw_line: clean_raw,
        dedup_hash,
        redacted,
        fields_json: "{}".to_string(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_empty_and_oversized_lines() {
        assert_eq!(parse_line("   "), Err(IngestError::Empty));
        let big = "a".repeat(MAX_LINE_BYTES + 1);
        assert_eq!(parse_line(&big), Err(IngestError::Oversized));
    }

    #[test]
    fn normalizes_timezone_to_utc() {
        let line = r#"{"timestamp":"2026-11-21T12:00:00+02:00","severity":"warn","host":"web-01","service":"sshd","message":"Failed login"}"#;
        let ev = parse_line(line).expect("parse json");
        assert!(ev.timestamp_utc.starts_with("2026-11-21T10:00:00"));
        assert!(ev.timestamp_utc.ends_with("+00:00") || ev.timestamp_utc.ends_with('Z'));
        assert_eq!(ev.severity, Severity::Warn);
        assert!(!ev.redacted);
    }

    #[test]
    fn redacts_secrets_and_marks_flag() {
        let line = "2026-11-21T10:00:00Z api-01 auth: login failed token=abc12345 Authorization: Bearer xyz987";
        let ev = parse_line(line).expect("parse syslog");
        assert!(ev.redacted);
        assert!(!ev.message.contains("abc12345"));
        assert!(!ev.message.contains("xyz987"));
        assert!(ev.message.contains("token=[REDACTED]"));
        assert!(ev.message.contains("Bearer [REDACTED]"));
    }

    #[test]
    fn rejects_malformed_json_and_syslog() {
        assert!(matches!(
            parse_line("{bad json"),
            Err(IngestError::Malformed(_))
        ));
        assert!(matches!(
            parse_line("only two"),
            Err(IngestError::Malformed(_))
        ));
    }
}
