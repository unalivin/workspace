use logforge_core::case_links::{link_alert_to_case, list_case_links};
use logforge_core::stats::top_sources;
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
