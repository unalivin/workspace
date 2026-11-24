use logforge_core::alert_workflow::{bulk_update_alerts, AlertBulkFilter};
use logforge_core::{Database, SiemService};

#[test]
fn bulk_update_only_changes_alerts_matching_every_selector() {
    let db = Database::open_in_memory().expect("database");
    let service = SiemService::new(db);
    service.seed_demo_data().expect("seed");
    let alerts = service.list_alerts(None).expect("alerts");
    assert!(!alerts.is_empty());
    let id = alerts[0].id;
    let count = bulk_update_alerts(
        service.db(),
        &AlertBulkFilter {
            current_status: Some("open".into()),
            alert_ids: vec![id],
            ..AlertBulkFilter::default()
        },
        "acknowledged",
    )
    .expect("bulk acknowledge");
    assert_eq!(count, 1);
    assert!(service
        .list_alerts(Some("acknowledged"))
        .expect("acknowledged")
        .iter()
        .any(|alert| alert.id == id));
}

#[test]
fn bulk_update_rejects_unbounded_and_invalid_selection() {
    let db = Database::open_in_memory().expect("database");
    let service = SiemService::new(db);
    assert!(bulk_update_alerts(service.db(), &AlertBulkFilter::default(), "resolved").is_err());
    assert!(bulk_update_alerts(
        service.db(),
        &AlertBulkFilter {
            alert_ids: vec![-5],
            ..AlertBulkFilter::default()
        },
        "resolved"
    )
    .is_err());
}
