use logforge_core::{Database, SiemService};

#[test]
fn ingested_events_drive_persistent_threshold_correlation_alerts() {
    let db = Database::open_in_memory().unwrap();
    let service = SiemService::new(db);
    let source = service.add_source("threshold", "file", "memory").unwrap();
    let rule = service
        .add_rule("Repeated login failure", "critical", "login failed")
        .unwrap();
    service
        .add_threshold_correlation(rule, "host", 600, 2)
        .unwrap();
    let base = chrono::Utc::now();
    let first_line = format!("{} host-a sshd: login failed for alice", base.to_rfc3339());
    service.ingest_lines(source, &[&first_line]).unwrap();
    assert_eq!(service.list_alerts(None).unwrap().len(), 1);
    let second_line = format!(
        "{} host-a sshd: login failed for bob",
        (base + chrono::Duration::seconds(60)).to_rfc3339()
    );
    service.ingest_lines(source, &[&second_line]).unwrap();
    let alerts = service.list_alerts(None).unwrap();
    assert_eq!(alerts.len(), 3);
    assert!(alerts
        .iter()
        .any(|alert| alert.title.contains("Correlation: Repeated login failure")));
}

#[test]
fn sequence_correlation_requires_first_then_second_pattern_on_same_key() {
    let db = Database::open_in_memory().unwrap();
    let service = SiemService::new(db);
    let source = service.add_source("sequence", "file", "memory").unwrap();
    let rule = service
        .add_rule("Privilege escalation sequence", "critical", "never-match")
        .unwrap();
    service
        .add_sequence_correlation(
            rule,
            "host",
            900,
            "authentication accepted",
            "privilege escalated",
        )
        .unwrap();
    let base = chrono::Utc::now();
    let first = format!("{} host-a sshd: authentication accepted", base.to_rfc3339());
    let wrong_host = format!(
        "{} host-b sudo: privilege escalated",
        (base + chrono::Duration::seconds(30)).to_rfc3339()
    );
    service.ingest_lines(source, &[&first]).unwrap();
    service.ingest_lines(source, &[&wrong_host]).unwrap();
    assert!(service.list_alerts(None).unwrap().is_empty());
    let second = format!(
        "{} host-a sudo: privilege escalated",
        (base + chrono::Duration::seconds(60)).to_rfc3339()
    );
    service.ingest_lines(source, &[&second]).unwrap();
    let alerts = service.list_alerts(None).unwrap();
    assert_eq!(alerts.len(), 1);
    assert!(alerts[0]
        .title
        .contains("Correlation: Privilege escalation sequence"));
}
