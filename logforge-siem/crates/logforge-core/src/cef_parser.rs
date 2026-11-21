use crate::ingest::{
    compute_dedup_hash, normalize_timestamp, redact_secrets, IngestError, NormalizedEvent,
};
use crate::model::Severity;
use std::collections::BTreeMap;

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

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CefRecord {
    pub header: CefHeader,
    pub extensions: BTreeMap<String, String>,
}

pub fn parse_cef_line(raw: &str) -> Result<NormalizedEvent, IngestError> {
    let record = parse_cef_record(raw)?;
    let extensions = &record.extensions;
    let timestamp = extensions
        .get("rt")
        .or_else(|| extensions.get("start"))
        .or_else(|| extensions.get("end"))
        .map(String::as_str);
    let timestamp_utc = parse_cef_timestamp(timestamp)?;
    let host = first_value(extensions, &["dvchost", "dhost", "shost"])
        .unwrap_or(&record.header.device_vendor)
        .to_string();
    let service = format!("{} {}", record.header.device_product, record.header.device_version)
        .trim()
        .to_string();
    let extension_message = first_value(extensions, &["msg", "request", "reason"]);
    let message = match extension_message {
        Some(value) if !value.trim().is_empty() => format!("[{}] {}: {}", record.header.signature_id, record.header.name, value),
        _ => format!("[{}] {}", record.header.signature_id, record.header.name),
    };
    let severity = cef_severity(&record.header.severity_raw);
    let extension_json = serde_json::to_string(extensions)
        .map_err(|error| IngestError::Malformed(format!("invalid CEF extension data: {error}")))?;
    let normalized_message = redact_secrets(&message).0;
    let (safe_raw, redacted) = redact_secrets(raw.trim());
    let dedup_hash = compute_dedup_hash(&timestamp_utc, &host, &service, &normalized_message);
    Ok(NormalizedEvent {
        timestamp_utc,
        severity,
        host,
        service,
        message: normalized_message,
        raw_line: safe_raw,
        dedup_hash,
        redacted,
        fields_json: extension_json,
    })
}

pub fn parse_cef_record(raw: &str) -> Result<CefRecord, IngestError> {
    let trimmed = raw.trim();
    if !trimmed.starts_with("CEF:") {
        return Err(malformed("missing CEF: prefix"));
    }
    let (safe_line, _) = redact_secrets(trimmed);
    let mut fields = split_header(&safe_line)?;
    if fields.len() != 7 {
        return Err(malformed("CEF requires exactly seven pipe-delimited header fields"));
    }
    let version = fields[0]
        .strip_prefix("CEF:")
        .and_then(|value| value.parse::<u8>().ok())
        .ok_or_else(|| malformed("invalid CEF version"))?;
    if version > 10 {
        return Err(malformed("unsupported CEF version"));
    }
    let extension = fields.pop().expect("validated field count");
    let severity_raw = fields.pop().expect("validated field count");
    let name = fields.pop().expect("validated field count");
    let signature_id = fields.pop().expect("validated field count");
    let device_version = fields.pop().expect("validated field count");
    let device_product = fields.pop().expect("validated field count");
    let device_vendor = fields.pop().expect("validated field count");
    let extensions = parse_extensions(&extension)?;
    Ok(CefRecord {
        header: CefHeader {
            version,
            device_vendor,
            device_product,
            device_version,
            signature_id,
            name,
            severity_raw,
        },
        extensions,
    })
}

fn split_header(line: &str) -> Result<Vec<String>, IngestError> {
    let mut output = Vec::with_capacity(7);
    let mut field = String::new();
    let mut escaped = false;
    for character in line.chars() {
        if escaped {
            field.push(decode_escape(character));
            escaped = false;
        } else if character == '\\' {
            escaped = true;
        } else if character == '|' && output.len() < 7 {
            output.push(std::mem::take(&mut field));
        } else {
            field.push(character);
        }
    }
    if escaped {
        field.push('\\');
    }
    output.push(field);
    if output.len() < 7 {
        return Err(malformed("CEF header has fewer than seven delimiters"));
    }
    let extension = output.split_off(7).join("|");
    output.push(extension);
    Ok(output)
}

fn decode_escape(character: char) -> char {
    match character {
        'n' => '\n',
        'r' => '\r',
        other => other,
    }
}

