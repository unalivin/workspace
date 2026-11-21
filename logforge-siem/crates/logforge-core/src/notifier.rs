use crate::db::{Database, DbError};
use crate::model::Severity;
use rusqlite::params;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NotificationRule {
    pub id: i64,
    pub event_type: String,
    pub min_severity: String,
    pub enabled: bool,
    pub quiet_start_hour: u8,
    pub quiet_end_hour: u8,
}

pub fn is_quiet_hour(hour: u8, start: u8, end: u8) -> bool {
    if start == end {
        return false;
    }
    if start < end {
        hour >= start && hour < end
    } else {
        hour >= start || hour < end
    }
}

pub fn severity_rank(s: &str) -> u8 {
    match Severity::parse(s) {
        Some(Severity::Debug) => 1,
        Some(Severity::Info) => 2,
        Some(Severity::Warn) => 3,
        Some(Severity::Error) => 4,
        Some(Severity::Critical) => 5,
        None => 0,
    }
}

pub fn should_notify(rule: &NotificationRule, severity: &str, current_hour_utc: u8) -> bool {
    if !rule.enabled {
        return false;
    }
    if is_quiet_hour(current_hour_utc, rule.quiet_start_hour, rule.quiet_end_hour) {
        return false;
    }
    severity_rank(severity) >= severity_rank(&rule.min_severity)
}

pub fn upsert_notification_rule(
    db: &Database,
    event_type: &str,
    min_severity: &str,
    enabled: bool,
    quiet_start_hour: u8,
    quiet_end_hour: u8,
) -> Result<(), DbError> {
    db.conn().execute(
        "INSERT INTO notification_rules (event_type, min_severity, enabled, quiet_start_hour, quiet_end_hour)
         VALUES (?1, ?2, ?3, ?4, ?5)
         ON CONFLICT(event_type) DO UPDATE SET
           min_severity = excluded.min_severity,
           enabled = excluded.enabled,
           quiet_start_hour = excluded.quiet_start_hour,
           quiet_end_hour = excluded.quiet_end_hour",
        params![
            event_type,
            min_severity,
            if enabled { 1 } else { 0 },
            quiet_start_hour as i64,
            quiet_end_hour as i64
        ],
    )?;
    Ok(())
}
