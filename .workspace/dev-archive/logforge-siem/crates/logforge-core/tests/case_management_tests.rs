use logforge_core::cases::CaseManager;
use logforge_core::{Database, SiemService};

#[test]
fn manages_case_lifecycle_notes_assignment_and_alert_links_atomically() {
    let db = Database::open_in_memory().unwrap();
    let service = SiemService::new(db);
    let source = service.add_source("case-test", "file", "memory").unwrap();
    service.add_rule("failed login", "warn", "failed").unwrap();
    service
        .ingest_lines(source, &["2026-11-21T10:00:00Z host sshd: failed login"])
        .unwrap();
    let alert = service.list_alerts(None).unwrap().remove(0);
    let cases = CaseManager::new(service.db());
    let case_id = cases
        .create(
            "Suspicious login burst",
            "Review related authentication events",
            "high",
            "analyst",
        )
        .unwrap();
    cases.link_alert(case_id, alert.id).unwrap();
    cases
        .add_note(case_id, "analyst", "Correlate with VPN audit trail")
        .unwrap();
    assert_eq!(cases.get(case_id).unwrap().unwrap().alert_count, 1);
    assert_eq!(cases.alerts(case_id).unwrap().len(), 1);
    assert_eq!(cases.notes(case_id).unwrap()[0].author, "analyst");
    cases.assign(case_id, "on-call").unwrap();
    cases.set_status(case_id, "in_progress").unwrap();
    let current = cases.get(case_id).unwrap().unwrap();
    assert_eq!(current.status, "in_progress");
    assert_eq!(current.assignee, "on-call");
    cases.set_status(case_id, "closed").unwrap();
    assert!(cases.get(case_id).unwrap().unwrap().closed_at.is_some());
    assert!(cases.unlink_alert(case_id, alert.id).unwrap());
    assert_eq!(cases.get(case_id).unwrap().unwrap().alert_count, 0);
}

#[test]
fn case_validation_rejects_empty_titles_and_unknown_priorities() {
    let db = Database::open_in_memory().unwrap();
    let cases = CaseManager::new(&db);
    assert!(cases.create("  ", "", "low", "").is_err());
    assert!(cases.create("Case", "", "urgent", "").is_err());
    let id = cases.create("Case", "", "low", "").unwrap();
    assert!(cases.set_status(id, "triaged").is_err());
}
