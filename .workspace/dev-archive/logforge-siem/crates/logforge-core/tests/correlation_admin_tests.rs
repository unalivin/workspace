use logforge_core::correlation_admin;
use logforge_core::{Database, SiemService};

#[test]
fn correlation_rules_can_be_reviewed_disabled_and_removed_with_state_cleanup() {
    let db = Database::open_in_memory().unwrap();
    let service = SiemService::new(db);
    let detection = service.add_rule("admin test", "warn", "failed").unwrap();
    let id = service
        .add_threshold_correlation(detection, "host", 60, 2)
        .unwrap();
    assert_eq!(
        correlation_admin::list_rules(service.db(), true)
            .unwrap()
            .len(),
        1
    );
    assert!(correlation_admin::set_enabled(service.db(), id, false).unwrap());
    assert!(correlation_admin::list_rules(service.db(), true)
        .unwrap()
        .is_empty());
    assert_eq!(
        correlation_admin::list_rules(service.db(), false).unwrap()[0].name,
        "admin test"
    );
    service.db().conn().execute(
        "INSERT INTO correlation_state (rule_key, correlation_key, pending_events, last_emitted_at, updated_at) VALUES (?1, 'host-a', '[]', NULL, '2026-11-21T00:00:00Z')",
        rusqlite::params![format!("rule:{detection}")],
    ).unwrap();
    assert!(correlation_admin::remove_rule(service.db(), id).unwrap());
    let state_count: i64 = service
        .db()
        .conn()
        .query_row("SELECT COUNT(*) FROM correlation_state", [], |row| {
            row.get(0)
        })
        .unwrap();
    assert_eq!(state_count, 0);
    assert!(!correlation_admin::remove_rule(service.db(), id).unwrap());
}
