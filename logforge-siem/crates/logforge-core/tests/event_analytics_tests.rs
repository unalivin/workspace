use logforge_core::event_analytics::{analytics_snapshot, event_rate_series, BucketSize};
use logforge_core::{Database, SiemService};

#[test]
fn analytics_snapshot_reports_distribution_latency_and_alerts() {
    let db = Database::open_in_memory().unwrap();
    let service = SiemService::new(db);
    let source = service.add_source("analytics", "file", "memory").unwrap();
    service.add_rule("error detector", "error", "error").unwrap();
    service.ingest_lines(source, &[
        "2026-11-21T00:00:00Z host-a api: error in first request",
        "2026-11-21T00:00:01Z host-a api: normal second request",
        "2026-11-21T00:00:02Z host-b sshd: normal third request",
    ]).unwrap();
    let snapshot = analytics_snapshot(service.db(), 10).unwrap();
    assert_eq!(snapshot.total_events, 3);
    assert_eq!(snapshot.total_alerts, 1);
    assert_eq!(snapshot.top_hosts[0].value, "host-a");
    assert_eq!(snapshot.top_hosts[0].event_count, 2);
    assert_eq!(snapshot.severity_counts.iter().map(|item| item.event_count).sum::<i64>(), 3);
    assert_eq!(snapshot.ingestion_latency.sample_count, 3);
}

#[test]
fn event_rate_series_emits_empty_buckets_and_rejects_bad_ranges() {
    let db = Database::open_in_memory().unwrap();
    let service = SiemService::new(db);
    let source = service.add_source("series", "file", "memory").unwrap();
    service.ingest_lines(source, &["2026-11-21T10:01:00Z host app: event"]).unwrap();
    let buckets = event_rate_series(service.db(), "2026-11-21T10:00:00Z", "2026-11-21T10:03:00Z", BucketSize::Minute).unwrap();
    assert_eq!(buckets.len(), 3);
    assert_eq!(buckets[0].event_count, 0);
    assert_eq!(buckets[1].event_count, 1);
    assert_eq!(buckets[2].event_count, 0);
    assert!(event_rate_series(service.db(), "2026-11-21T10:03:00Z", "2026-11-21T10:00:00Z", BucketSize::Hour).is_err());
}
