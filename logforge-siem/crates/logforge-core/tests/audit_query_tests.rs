use logforge_core::audit_verify::{
    audit_action_counts, query_audit_log, verify_audit_chain, AuditFilter,
};
use logforge_core::{Database, SiemService};

#[test]
fn audit_queries_filter_page_and_preserve_chain_integrity() {
    let db = Database::open_in_memory().expect("database");
    let service = SiemService::new(db);
    service.record_audit("analyst", "rule.update", "rule:12").expect("audit");
    service.record_audit("analyst", "case.create", "case:4").expect("audit");
    service.record_audit("system", "ingest.complete", "source:2").expect("audit");

    let all = query_audit_log(service.db(), &AuditFilter { limit: 2, ..AuditFilter::default() })
        .expect("audit page");
    assert_eq!(all.len(), 2);
    assert!(all[0].id > all[1].id);
    let actor = query_audit_log(service.db(), &AuditFilter {
        actor: Some("analyst".into()),
        limit: 10,
        ..AuditFilter::default()
    }).expect("actor filter");
    assert_eq!(actor.len(), 2);
    let actions = audit_action_counts(service.db()).expect("action totals");
    assert_eq!(actions.len(), 3);
    assert!(verify_audit_chain(service.db()).expect("verify chain").valid);
}

#[test]
fn audit_query_validates_bounds_and_rejects_zero_limit() {
    let db = Database::open_in_memory().expect("database");
    let service = SiemService::new(db);
    assert!(query_audit_log(service.db(), &AuditFilter::default()).is_err());
    assert!(query_audit_log(service.db(), &AuditFilter {
        from_utc: Some("2026-12-01T00:00:00Z".into()),
        to_utc: Some("2026-11-01T00:00:00Z".into()),
        limit: 20,
        ..AuditFilter::default()
    }).is_err());
}
