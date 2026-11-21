use crate::ingest::{compute_dedup_hash, IngestError, NormalizedEvent};
use crate::model::Severity;
use chrono::{DateTime, NaiveDateTime, TimeZone, Utc};
use serde_json::json;

fn event(
    timestamp: String,
    host: String,
    service: &str,
    message: String,
    severity: Severity,
    raw: &str,
) -> NormalizedEvent {
    let dedup_hash = compute_dedup_hash(&timestamp, &host, service, &message);
    NormalizedEvent {
        timestamp_utc: timestamp,
        severity,
        host,
        service: service.to_owned(),
        message,
        raw_line: raw.to_owned(),
        dedup_hash,
        redacted: false,
        fields_json: "{}".to_owned(),
    }
}

pub fn parse_rfc3164(
    raw: &str,
    year: i32,
    offset_seconds: i32,
) -> Result<NormalizedEvent, IngestError> {
    let line = raw.trim();
    if line.len() > crate::ingest::MAX_LINE_BYTES {
        return Err(IngestError::Oversized);
    }
    let close = line
        .find('>')
        .filter(|_| line.starts_with('<'))
        .ok_or_else(|| IngestError::Malformed("RFC3164 PRI missing".to_owned()))?;
    let pri: u8 = line[1..close]
        .parse()
        .map_err(|_| IngestError::Malformed("RFC3164 PRI invalid".to_owned()))?;
    if pri > 191 {
        return Err(IngestError::Malformed(
            "RFC3164 PRI out of range".to_owned(),
        ));
    }
    let offset = chrono::FixedOffset::east_opt(offset_seconds)
        .ok_or_else(|| IngestError::Malformed("source timezone offset out of range".to_owned()))?;
    let rest = line[close + 1..].trim_start();
    let mut parts = rest.splitn(4, ' ');
    let month = parts.next().unwrap_or_default();
    let day = parts.next().unwrap_or_default();
    let time = parts.next().unwrap_or_default();
    let tail = parts
        .next()
        .ok_or_else(|| IngestError::Malformed("RFC3164 header incomplete".to_owned()))?;
    let stamp = format!("{year} {month} {day} {time}");
    let local = NaiveDateTime::parse_from_str(&stamp, "%Y %b %e %H:%M:%S")
        .or_else(|_| NaiveDateTime::parse_from_str(&stamp, "%Y %b %d %H:%M:%S"))
        .map_err(|_| IngestError::Malformed("RFC3164 timestamp invalid".to_owned()))?;
    let timestamp = offset
        .from_local_datetime(&local)
        .single()
        .ok_or_else(|| IngestError::Malformed("RFC3164 local time ambiguous".to_owned()))?
        .with_timezone(&Utc)
        .to_rfc3339();
    let (host, tag) = tail
        .split_once(' ')
        .ok_or_else(|| IngestError::Malformed("RFC3164 hostname or message missing".to_owned()))?;
    let (service, message) = tag
        .split_once(':')
        .ok_or_else(|| IngestError::Malformed("RFC3164 tag separator missing".to_owned()))?;
    if host.is_empty() || service.is_empty() {
        return Err(IngestError::Malformed(
            "RFC3164 hostname or tag empty".to_owned(),
        ));
    }
    let severity = match pri % 8 {
        0..=2 => Severity::Critical,
        3 => Severity::Error,
        4 => Severity::Warn,
        5..=6 => Severity::Info,
        _ => Severity::Debug,
    };
    Ok(event(
        timestamp,
        host.to_owned(),
        service.trim_end_matches(|c: char| c == '[' || c.is_ascii_digit() || c == ']'),
        message.trim().to_owned(),
        severity,
        line,
    ))
}

fn split_access_fields(line: &str) -> Result<Vec<String>, IngestError> {
    let mut fields = Vec::new();
    let mut chars = line.chars().peekable();
    while fields.len() < 10 {
        while chars.peek().is_some_and(|c| c.is_whitespace()) {
            chars.next();
        }
        let Some(first) = chars.next() else {
            break;
        };
        if first == '"' || first == '[' {
            let end = if first == '"' { '"' } else { ']' };
            let mut value = String::new();
            let mut escaped = false;
            let mut closed = false;
            for c in chars.by_ref() {
                if escaped {
                    value.push(c);
                    escaped = false;
                } else if c == '\\' {
                    escaped = true;
                } else if c == end {
                    closed = true;
                    break;
                } else {
                    value.push(c);
                }
            }
            if !closed || escaped {
                return Err(IngestError::Malformed(
                    "unterminated quoted access-log field".to_owned(),
                ));
            }
            fields.push(value);
        } else {
            let mut value = String::from(first);
            while chars.peek().is_some_and(|c| !c.is_whitespace()) {
                value.push(chars.next().unwrap());
            }
            fields.push(value);
        }
    }
    if fields.len() < 7 {
        return Err(IngestError::Malformed(
            "access-log record has too few fields".to_owned(),
        ));
    }
    Ok(fields)
}

pub fn parse_combined_access(raw: &str, host: &str) -> Result<NormalizedEvent, IngestError> {
    let line = raw.trim();
    if line.len() > crate::ingest::MAX_LINE_BYTES {
        return Err(IngestError::Oversized);
    }
    let fields = split_access_fields(line)?;
    let timestamp = DateTime::parse_from_str(&fields[3], "%d/%b/%Y:%H:%M:%S %z")
        .map_err(|_| IngestError::Malformed("access-log timestamp invalid".to_owned()))?
        .with_timezone(&Utc)
        .to_rfc3339();
    let status: u16 = fields[5]
        .parse()
        .map_err(|_| IngestError::Malformed("access-log status invalid".to_owned()))?;
    let severity = match status {
        500..=599 => Severity::Error,
        400..=499 => Severity::Warn,
        _ => Severity::Info,
    };
    let message = format!("{} {} {}", fields[0], fields[4], status);
    let structured = json!({"remote_addr": fields[0], "user": fields[2], "request": fields[4], "status": status, "bytes": fields[6], "referer": fields.get(7), "user_agent": fields.get(8)}).to_string();
    let mut normalized = event(timestamp, host.to_owned(), "http", message, severity, line);
    normalized.fields_json = structured;
    Ok(normalized)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_rfc3164_priority_timezone_and_tag_pid() {
        let parsed =
            parse_rfc3164("<34>Oct 11 22:14:15 host app[42]: started", 2026, 3600).unwrap();
        assert_eq!(parsed.host, "host");
        assert_eq!(parsed.service, "app");
        assert!(parsed.timestamp_utc.starts_with("2026-10-11T21:14:15"));
    }

    #[test]
    fn parses_combined_access_record_with_quoted_fields() {
        let parsed = parse_combined_access("192.0.2.1 - alice [11/Oct/2026:22:14:15 +0200] \"GET /login HTTP/1.1\" 401 17 \"-\" \"client agent\"", "web-01").unwrap();
        assert_eq!(parsed.severity, Severity::Warn);
        assert_eq!(parsed.host, "web-01");
        assert!(parsed.fields_json.contains("client agent"));
    }

    #[test]
    fn arbitrary_truncated_inputs_never_panic() {
        for parser in [0, 1] {
            for len in 0..256 {
                let raw =
                    String::from_utf8_lossy(&(0..len).map(|i| (i * 31) as u8).collect::<Vec<_>>())
                        .into_owned();
                let result = std::panic::catch_unwind(|| {
                    if parser == 0 {
                        let _ = parse_rfc3164(&raw, 2026, 0);
                    } else {
                        let _ = parse_combined_access(&raw, "host");
                    }
                });
                assert!(result.is_ok());
            }
        }
    }
}
