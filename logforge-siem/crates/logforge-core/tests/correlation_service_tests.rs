use logforge_core::{Database, SiemService};

#[test]
fn ingested_events_drive_persistent_threshold_correlation_alerts() {
    let db = Database::open_in_memory().unwrap();
    let service = SiemService::new(db);
    let source = service.add_source("threshold", "file", "memory").unwrap();
    let rule = service.add_rule("Repeated login failure", "critical", "login failed").unwrap();
    service.add_threshold_correlation(rule, "host", 600, 2).unwrap();
    service.ingest_lines(source, &["2026-11-21T10:00:00Z host-a sshd: login failed for alice"]).unwrap();
    assert_eq!(service.list_alerts(None).unwrap().len(), 1);
    service.ingest_lines(source, &["2026-11-21T10:01:00Z host-a sshd: login failed for bob"]).unwrap();
    let alerts = service.list_alerts(None).unwrap();
    assert_eq!(alerts.len(), 3);
    assert!(alerts.iter().any(|alert| alert.title.contains("Correlation: Repeated login failure")));
}

#[test]
fn sequence_correlation_requires_first_then_second_pattern_on_same_key() {
    let db = Database::open_in_memory().unwrap();
    let service = SiemService::new(db);
    let source = service.add_source("sequence", "file", "memory").unwrap();
    let rule = service.add_rule("Privilege escalation sequence", "critical", "never-match").unwrap();
    service.add_sequence_correlation(rule, "host", 900, "authentication accepted", "privilege escalated").unwrap();
    service.ingest_lines(source, &["2026-11-21T11:00:00Z host-a sshd: authentication accepted"]).unwrap();
    service.ingest_lines(source, &["2026-11-21T11:01:00Z host-b sudo: privilege escalated"]).unwrap();
    assert!(service.list_alerts(None).unwrap().is_empty());
    service.ingest_lines(source, &["2026-11-21T11:02:00Z host-a sudo: privilege escalated"]).unwrap();
    let alerts = service.list_alerts(None).unwrap();
    assert_eq!(alerts.len(), 1);
    assert!(alerts[0].title.contains("Correlation: Privilege escalation sequence"));
}
