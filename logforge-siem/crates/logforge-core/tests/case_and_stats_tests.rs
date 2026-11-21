use logforge_core::case_links::{link_alert_to_case, list_case_links};
use logforge_core::stats::{
    alert_severity_matrix, alert_status_counts, count_events_in_range, events_by_severity,
    ingestion_stats, source_health, top_sources,
};
use logforge_core::{Database, SiemService};

#[test]
fn test_link_alert_to_case_updates_alert_and_records_link() {
    let db = Database::open_in_memory().expect("db");
    let svc = SiemService::new(db);
    svc.seed_demo_data().expect("seed");
    let alerts = svc.list_alerts(None).expect("alerts");
    let aid = alerts[0].id;
    let link_id = link_alert_to_case(svc.db(), aid, "INC-2026-001", "triaged").expect("link");
    assert!(link_id > 0);
    let links = list_case_links(svc.db(), aid).expect("links");
    assert_eq!(links.len(), 1);
    assert_eq!(links[0].case_ref, "INC-2026-001");
}

#[test]
fn test_top_sources_returns_seeded_source() {
    let db = Database::open_in_memory().expect("db");
    let svc = SiemService::new(db);
    svc.seed_demo_data().expect("seed");
    let sources = top_sources(svc.db()).expect("top");
    assert_eq!(sources.len(), 1);
    assert_eq!(sources[0].name, "demo-auth");
    assert_eq!(sources[0].event_count, 3);
}

#[test]
fn operational_stats_include_empty_severities_and_source_health() {
    let db = Database::open_in_memory().expect("db");
    let svc = SiemService::new(db);
    svc.seed_demo_data().expect("seed");

    let severity = events_by_severity(svc.db()).expect("severity counts");
    assert_eq!(severity.len(), 5);
    assert_eq!(severity.iter().map(|item| item.event_count).sum::<i64>(), 3);

    let health = source_health(svc.db()).expect("source health");
    assert_eq!(health.len(), 1);
    assert_eq!(health[0].name, "demo-auth");
    assert_eq!(health[0].total_events, 3);
    assert!(health[0].last_event_at.is_some());
}

#[test]
fn ingestion_rates_are_zero_safe_and_time_ranges_are_half_open() {
    let db = Database::open_in_memory().expect("db");
    let svc = SiemService::new(db);
    svc.seed_demo_data().expect("seed");

    let metrics = ingestion_stats(svc.db()).expect("ingestion stats");
    assert!(metrics.received >= metrics.accepted);
    assert!((0.0..=100.0).contains(&metrics.duplicate_rate_percent));
    assert_eq!(
        count_events_in_range(svc.db(), "2000-01-01T00:00:00Z", "2100-01-01T00:00:00Z")
            .expect("range count"),
        3
    );
    assert!(
        count_events_in_range(svc.db(), "2100-01-01T00:00:00Z", "2000-01-01T00:00:00Z").is_err()
    );
}

#[test]
fn alert_statistics_include_empty_workflow_buckets() {
    let db = Database::open_in_memory().expect("db");
    let svc = SiemService::new(db);
    svc.seed_demo_data().expect("seed");

    let statuses = alert_status_counts(svc.db()).expect("status counts");
    assert_eq!(statuses.len(), 3);
    assert_eq!(statuses.iter().map(|item| item.alert_count).sum::<i64>(), 1);
    let matrix = alert_severity_matrix(svc.db()).expect("severity matrix");
    assert_eq!(matrix.len(), 5);
    assert_eq!(
        matrix
            .iter()
            .map(|item| item.open_count + item.acknowledged_count + item.resolved_count)
            .sum::<i64>(),
        1
    );
}
