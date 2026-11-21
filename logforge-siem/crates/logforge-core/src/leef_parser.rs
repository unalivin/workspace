use crate::ingest::{
    compute_dedup_hash, normalize_timestamp, redact_secrets, IngestError, NormalizedEvent,
};
use crate::model::Severity;
use std::collections::BTreeMap;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LeefRecord {
    pub version: String,
    pub vendor: String,
    pub product: String,
    pub product_version: String,
    pub event_id: String,
    pub delimiter: char,
    pub attributes: BTreeMap<String, String>,
}

pub fn parse_leef_line(raw: &str) -> Result<NormalizedEvent, IngestError> {
    let record = parse_leef_record(raw)?;
    let attributes = &record.attributes;
    let timestamp = attributes
        .get("devTime")
        .or_else(|| attributes.get("timestamp"))
        .or_else(|| attributes.get("rt"))
        .map(String::as_str);
    let timestamp_utc = parse_timestamp(timestamp)?;
    let host = first_attribute(attributes, &["devName", "hostname", "srcHostName"])
        .unwrap_or(&record.vendor)
        .to_string();
    let service = record.product.clone();
    let message = first_attribute(attributes, &["msg", "message", "cat"])
        .map(|text| format!("[{}] {}", record.event_id, text))
        .unwrap_or_else(|| format!("[{}] {}", record.event_id, record.product));
    let severity = leef_severity(first_attribute(attributes, &["sev", "severity"]));
    let (safe_raw, redacted) = redact_secrets(raw.trim());
    let message = redact_secrets(&message).0;
    let fields_json = serde_json::to_string(attributes)
        .map_err(|error| IngestError::Malformed(format!("invalid LEEF extension data: {error}")))?;
    let dedup_hash = compute_dedup_hash(&timestamp_utc, &host, &service, &message);
    Ok(NormalizedEvent {
        timestamp_utc,
        severity,
        host,
        service,
        message,
        raw_line: safe_raw,
        dedup_hash,
        redacted,
        fields_json,
    })
}

pub fn parse_leef_record(raw: &str) -> Result<LeefRecord, IngestError> {
    let (safe_line, _) = redact_secrets(raw.trim());
    let parts: Vec<&str> = safe_line.splitn(7, '|').collect();
    if parts.len() != 7 || !parts[0].starts_with("LEEF:") {
        return Err(malformed(
            "LEEF requires version, vendor, product, event, delimiter, and attributes",
        ));
    }
    let version = parts[0].trim_start_matches("LEEF:");
    if !matches!(version, "1.0" | "2.0") {
        return Err(malformed("unsupported LEEF version"));
    }
    let delimiter = decode_delimiter(parts[5])?;
    let attributes = parse_attributes(parts[6], delimiter)?;
    Ok(LeefRecord {
        version: version.to_string(),
        vendor: parts[1].to_string(),
        product: parts[2].to_string(),
        product_version: parts[3].to_string(),
        event_id: parts[4].to_string(),
        delimiter,
        attributes,
    })
}

fn decode_delimiter(value: &str) -> Result<char, IngestError> {
    match value {
        "\\t" => Ok('\t'),
        "\\n" => Ok('\n'),
        "^" => Ok('^'),
        "|" => Ok('|'),
        value if value.chars().count() == 1 => Ok(value.chars().next().expect("one character")),
        _ => Err(malformed("invalid LEEF extension delimiter")),
    }
}

fn parse_attributes(raw: &str, delimiter: char) -> Result<BTreeMap<String, String>, IngestError> {
    let mut attributes = BTreeMap::new();
    let mut pair = String::new();
    let mut escaped = false;
    let mut pairs = Vec::new();
    for character in raw.chars() {
        if escaped {
            pair.push(match character {
                'n' => '\n',
                'r' => '\r',
                't' => '\t',
                other => other,
            });
            escaped = false;
        } else if character == '\\' {
            escaped = true;
        } else if character == delimiter {
            pairs.push(std::mem::take(&mut pair));
        } else {
            pair.push(character);
        }
    }
    if escaped {
        pair.push('\\');
    }
    pairs.push(pair);
    for pair in pairs {
        if pair.is_empty() {
            continue;
        }
        let (key, value) = pair
            .split_once('=')
            .ok_or_else(|| malformed("LEEF attributes must be key=value pairs"))?;
        let key = key.trim();
        if key.is_empty()
            || !key
                .chars()
                .all(|character| character.is_ascii_alphanumeric() || character == '_')
        {
            return Err(malformed("invalid LEEF attribute name"));
        }
        if attributes
            .insert(key.to_string(), value.to_string())
            .is_some()
        {
            return Err(malformed("duplicate LEEF attribute"));
        }
    }
    Ok(attributes)
}

fn first_attribute<'a>(
    attributes: &'a BTreeMap<String, String>,
    keys: &[&str],
) -> Option<&'a String> {
    keys.iter().find_map(|key| attributes.get(*key))
}

fn parse_timestamp(value: Option<&str>) -> Result<String, IngestError> {
    let Some(value) = value else {
        return normalize_timestamp(None);
    };
    if let Ok(milliseconds) = value.parse::<i64>() {
        let seconds = milliseconds.div_euclid(1000);
        let nanos = milliseconds.rem_euclid(1000) as u32 * 1_000_000;
        let timestamp = chrono::DateTime::from_timestamp(seconds, nanos)
            .ok_or_else(|| malformed("LEEF epoch timestamp is out of range"))?;
        return Ok(timestamp.to_rfc3339());
    }
    normalize_timestamp(Some(value))
}

fn leef_severity(value: Option<&String>) -> Severity {
    match value
        .and_then(|severity| severity.parse::<u8>().ok())
        .unwrap_or(2)
        .min(10)
    {
        0..=2 => Severity::Info,
        3..=4 => Severity::Warn,
        5..=7 => Severity::Error,
        _ => Severity::Critical,
    }
}

fn malformed(message: &str) -> IngestError {
    IngestError::Malformed(message.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_leef_attributes_delimiter_timestamp_severity_and_host() {
        let raw = "LEEF:2.0|Vendor|Firewall|3.1|evt-4|\\t|devTime=2026-11-21T10:00:00Z\tdevName=edge-7\tsev=9\tmsg=Blocked\\trequest";
        let event = parse_leef_line(raw).unwrap();
        assert_eq!(event.host, "edge-7");
        assert_eq!(event.severity, Severity::Critical);
        assert!(event.message.contains("Blocked\trequest"));
        assert!(event.fields_json.contains("devName"));
    }

    #[test]
    fn rejects_invalid_versions_and_duplicate_keys() {
        assert!(parse_leef_line("LEEF:9.0|V|P|1|E|\\t|sev=5").is_err());
        assert!(parse_leef_line("LEEF:2.0|V|P|1|E|^|sev=3^sev=4").is_err());
    }
}
