use crate::model::{Alert, LogEvent};

pub fn export_events_json(events: &[LogEvent]) -> Result<String, serde_json::Error> {
    serde_json::to_string_pretty(events)
}

pub fn export_alerts_json(alerts: &[Alert]) -> Result<String, serde_json::Error> {
    serde_json::to_string_pretty(alerts)
}

pub fn export_events_csv(events: &[LogEvent]) -> String {
    let mut out = String::from("id,timestamp_utc,severity,host,service,message,redacted\n");
    for ev in events {
        let msg = ev.message.replace('"', "\"\"");
        out.push_str(&format!(
            "{},{},{},{},{},\"{}\",{}\n",
            ev.id, ev.timestamp_utc, ev.severity, ev.host, ev.service, msg, ev.redacted
        ));
    }
    out
}
