use logforge_core::stats::{source_liveness, SourceState};
use logforge_core::{Database, SiemService};

#[test]
fn liveness_distinguishes_active_never_seen_and_disabled_sources() {
    let db = Database::open_in_memory().expect("database");
    let service = SiemService::new(db);
    let active = service.add_source("active", "file", "a.log").expect("source");
    service.add_source("never", "file", "b.log").expect("source");
    let disabled = service.add_source("disabled", "api", "https://local").expect("source");
    service.set_source_enabled(disabled, false).expect("disable");
    service
        .ingest_lines(active, &["2026-11-21T00:00:00Z host app: event"])
        .expect("ingest");

    let report = source_liveness(
        service.db(),
        &chrono::Utc::now().to_rfc3339(),
        60,
    )
    .expect("liveness");
    assert_eq!(report.active, 1);
    assert_eq!(report.never_seen, 1);
    assert_eq!(report.disabled, 1);
    assert_eq!(report.stale, 0);
    assert!(report.sources.iter().any(|source| source.state == SourceState::NeverSeen));
}

#[test]
fn liveness_rejects_invalid_clock_and_staleness_threshold() {
    let db = Database::open_in_memory().expect("database");
    assert!(source_liveness(&db, "invalid", 30).is_err());
    assert!(source_liveness(&db, "2026-11-21T00:00:00Z", 0).is_err());
}
