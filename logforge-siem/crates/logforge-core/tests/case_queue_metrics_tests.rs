use logforge_core::cases::{case_queue_metrics, case_workload_by_assignee, CaseManager};
use logforge_core::{Database, SiemService};

#[test]
fn case_metrics_report_priority_ownership_and_sla_age() {
    let db = Database::open_in_memory().expect("database");
    let service = SiemService::new(db);
    let manager = CaseManager::new(service.db());
    let first = manager
        .create("Unassigned incident", "", "critical", "")
        .expect("create case");
    manager
        .create("Assigned incident", "", "high", "analyst")
        .expect("create case");
    service
        .db()
        .conn()
        .execute(
            "UPDATE cases SET created_at = '2020-01-01T00:00:00+00:00'",
            [],
        )
        .expect("age cases");
    let now = chrono::Utc::now().to_rfc3339();
    let metrics = case_queue_metrics(service.db(), &now, 1).expect("metrics");
    assert_eq!(metrics.open_cases, 2);
    assert_eq!(metrics.unassigned_active, 1);
    assert_eq!(metrics.critical_active, 1);
    assert_eq!(metrics.overdue_active, 2);
    assert!(metrics.oldest_active_at.is_some());

    let workload = case_workload_by_assignee(service.db()).expect("workload");
    assert_eq!(workload.len(), 2);
    assert!(workload.iter().any(|entry| entry.assignee == "(unassigned)"));
    manager.set_status(first, "closed").expect("close case");
    let after_close = case_queue_metrics(service.db(), &now, 1).expect("metrics after close");
    assert_eq!(after_close.open_cases, 1);
    assert_eq!(after_close.closed_cases, 1);
}

#[test]
fn case_metrics_validate_reference_time_and_sla_bounds() {
    let db = Database::open_in_memory().expect("database");
    let service = SiemService::new(db);
    assert!(case_queue_metrics(service.db(), "not-a-time", 24).is_err());
    assert!(case_queue_metrics(service.db(), "2026-11-21T00:00:00Z", -1).is_err());
}
