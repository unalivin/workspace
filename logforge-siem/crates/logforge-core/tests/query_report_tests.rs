use logforge_core::query_report::{
    alert_counts_by_rule, event_volume_report, EventDimension, EventReportFilter,
};
use logforge_core::{Database, SiemService};

#[test]
fn event_volume_report_groups_all_safe_dimensions_and_keeps_total() {
    let db = Database::open_in_memory().expect("database");
    let service = SiemService::new(db);
    service.seed_demo_data().expect("seed");

    for dimension in [
        EventDimension::Severity,
        EventDimension::Host,
        EventDimension::Service,
        EventDimension::Source,
    ] {
        let report = event_volume_report(service.db(), dimension, &EventReportFilter::default())
            .expect("report");
        assert_eq!(report.total_events, 3);
        assert!(!report.buckets.is_empty());
        assert!(report.buckets.iter().all(|bucket| bucket.event_count > 0));
        let percentages: f64 = report.buckets.iter().map(|bucket| bucket.percentage).sum();
        assert!(percentages <= 100.001);
    }
}

#[test]
fn report_filters_validate_ranges_and_limit_results() {
    let db = Database::open_in_memory().expect("database");
    let service = SiemService::new(db);
    service.seed_demo_data().expect("seed");
    let filter = EventReportFilter {
        from_utc: Some("2026-11-21T08:00:00Z".to_string()),
        to_utc: Some("2026-11-21T08:02:00Z".to_string()),
        limit: 1,
        ..EventReportFilter::default()
    };
    let report = event_volume_report(service.db(), EventDimension::Host, &filter).expect("report");
    assert_eq!(report.total_events, 2);
    assert_eq!(report.buckets.len(), 1);
    assert!(event_volume_report(
        service.db(),
        EventDimension::Host,
        &EventReportFilter {
            limit: 501,
            ..EventReportFilter::default()
        }
    )
    .is_err());
}

#[test]
fn rules_without_alerts_are_present_in_rule_report() {
    let db = Database::open_in_memory().expect("database");
    let service = SiemService::new(db);
    service.seed_demo_data().expect("seed");
    let report = alert_counts_by_rule(service.db()).expect("rule report");
    assert_eq!(report.len(), 2);
    assert!(report.iter().any(|rule| rule.total_alerts == 0));
}
