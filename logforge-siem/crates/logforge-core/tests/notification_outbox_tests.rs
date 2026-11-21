use logforge_core::notifier::{
    claim_due_notifications, enqueue_alert_notification, finish_notification, outbox_counts,
    upsert_notification_rule,
};
use logforge_core::{Database, SiemService};
use serde_json::json;

#[test]
fn generated_alerts_enqueue_and_retry_notifications_with_backoff() {
    let db = Database::open_in_memory().unwrap();
    let service = SiemService::new(db);
    let source = service.add_source("notify", "file", "memory").unwrap();
    service.add_rule("failed login", "error", "failed").unwrap();
    upsert_notification_rule(service.db(), "alert.created", "warn", true, 0, 0).unwrap();
    service.ingest_lines(source, &["2026-11-21T10:00:00Z host sshd: failed password"]).unwrap();
    let counts = outbox_counts(service.db()).unwrap();
    assert_eq!(counts.queued, 1);
    let now = chrono::Utc::now().to_rfc3339();
    let claimed = claim_due_notifications(service.db(), &now, 20).unwrap();
    assert_eq!(claimed.len(), 1);
    assert!(finish_notification(service.db(), claimed[0].id, &now, Some("token=secret" )).unwrap());
    assert_eq!(outbox_counts(service.db()).unwrap().queued, 1);
    let retry_at = (chrono::Utc::now() + chrono::Duration::seconds(31)).to_rfc3339();
    let retry = claim_due_notifications(service.db(), &retry_at, 20).unwrap();
    assert_eq!(retry.len(), 1);
    assert!(finish_notification(service.db(), retry[0].id, &retry_at, None).unwrap());
    let counts = outbox_counts(service.db()).unwrap();
    assert_eq!(counts.delivered, 1);
    let error: String = service.db().conn().query_row("SELECT last_error FROM notification_outbox WHERE id = ?1", rusqlite::params![retry[0].id], |row| row.get(0)).unwrap();
    assert!(!error.contains("secret"));
}

#[test]
fn duplicate_outbox_messages_are_suppressed_and_severity_threshold_is_respected() {
    let db = Database::open_in_memory().unwrap();
    let service = SiemService::new(db);
    let source = service.add_source("notify-dedup", "file", "memory").unwrap();
    let alert_rule = service.add_rule("warning", "warn", "attention").unwrap();
    upsert_notification_rule(service.db(), "alert.created", "error", true, 0, 0).unwrap();
    service.ingest_lines(source, &["2026-11-21T10:00:00Z host app: attention needed"]).unwrap();
    assert_eq!(outbox_counts(service.db()).unwrap().queued, 0);

    let alert_id: i64 = service.db().conn().query_row("SELECT id FROM alerts WHERE rule_id = ?1", rusqlite::params![alert_rule], |row| row.get(0)).unwrap();
    let payload = json!({"detail": "test"});
    assert!(enqueue_alert_notification(service.db(), alert_id, "alert.created", "error", &payload, "2026-11-21T10:01:00Z").unwrap());
    assert!(!enqueue_alert_notification(service.db(), alert_id, "alert.created", "error", &payload, "2026-11-21T10:02:00Z").unwrap());
    assert_eq!(outbox_counts(service.db()).unwrap().queued, 1);
}
