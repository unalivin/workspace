use logforge_core::correlation::correlate_by_host;
use logforge_core::diagnostics::collect_diagnostics;
use logforge_core::ingest::{compute_dedup_hash, normalize_timestamp, parse_line, redact_secrets};
use logforge_core::model::{LogEvent, Severity};
use logforge_core::{Database, SiemService};

#[test]
fn test_diagnostics_report_fields() {
    let db = Database::open_in_memory().expect("db");
    let report = collect_diagnostics(&db).expect("diag");
    assert_eq!(report.app_name, "LogForge-SIEM");
    assert_eq!(report.version, "0.1.0-dev");
    assert_eq!(report.schema_version, 7);
}

#[test]
fn test_correlate_by_host_threshold() {
    let events = vec![
        LogEvent {
            id: 1,
            source_id: 1,
            timestamp_utc: "2026-11-21T00:00:00Z".to_string(),
            received_at: "2026-11-21T00:00:00Z".to_string(),
            severity: "warn".to_string(),
            host: "web-01".to_string(),
            service: "sshd".to_string(),
            message: "fail".to_string(),
            raw_line: "raw".to_string(),
            dedup_hash: "h1".to_string(),
            redacted: false,
            fields_json: "{}".to_string(),
        },
        LogEvent {
            id: 2,
            source_id: 1,
            timestamp_utc: "2026-11-21T00:00:01Z".to_string(),
            received_at: "2026-11-21T00:00:01Z".to_string(),
            severity: "warn".to_string(),
            host: "web-01".to_string(),
            service: "sshd".to_string(),
            message: "fail".to_string(),
            raw_line: "raw".to_string(),
            dedup_hash: "h2".to_string(),
            redacted: false,
            fields_json: "{}".to_string(),
        },
    ];
    let hits = correlate_by_host(&events, 2);
    assert_eq!(hits.len(), 1);
    assert_eq!(hits[0].host, "web-01");
    assert_eq!(hits[0].count, 2);
}

#[test]
fn test_correlate_by_host_empty() {
    assert!(correlate_by_host(&[], 1).is_empty());
}

#[test]
fn test_ingest_multiple_sources() {
    let db = Database::open_in_memory().expect("db");
    let svc = SiemService::new(db);
    let s1 = svc.add_source("s1", "file", "/tmp/1.log").expect("s1");
    let s2 = svc.add_source("s2", "file", "/tmp/2.log").expect("s2");
    svc.ingest_lines(s1, &["2026-11-21T00:00:00Z h1 a: m1"])
        .expect("i1");
    svc.ingest_lines(s2, &["2026-11-21T00:00:01Z h2 b: m2"])
        .expect("i2");
    assert_eq!(svc.overview_counts().expect("c").total_events, 2);
}

#[test]
fn test_redact_preserves_surrounding_text() {
    let (out, flag) = redact_secrets("before password=123 after");
    assert!(flag);
    assert_eq!(out, "before password=[REDACTED] after");
}

#[test]
fn test_dedup_hash_differs_on_message() {
    let a = compute_dedup_hash("t", "h", "s", "m1");
    let b = compute_dedup_hash("t", "h", "s", "m2");
    assert_ne!(a, b);
}

#[test]
fn test_normalize_timestamp_none_returns_utc() {
    let ts = normalize_timestamp(None).expect("now");
    assert!(ts.contains('T'));
}

#[test]
fn test_parse_json_critical_severity() {
    let ev = parse_line(r#"{"severity":"critical","message":"kernel panic"}"#).expect("ok");
    assert_eq!(ev.severity, Severity::Critical);
}
