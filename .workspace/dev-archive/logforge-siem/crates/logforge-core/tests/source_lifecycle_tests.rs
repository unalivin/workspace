use logforge_core::{Database, SiemService};

#[test]
fn disabling_source_blocks_new_ingest_without_removing_existing_data() {
    let db = Database::open_in_memory().unwrap();
    let service = SiemService::new(db);
    let source = service.add_source("lifecycle", "file", "memory").unwrap();
    service
        .ingest_lines(source, &["2026-11-21T10:00:00Z host app: preserved"])
        .unwrap();
    assert!(service.set_source_enabled(source, false).unwrap());
    assert!(service
        .ingest_lines(source, &["2026-11-21T10:01:00Z host app: blocked"])
        .is_err());
    assert_eq!(service.overview_counts().unwrap().total_events, 1);
    assert!(service.set_source_enabled(source, true).unwrap());
    service
        .ingest_lines(source, &["2026-11-21T10:01:00Z host app: accepted"])
        .unwrap();
    assert_eq!(service.overview_counts().unwrap().total_events, 2);
}

#[test]
fn removing_source_cascades_its_events_and_alerts() {
    let db = Database::open_in_memory().unwrap();
    let service = SiemService::new(db);
    let source = service.add_source("remove-me", "file", "memory").unwrap();
    service.add_rule("error", "error", "failed").unwrap();
    service
        .ingest_lines(source, &["2026-11-21T10:00:00Z host app: failed request"])
        .unwrap();
    assert_eq!(service.list_alerts(None).unwrap().len(), 1);
    assert!(service.remove_source(source).unwrap());
    assert_eq!(service.list_alerts(None).unwrap().len(), 0);
    assert_eq!(service.overview_counts().unwrap().total_events, 0);
    assert!(!service.remove_source(source).unwrap());
}
