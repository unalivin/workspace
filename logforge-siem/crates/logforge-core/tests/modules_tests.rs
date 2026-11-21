use logforge_core::category_colors::assign_category_color;
use logforge_core::export::{export_alerts_json, export_events_csv, export_events_json};
use logforge_core::notifier::{
    is_quiet_hour, should_notify, upsert_notification_rule, NotificationRule,
};
use logforge_core::query_parser::parse_search_query;
use logforge_core::retention::{get_default_policy, prune_excess_events};
use logforge_core::{Database, SiemService};

#[test]
fn test_quiet_hours_disabled_when_equal() {
    assert!(!is_quiet_hour(0, 0, 0));
    assert!(!is_quiet_hour(15, 5, 5));
}

#[test]
fn test_quiet_hours_same_day_range() {
    assert!(is_quiet_hour(10, 9, 17));
    assert!(!is_quiet_hour(8, 9, 17));
    assert!(!is_quiet_hour(17, 9, 17));
}

#[test]
fn test_quiet_hours_overnight_range() {
    assert!(is_quiet_hour(23, 22, 6));
    assert!(is_quiet_hour(2, 22, 6));
    assert!(!is_quiet_hour(12, 22, 6));
}

#[test]
fn test_should_notify_respects_enabled_and_threshold() {
    let rule = NotificationRule {
        id: 1,
        event_type: "alert".to_string(),
        min_severity: "warn".to_string(),
        enabled: true,
        quiet_start_hour: 0,
        quiet_end_hour: 0,
    };
    assert!(!should_notify(&rule, "info", 12));
    assert!(should_notify(&rule, "warn", 12));
    assert!(should_notify(&rule, "critical", 12));
}

#[test]
fn test_upsert_notification_rule_persists() {
    let db = Database::open_in_memory().expect("db");
    upsert_notification_rule(&db, "alert", "error", true, 22, 6).expect("upsert");
    let count: i64 = db
        .conn()
        .query_row("SELECT COUNT(*) FROM notification_rules", [], |r| r.get(0))
        .expect("count");
    assert_eq!(count, 1);
}

#[test]
fn test_default_retention_policy_exists() {
    let db = Database::open_in_memory().expect("db");
    let p = get_default_policy(&db).expect("policy");
    assert_eq!(p.name, "default");
    assert_eq!(p.max_days, 90);
}

#[test]
fn test_prune_excess_events_deletes_oldest() {
    let db = Database::open_in_memory().expect("db");
    let svc = SiemService::new(db);
    let sid = svc.add_source("s1", "file", "/tmp/a.log").expect("src");
    svc.ingest_lines(
        sid,
        &[
            "2026-11-21T01:00:00Z h1 app: one",
            "2026-11-21T02:00:00Z h1 app: two",
            "2026-11-21T03:00:00Z h1 app: three",
        ],
    )
    .expect("ingest");
    let deleted = prune_excess_events(svc.db(), 1).expect("prune");
    assert_eq!(deleted, 2);
}

#[test]
fn test_parse_search_query_extracts_filters() {
    let q = parse_search_query("severity:critical host:web-01 failed login");
    assert_eq!(q.severity_filter.as_deref(), Some("critical"));
    assert_eq!(q.host_filter.as_deref(), Some("web-01"));
    assert_eq!(q.fts_terms, vec!["failed", "login"]);
}

#[test]
fn test_category_color_assignment_stable() {
    let db = Database::open_in_memory().expect("db");
    let c1 = assign_category_color(&db, "source", "auth", false).expect("c1");
    let c2 = assign_category_color(&db, "source", "auth", false).expect("c2");
    assert_eq!(c1, c2);
}

#[test]
fn test_export_events_json_and_csv() {
    let db = Database::open_in_memory().expect("db");
    let svc = SiemService::new(db);
    svc.seed_demo_data().expect("seed");
    let events = svc.search_events("", 10).expect("events");
    let json = export_events_json(&events).expect("json");
    assert!(json.contains("web-01"));
    let csv = export_events_csv(&events);
    assert!(csv.starts_with("id,timestamp_utc,severity,host,service,message,redacted\n"));
    let alerts = svc.list_alerts(None).expect("alerts");
    let ajson = export_alerts_json(&alerts).expect("ajson");
    assert!(ajson.contains("Failed SSH login"));
}
