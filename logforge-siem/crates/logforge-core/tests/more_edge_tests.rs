use logforge_core::ingest::{compute_dedup_hash, normalize_timestamp, parse_line, redact_secrets, IngestError};
use logforge_core::model::Severity;
use logforge_core::notifier::{is_quiet_hour, severity_rank};
use logforge_core::query_parser::parse_search_query;
use logforge_core::theme::{severity_color, ThemeMode};

#[test]
fn test_rank_ordering() {
    assert!(severity_rank("debug") < severity_rank("info"));
    assert!(severity_rank("info") < severity_rank("warn"));
    assert!(severity_rank("warn") < severity_rank("error"));
    assert!(severity_rank("error") < severity_rank("critical"));
}

#[test]
fn test_rank_unknown_is_zero() {
    assert_eq!(severity_rank("none"), 0);
}

#[test]
fn test_quiet_hour_wrap_midnight() {
    assert!(is_quiet_hour(0, 23, 5));
    assert!(is_quiet_hour(4, 23, 5));
    assert!(!is_quiet_hour(5, 23, 5));
}

#[test]
fn test_theme_mode_invalid() {
    assert_eq!(ThemeMode::parse("neon"), None);
}

#[test]
fn test_severity_color_defaults_to_info() {
    assert_eq!(severity_color("unknown", false), "#2B6CB0");
    assert_eq!(severity_color("unknown", true), "#7DB4F0");
}

#[test]
fn test_query_parser_only_host() {
    let q = parse_search_query("host:db-01");
    assert_eq!(q.host_filter.as_deref(), Some("db-01"));
    assert!(q.fts_terms.is_empty());
}

#[test]
fn test_query_parser_only_severity() {
    let q = parse_search_query("severity:WARN");
    assert_eq!(q.severity_filter.as_deref(), Some("warn"));
}

#[test]
fn test_redact_empty_string() {
    let (out, flag) = redact_secrets("");
    assert!(!flag);
    assert_eq!(out, "");
}

#[test]
fn test_dedup_hash_differs_on_service() {
    let a = compute_dedup_hash("t", "h", "s1", "m");
    let b = compute_dedup_hash("t", "h", "s2", "m");
    assert_ne!(a, b);
}

#[test]
fn test_normalize_timestamp_empty_returns_now() {
    let ts = normalize_timestamp(Some("")).expect("now");
    assert!(ts.contains('T'));
}

#[test]
fn test_parse_json_invalid_timestamp_rejected() {
    let line = r#"{"timestamp":"bad","message":"hi"}"#;
    assert!(matches!(parse_line(line), Err(IngestError::Malformed(_))));
}

#[test]
fn test_parse_syslog_trims_trailing_colon_on_service() {
    let ev = parse_line("2026-11-21T00:00:00Z web-01 nginx: started").expect("ok");
    assert_eq!(ev.service, "nginx");
}

#[test]
fn test_severity_parse_whitespace() {
    assert_eq!(Severity::parse("  info  "), Some(Severity::Info));
}
