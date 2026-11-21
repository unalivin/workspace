use logforge_core::risk_scoring::score_host_risk;
use logforge_core::{Database, SiemService};

#[test]
fn host_risk_report_orders_hosts_by_alert_and_severity_signal() {
    let db = Database::open_in_memory().expect("database");
    let service = SiemService::new(db);
    service.seed_demo_data().expect("seed");
    let report = score_host_risk(service.db(), "2000-01-01T00:00:00Z", 20).expect("risk");
    assert_eq!(report.host_count, 3);
    assert_eq!(report.hosts.len(), 3);
    assert!(report.hosts[0].score >= report.hosts[1].score);
    assert!(report.hosts.iter().all(|host| host.score <= 100));
    assert!(report.hosts.iter().all(|host| !host.band.is_empty()));
}

#[test]
fn risk_report_rejects_invalid_ranges_and_unbounded_limits() {
    let db = Database::open_in_memory().expect("database");
    assert!(score_host_risk(&db, "yesterday", 10).is_err());
    assert!(score_host_risk(&db, "2026-11-21T00:00:00Z", 501).is_err());
}
