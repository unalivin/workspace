use logforge_core::audit_verify::verify_audit_chain;
use logforge_core::cef_parser::parse_cef_line;
use logforge_core::dedup_cache::DedupCache;
use logforge_core::leef_parser::parse_leef_line;
use logforge_core::storage_metrics::collect_storage_metrics;
use logforge_core::syslog_rfc5424::parse_rfc5424;
use logforge_core::time_series::events_per_hour;
use logforge_core::{Database, SiemService};

#[test]
fn test_parse_cef_valid() {
    let line = "CEF:0|Acme|Firewall|1.0|100|Blocked|7|src=192.0.2.1";
    let ev = parse_cef_line(line).expect("cef");
    assert_eq!(ev.host, "Acme");
    assert_eq!(ev.service, "Firewall 1.0");
}

#[test]
fn test_parse_leef_valid() {
    let line = "LEEF:1.0|Acme|IDS|2.0|200|^|src=192.0.2.2";
    let ev = parse_leef_line(line).expect("leef");
    assert_eq!(ev.host, "Acme");
    assert_eq!(ev.service, "IDS");
}

#[test]
fn test_parse_rfc5424_valid() {
    let line = "<34>1 2026-11-21T10:00:00Z web-01 su - ID47 - su root failed";
    let ev = parse_rfc5424(line).expect("rfc5424");
    assert_eq!(ev.host, "web-01");
    assert_eq!(ev.service, "su");
}

#[test]
fn test_dedup_cache_eviction() {
    let mut cache = DedupCache::new(2);
    assert!(cache.insert("a"));
    assert!(!cache.insert("a"));
    assert!(cache.insert("b"));
    assert!(cache.insert("c"));
    assert_eq!(cache.len(), 2);
}

#[test]
fn test_audit_verify_valid_chain() {
    let db = Database::open_in_memory().expect("db");
    let svc = SiemService::new(db);
    svc.seed_demo_data().expect("seed");
    let res = verify_audit_chain(svc.db()).expect("verify");
    assert!(res.valid);
    assert!(res.total_entries >= 1);
}

#[test]
fn test_storage_metrics_and_time_series() {
    let db = Database::open_in_memory().expect("db");
    let svc = SiemService::new(db);
    svc.seed_demo_data().expect("seed");
    let metrics = collect_storage_metrics(svc.db()).expect("metrics");
    assert_eq!(metrics.events_count, 3);
    let buckets = events_per_hour(svc.db(), 10).expect("buckets");
    assert!(!buckets.is_empty());
}
