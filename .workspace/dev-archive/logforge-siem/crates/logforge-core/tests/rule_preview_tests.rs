use logforge_core::event_query::EventFilter;
use logforge_core::rule_preview::preview_detection_rules;
use logforge_core::{Database, SiemService};

#[test]
fn preview_reports_rule_hits_without_creating_alerts() {
    let db = Database::open_in_memory().expect("database");
    let service = SiemService::new(db);
    service.seed_demo_data().expect("seed");
    let before = service.list_alerts(None).expect("alerts before").len();
    let report = preview_detection_rules(
        &service,
        &EventFilter {
            limit: 100,
            ..EventFilter::default()
        },
    )
    .expect("preview");
    let after = service.list_alerts(None).expect("alerts after").len();
    assert_eq!(before, after);
    assert_eq!(report.examined_events, 3);
    assert!(report.matched_events > 0);
    assert!(report.total_matches >= report.matched_events);
    assert!(!report.rules.is_empty());
}

#[test]
fn preview_respects_filters_and_empty_results() {
    let db = Database::open_in_memory().expect("database");
    let service = SiemService::new(db);
    service.seed_demo_data().expect("seed");
    let report = preview_detection_rules(
        &service,
        &EventFilter {
            host: Some("unknown-host".into()),
            limit: 20,
            ..EventFilter::default()
        },
    )
    .expect("empty preview");
    assert_eq!(report.examined_events, 0);
    assert_eq!(report.matched_events, 0);
    assert!(report.rules.is_empty());
}
