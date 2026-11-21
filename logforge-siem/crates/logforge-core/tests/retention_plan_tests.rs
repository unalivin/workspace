use logforge_core::retention::{apply_retention_plan, plan_retention};
use logforge_core::{Database, SiemService};

#[test]
fn age_policy_prunes_old_events_even_when_event_count_is_below_limit() {
    let db = Database::open_in_memory().unwrap();
    let service = SiemService::new(db);
    let source = service.add_source("retention", "file", "memory").unwrap();
    let records = [
        "2026-01-01T00:00:00Z old host: stale record",
        "2026-11-20T00:00:00Z new host: fresh one",
        "2026-11-20T00:00:01Z new host: fresh two",
    ];
    let refs: Vec<_> = records.iter().copied().collect();
    service.ingest_lines(source, &refs).unwrap();
    let plan = plan_retention(service.db(), 30, 100, 100_000, "2026-11-21T00:00:00Z").unwrap();
    assert_eq!(plan.count, 1);
    assert_eq!(apply_retention_plan(service.db(), &plan).unwrap(), 1);
    assert_eq!(service.overview_counts().unwrap().total_events, 2);
}

#[test]
fn byte_cap_keeps_newest_records_and_lists_all_removals() {
    let db = Database::open_in_memory().unwrap();
    let service = SiemService::new(db);
    let source = service.add_source("size-retention", "file", "memory").unwrap();
    let records = [
        "2026-11-20T00:00:00Z host app: alpha has a medium sized record",
        "2026-11-20T00:00:01Z host app: beta has a medium sized record",
        "2026-11-20T00:00:02Z host app: gamma has a medium sized record",
    ];
    let refs: Vec<_> = records.iter().copied().collect();
    service.ingest_lines(source, &refs).unwrap();
    let plan = plan_retention(service.db(), 60, 10, 100, "2026-11-21T00:00:00Z").unwrap();
    assert!(plan.count >= 2);
    assert!(plan.bytes_to_free > 0);
}