fn parse_extensions(input: &str) -> Result<BTreeMap<String, String>, IngestError> {
    let bytes = input.as_bytes();
    let mut fields = BTreeMap::new();
    let mut index = 0;
    while index < bytes.len() {
        while index < bytes.len() && bytes[index].is_ascii_whitespace() {
            index += 1;
        }
        if index == bytes.len() {
            break;
        }
        let key_start = index;
        while index < bytes.len() && (bytes[index].is_ascii_alphanumeric() || bytes[index] == b'_') {
            index += 1;
        }
        if key_start == index || index >= bytes.len() || bytes[index] != b'=' {
            return Err(malformed("invalid CEF extension key/value pair"));
        }
        let key = std::str::from_utf8(&bytes[key_start..index])
            .map_err(|_| malformed("CEF extension key is not UTF-8"))?
            .to_string();
        index += 1;
        let value_start = index;
        let mut escaped = false;
        while index < bytes.len() {
            if escaped {
                escaped = false;
                index += 1;
                continue;
            }
            if bytes[index] == b'\\' {
                escaped = true;
                index += 1;
                continue;
            }
            if bytes[index].is_ascii_whitespace() {
                let mut probe = index;
                while probe < bytes.len() && bytes[probe].is_ascii_whitespace() {
                    probe += 1;
                }
                let candidate_start = probe;
                while probe < bytes.len() && (bytes[probe].is_ascii_alphanumeric() || bytes[probe] == b'_') {
                    probe += 1;
                }
                if probe > candidate_start && probe < bytes.len() && bytes[probe] == b'=' {
                    break;
                }
            }
            index += 1;
        }
        let value = std::str::from_utf8(&bytes[value_start..index])
            .map_err(|_| malformed("CEF extension value is not UTF-8"))?;
        let value = unescape_extension(value);
        if fields.insert(key, value).is_some() {
            return Err(malformed("duplicate CEF extension key"));
        }
    }
    Ok(fields)
}

fn unescape_extension(input: &str) -> String {
    let mut output = String::with_capacity(input.len());
    let mut characters = input.chars();
    while let Some(character) = characters.next() {
        if character == '\\' {
            if let Some(next) = characters.next() {
                output.push(decode_escape(next));
            } else {
                output.push('\\');
            }
        } else {
            output.push(character);
        }
    }
    output
}

fn first_value<'a>(fields: &'a BTreeMap<String, String>, keys: &[&str]) -> Option<&'a String> {
    keys.iter().find_map(|key| fields.get(*key))
}

fn cef_severity(raw: &str) -> Severity {
    match raw.parse::<u8>().unwrap_or(3).min(10) {
        0..=3 => Severity::Info,
        4..=6 => Severity::Warn,
        7..=8 => Severity::Error,
        _ => Severity::Critical,
    }
}

fn parse_cef_timestamp(value: Option<&str>) -> Result<String, IngestError> {
    let Some(value) = value else { return normalize_timestamp(None) };
    if let Ok(milliseconds) = value.parse::<i64>() {
        let seconds = milliseconds.div_euclid(1000);
        let nanos = milliseconds.rem_euclid(1000) as u32 * 1_000_000;
        let date = chrono::DateTime::from_timestamp(seconds, nanos)
            .ok_or_else(|| malformed("CEF epoch timestamp is out of range"))?;
        return Ok(date.to_rfc3339());
    }
    normalize_timestamp(Some(value))
}

fn malformed(message: &str) -> IngestError {
    IngestError::Malformed(message.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decodes_escaped_header_fields_and_multitoken_extensions() {
        let line = r"CEF:0|Vendor|Product|2|sig\|42|Blocked\| login|9|rt=2026-11-21T10:00:00Z dvchost=edge-1 msg=login failed\| user src=192.0.2.1 cs1Label=tenant cs1=blue";
        let event = parse_cef_line(line).unwrap();
        assert_eq!(event.severity, Severity::Critical);
        assert_eq!(event.host, "edge-1");
        assert!(event.message.contains("login failed| user"));
        let fields: BTreeMap<String, String> = serde_json::from_str(&event.fields_json).unwrap();
        assert_eq!(fields.get("cs1"), Some(&"blue".to_string()));
    }

    #[test]
    fn parses_epoch_time_and_rejects_duplicate_extension_keys() {
        let event = parse_cef_line("CEF:0|V|P|1|id|notice|3|rt=1795255200000 msg=ok").unwrap();
        assert!(event.timestamp_utc.starts_with("2026-11-21T10:00:00"));
        assert!(parse_cef_line("CEF:0|V|P|1|id|notice|3|src=1 src=2").is_err());
    }
}
