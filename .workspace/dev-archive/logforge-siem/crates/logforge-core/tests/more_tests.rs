use logforge_core::ingest::{parse_line, redact_secrets, IngestError, MAX_LINE_BYTES};
use logforge_core::model::Severity;
use logforge_core::{Database, SiemService};

#[test]
fn test_alert_acknowledge_and_resolve() {
    let db = Database::open_in_memory().expect("db");
    let svc = SiemService::new(db);
    let sid = svc.add_source("s1", "file", "/tmp/a.log").expect("src");
    svc.add_rule("r1", "critical", "breach").expect("rule");
    svc.ingest_lines(sid, &["2026-11-21T01:00:00Z h1 app: breach detected"])
        .expect("ingest");
    let alerts = svc.list_alerts(Some("open")).expect("alerts");
    assert_eq!(alerts.len(), 1);
    svc.update_alert_status(alerts[0].id, "acknowledged")
        .expect("ack");
    assert_eq!(
        svc.list_alerts(Some("acknowledged"))
            .expect("ack list")
            .len(),
        1
    );
    svc.update_alert_status(alerts[0].id, "resolved")
        .expect("res");
    assert_eq!(
        svc.list_alerts(Some("resolved")).expect("res list").len(),
        1
    );
}

#[test]
fn test_fts_search_multiple_terms() {
    let db = Database::open_in_memory().expect("db");
    let svc = SiemService::new(db);
    let sid = svc.add_source("s1", "file", "/tmp/a.log").expect("src");
    svc.ingest_lines(
        sid,
        &[
            "2026-11-21T01:00:00Z web-01 nginx: 502 bad gateway",
            "2026-11-21T01:01:00Z db-01 postgres: checkpoint complete",
        ],
    )
    .expect("ingest");
    assert_eq!(svc.search_events("gateway", 10).expect("search").len(), 1);
    assert_eq!(svc.search_events("postgres", 10).expect("search").len(), 1);
    assert_eq!(svc.search_events("", 10).expect("all").len(), 2);
}

#[test]
fn test_saved_query_upsert() {
    let db = Database::open_in_memory().expect("db");
    let svc = SiemService::new(db);
    svc.save_query("errors", "severity:error").expect("save 1");
    svc.save_query("errors", "severity:critical")
        .expect("save 2");
    let list = svc.list_saved_queries().expect("list");
    assert_eq!(list.len(), 1);
    assert_eq!(list[0].query, "severity:critical");
}

#[test]
fn test_audit_chain_records_entries() {
    let db = Database::open_in_memory().expect("db");
    let svc = SiemService::new(db);
    svc.record_audit("alice", "login", "session").expect("a1");
    svc.record_audit("bob", "export", "report.json")
        .expect("a2");
    let count: i64 = svc
        .db()
        .conn()
        .query_row("SELECT COUNT(*) FROM audit_log", [], |r| r.get(0))
        .expect("count");
    assert_eq!(count, 2);
}

#[test]
fn test_redact_multiple_secrets_in_one_line() {
    let (out, flag) = redact_secrets("password=p1 api_key=k1 token=t1 Bearer b1");
    assert!(flag);
    assert!(!out.contains("p1"));
    assert!(!out.contains("k1"));
    assert!(!out.contains("t1"));
    assert!(!out.contains("b1"));
}

#[test]
fn test_parse_json_with_default_fields() {
    let ev = parse_line(r#"{"message":"hello world"}"#).expect("json");
    assert_eq!(ev.host, "unknown");
    assert_eq!(ev.service, "unknown");
    assert_eq!(ev.severity, Severity::Info);
}

#[test]
fn test_parse_line_at_exact_max_size() {
    let prefix = "2026-11-21T00:00:00Z h s: ";
    let pad = "a".repeat(MAX_LINE_BYTES - prefix.len());
    let line = format!("{prefix}{pad}");
    assert!(parse_line(&line).is_ok());
}

#[test]
fn test_parse_line_one_over_max_size() {
    let prefix = "2026-11-21T00:00:00Z h s: ";
    let pad = "a".repeat(MAX_LINE_BYTES - prefix.len() + 1);
    let line = format!("{prefix}{pad}");
    assert_eq!(parse_line(&line), Err(IngestError::Oversized));
}

#[test]
fn test_syslog_severity_inference() {
    let e1 = parse_line("2026-11-21T00:00:00Z h s: critical disk failure").expect("crit");
    assert_eq!(e1.severity, Severity::Critical);
    let e2 = parse_line("2026-11-21T00:00:00Z h s: error opening socket").expect("err");
    assert_eq!(e2.severity, Severity::Error);
    let e3 = parse_line("2026-11-21T00:00:00Z h s: warn high latency").expect("warn");
    assert_eq!(e3.severity, Severity::Warn);
}
