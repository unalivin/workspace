use logforge_core::ingest::{compute_dedup_hash, normalize_timestamp, parse_line, redact_secrets, IngestError, MAX_LINE_BYTES};
use logforge_core::model::Severity;
use logforge_core::theme::{severity_color, ThemeMode, CATEGORICAL_DARK, CATEGORICAL_LIGHT};
use logforge_core::{Database, SiemService};

#[test]
fn test_severity_parse_debug() {
    assert_eq!(Severity::parse("debug"), Some(Severity::Debug));
}

#[test]
fn test_severity_parse_info() {
    assert_eq!(Severity::parse("info"), Some(Severity::Info));
}

#[test]
fn test_severity_parse_warn() {
    assert_eq!(Severity::parse("warn"), Some(Severity::Warn));
    assert_eq!(Severity::parse("warning"), Some(Severity::Warn));
}

#[test]
fn test_severity_parse_error() {
    assert_eq!(Severity::parse("error"), Some(Severity::Error));
    assert_eq!(Severity::parse("err"), Some(Severity::Error));
}

#[test]
fn test_severity_parse_critical() {
    assert_eq!(Severity::parse("critical"), Some(Severity::Critical));
    assert_eq!(Severity::parse("crit"), Some(Severity::Critical));
    assert_eq!(Severity::parse("alert"), Some(Severity::Critical));
    assert_eq!(Severity::parse("emerg"), Some(Severity::Critical));
}

#[test]
fn test_severity_parse_unknown() {
    assert_eq!(Severity::parse("unknown"), None);
    assert_eq!(Severity::parse(""), None);
}

#[test]
fn test_severity_as_str() {
    assert_eq!(Severity::Debug.as_str(), "debug");
    assert_eq!(Severity::Info.as_str(), "info");
    assert_eq!(Severity::Warn.as_str(), "warn");
    assert_eq!(Severity::Error.as_str(), "error");
    assert_eq!(Severity::Critical.as_str(), "critical");
}

#[test]
fn test_redact_password() {
    let (out, flag) = redact_secrets("user=alice password=secret123 host=web1");
    assert!(flag);
    assert!(out.contains("password=[REDACTED]"));
    assert!(!out.contains("secret123"));
}

#[test]
fn test_redact_api_key() {
    let (out, flag) = redact_secrets("api_key=abcdef123456");
    assert!(flag);
    assert!(out.contains("api_key=[REDACTED]"));
}

#[test]
fn test_redact_token() {
    let (out, flag) = redact_secrets("token=tok_999");
    assert!(flag);
    assert!(out.contains("token=[REDACTED]"));
}

#[test]
fn test_redact_bearer() {
    let (out, flag) = redact_secrets("Authorization: Bearer eyJhbGciOiJIUzI1NiJ9");
    assert!(flag);
    assert!(out.contains("Bearer [REDACTED]"));
}

#[test]
fn test_redact_no_secret() {
    let (out, flag) = redact_secrets("normal log line without secrets");
    assert!(!flag);
    assert_eq!(out, "normal log line without secrets");
}

#[test]
fn test_normalize_timestamp_utc() {
    let ts = normalize_timestamp(Some("2026-11-21T10:00:00Z")).expect("utc");
    assert!(ts.starts_with("2026-11-21T10:00:00"));
}

#[test]
fn test_normalize_timestamp_offset() {
    let ts = normalize_timestamp(Some("2026-11-21T12:00:00+02:00")).expect("offset");
    assert!(ts.starts_with("2026-11-21T10:00:00"));
}

#[test]
fn test_normalize_timestamp_invalid() {
    assert!(normalize_timestamp(Some("not-a-timestamp")).is_err());
}

#[test]
fn test_dedup_hash_deterministic() {
    let h1 = compute_dedup_hash("2026-11-21T10:00:00Z", "web-01", "sshd", "msg");
    let h2 = compute_dedup_hash("2026-11-21T10:00:00Z", "web-01", "sshd", "msg");
    assert_eq!(h1, h2);
    assert_eq!(h1.len(), 64);
}

#[test]
fn test_dedup_hash_differs_on_host() {
    let h1 = compute_dedup_hash("2026-11-21T10:00:00Z", "web-01", "sshd", "msg");
    let h2 = compute_dedup_hash("2026-11-21T10:00:00Z", "web-02", "sshd", "msg");
    assert_ne!(h1, h2);
}

#[test]
fn test_parse_empty_line() {
    assert_eq!(parse_line(""), Err(IngestError::Empty));
    assert_eq!(parse_line("   \t  "), Err(IngestError::Empty));
}

#[test]
fn test_parse_oversized_line() {
    let big = "x".repeat(MAX_LINE_BYTES + 1);
    assert_eq!(parse_line(&big), Err(IngestError::Oversized));
}

#[test]
fn test_parse_json_missing_message() {
    let line = r#"{"timestamp":"2026-11-21T10:00:00Z","host":"web-01"}"#;
    assert!(matches!(parse_line(line), Err(IngestError::Malformed(_))));
}

#[test]
fn test_parse_syslog_valid() {
    let line = "2026-11-21T10:00:00Z db-01 postgres: connection accepted";
    let ev = parse_line(line).expect("syslog");
    assert_eq!(ev.host, "db-01");
    assert_eq!(ev.service, "postgres");
    assert_eq!(ev.severity, Severity::Info);
}

#[test]
fn test_theme_mode_roundtrip() {
    for mode in [ThemeMode::Light, ThemeMode::Dark, ThemeMode::System, ThemeMode::HighContrast] {
        assert_eq!(ThemeMode::parse(mode.as_str()), Some(mode));
    }
}

#[test]
fn test_categorical_palettes_have_eight_colors() {
    assert_eq!(CATEGORICAL_LIGHT.len(), 8);
    assert_eq!(CATEGORICAL_DARK.len(), 8);
}

#[test]
fn test_severity_colors_light_and_dark() {
    assert_eq!(severity_color("critical", false), "#B3261E");
    assert_eq!(severity_color("critical", true), "#FF8A80");
}

#[test]
fn test_db_settings_roundtrip() {
    let db = Database::open_in_memory().expect("db");
    db.set_setting("theme", "dark", "2026-11-21T00:00:00Z").expect("set");
    assert_eq!(db.get_setting("theme").expect("get"), Some("dark".to_string()));
}

#[test]
fn test_service_source_and_rule_crud() {
    let db = Database::open_in_memory().expect("db");
    let svc = SiemService::new(db);
    let sid = svc.add_source("app-logs", "file", "/var/log/app.log").expect("source");
    assert!(sid > 0);
    let rid = svc.add_rule("Error spike", "error", "panic").expect("rule");
    assert!(rid > 0);
    assert_eq!(svc.list_sources().expect("sources").len(), 1);
    assert_eq!(svc.list_rules().expect("rules").len(), 1);
}
