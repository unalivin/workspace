use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Severity {
    Debug,
    Info,
    Warn,
    Error,
    Critical,
}

impl Severity {
    pub fn as_str(&self) -> &'static str {
        match self {
            Severity::Debug => "debug",
            Severity::Info => "info",
            Severity::Warn => "warn",
            Severity::Error => "error",
            Severity::Critical => "critical",
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        match s.trim().to_ascii_lowercase().as_str() {
            "debug" => Some(Severity::Debug),
            "info" => Some(Severity::Info),
            "warn" | "warning" => Some(Severity::Warn),
            "error" | "err" => Some(Severity::Error),
            "critical" | "crit" | "alert" | "emerg" => Some(Severity::Critical),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Source {
    pub id: i64,
    pub name: String,
    pub kind: String,
    pub location: String,
    pub enabled: bool,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LogEvent {
    pub id: i64,
    pub source_id: i64,
    pub timestamp_utc: String,
    pub received_at: String,
    pub severity: String,
    pub host: String,
    pub service: String,
    pub message: String,
    pub raw_line: String,
    pub dedup_hash: String,
    pub redacted: bool,
    pub fields_json: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DetectionRule {
    pub id: i64,
    pub name: String,
    pub severity: String,
    pub pattern: String,
    pub enabled: bool,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Alert {
    pub id: i64,
    pub rule_id: i64,
    pub event_id: i64,
    pub severity: String,
    pub status: String,
    pub title: String,
    pub case_ref: String,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SavedQuery {
    pub id: i64,
    pub name: String,
    pub query: String,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OverviewCounts {
    pub total_events: i64,
    pub active_sources: i64,
    pub open_alerts: i64,
    pub stat_received: i64,
    pub stat_deduplicated: i64,
    pub stat_dropped: i64,
    pub stat_malformed: i64,
}
