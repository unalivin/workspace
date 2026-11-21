use crate::model::{Alert, LogEvent};
use std::io::{self, Write};

pub fn export_events_json(events: &[LogEvent]) -> Result<String, serde_json::Error> {
    serde_json::to_string_pretty(events)
}

pub fn export_alerts_json(alerts: &[Alert]) -> Result<String, serde_json::Error> {
    serde_json::to_string_pretty(alerts)
}

/// Writes normalized event fields as RFC 4180-compatible CSV. Every text field
/// is quoted and embedded quote marks are doubled, including host and service.
pub fn write_events_csv<W: Write>(events: &[LogEvent], writer: &mut W) -> io::Result<usize> {
    writer.write_all(b"id,source_id,timestamp_utc,received_at,severity,host,service,message,redacted\r\n")?;
    for event in events {
        let row = [
            event.id.to_string(),
            event.source_id.to_string(),
            event.timestamp_utc.clone(),
            event.received_at.clone(),
            event.severity.to_string(),
            event.host.clone(),
            event.service.clone(),
            event.message.clone(),
            event.redacted.to_string(),
        ];
        for (index, field) in row.iter().enumerate() {
            if index > 0 {
                writer.write_all(b",")?;
            }
            write_csv_field(writer, field)?;
        }
        writer.write_all(b"\r\n")?;
    }
    Ok(events.len())
}

/// Writes alert data as a CSV report, including status and workflow timestamps.
pub fn write_alerts_csv<W: Write>(alerts: &[Alert], writer: &mut W) -> io::Result<usize> {
    writer.write_all(b"id,rule_id,event_id,severity,status,title,case_ref,created_at,updated_at\r\n")?;
    for alert in alerts {
        let row = [
            alert.id.to_string(),
            alert.rule_id.to_string(),
            alert.event_id.to_string(),
            alert.severity.clone(),
            alert.status.clone(),
            alert.title.clone(),
            alert.case_ref.clone(),
            alert.created_at.clone(),
            alert.updated_at.clone(),
        ];
        for (index, field) in row.iter().enumerate() {
            if index > 0 {
                writer.write_all(b",")?;
            }
            write_csv_field(writer, field)?;
        }
        writer.write_all(b"\r\n")?;
    }
    Ok(alerts.len())
}

fn write_csv_field<W: Write>(writer: &mut W, field: &str) -> io::Result<()> {
    writer.write_all(b"\"")?;
    let escaped = field.replace('"', "\"\"");
    writer.write_all(escaped.as_bytes())?;
    writer.write_all(b"\"")
}

pub fn export_events_csv(events: &[LogEvent]) -> String {
    let mut output = Vec::new();
    write_events_csv(events, &mut output).expect("writing to Vec cannot fail");
    String::from_utf8(output).expect("CSV output is valid UTF-8")
}
