use logforge_core::case_export::export_case_bundle;
use logforge_core::cases::CaseManager;
use logforge_core::{Database, SiemService};

#[test]
fn case_bundle_contains_related_alerts_events_notes_and_audit_status() {
    let db = Database::open_in_memory().expect("database");
    let service = SiemService::new(db);
    service.seed_demo_data().expect("seed");
    let manager = CaseManager::new(service.db());
    let case_id = manager
        .create(
            "SSH investigation",
            "Review repeated login failures",
            "high",
            "analyst",
        )
        .expect("case");
    let alert = service.list_alerts(None).expect("alerts").remove(0);
    manager.link_alert(case_id, alert.id).expect("link alert");
    manager
        .add_note(case_id, "analyst", "Preserve source event for review")
        .expect("note");

    let bundle = export_case_bundle(service.db(), case_id)
        .expect("export")
        .expect("case exists");
    assert_eq!(bundle.alerts.len(), 1);
    assert_eq!(bundle.events.len(), 1);
    assert_eq!(bundle.notes.len(), 1);
    assert!(bundle.audit_chain.valid);
    assert_eq!(bundle.case.id, case_id);
}

#[test]
fn exporting_unknown_case_returns_none() {
    let db = Database::open_in_memory().expect("database");
    assert!(export_case_bundle(&db, 99).expect("export").is_none());
}
