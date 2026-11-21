use logforge_core::ingest::{parse_line, redact_secrets, IngestError};
use logforge_core::model::Severity;
use logforge_core::query_parser::parse_search_query;

#[test]
fn test_parse_syslog_with_colon_in_message() {
    let ev = parse_line("2026-11-21T00:00:00Z h1 svc: key: value").expect("ok");
    assert_eq!(ev.service, "svc");
    assert_eq!(ev.message, "key: value");
}

#[test]
fn test_parse_json_with_extra_fields() {
    let ev = parse_line(r#"{"message":"ok","pid":123,"user":"root"}"#).expect("ok");
    assert_eq!(ev.message, "ok");
    assert!(ev.fields_json.contains("123"));
}

#[test]
fn test_redact_secret_prefix() {
    let (out, flag) = redact_secrets("secret=mysecretvalue");
    assert!(flag);
    assert_eq!(out, "secret=[REDACTED]");
}

#[test]
fn test_redact_already_redacted() {
    let (out, flag) = redact_secrets("token=[REDACTED]");
    assert!(!flag);
    assert_eq!(out, "token=[REDACTED]");
}

#[test]
fn test_query_parser_empty() {
    let q = parse_search_query("   ");
    assert!(q.fts_terms.is_empty());
    assert!(q.severity_filter.is_none());
    assert!(q.host_filter.is_none());
}

#[test]
fn test_query_parser_strips_punctuation() {
    let q = parse_search_query("failed; DROP TABLE events;");
    assert_eq!(q.fts_terms, vec!["failed", "DROP", "TABLE", "events"]);
}

#[test]
fn test_parse_json_array_rejected() {
    assert!(matches!(parse_line("[1, 2, 3]"), Err(IngestError::Malformed(_))));
}

#[test]
fn test_parse_json_null_rejected() {
    assert!(matches!(parse_line("null"), Err(IngestError::Malformed(_))));
}

#[test]
fn test_severity_case_insensitive() {
    assert_eq!(Severity::parse("CRITICAL"), Some(Severity::Critical));
    assert_eq!(Severity::parse("Warning"), Some(Severity::Warn));
}
