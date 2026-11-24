use logforge_core::event_query::EventFilter;
use logforge_core::{Database, SiemService};

#[test]
fn filtered_search_supports_parameterized_facets_and_keyset_pages() {
    let db = Database::open_in_memory().unwrap();
    let service = SiemService::new(db);
    let source = service.add_source("filter", "file", "memory").unwrap();
    service
        .ingest_lines(
            source,
            &[
                "2026-11-21T10:00:00Z node-a sshd: access denied for user",
                "2026-11-21T10:01:00Z node-a sshd: access denied for root",
                "2026-11-21T10:02:00Z node-b nginx: access granted",
            ],
        )
        .unwrap();
    let filter = EventFilter {
        query: Some("denied".to_string()),
        host: Some("node-a' OR 1=1 --".to_string()),
        severity: Some("info".to_string()),
        limit: 10,
        ..EventFilter::default()
    };
    assert_eq!(service.count_events_filtered(&filter).unwrap(), 0);
    let all_denied = EventFilter {
        query: Some("denied".to_string()),
        limit: 10,
        ..EventFilter::default()
    };
    assert_eq!(service.count_events_filtered(&all_denied).unwrap(), 2);
    let page_one = EventFilter {
        query: Some("denied".to_string()),
        limit: 1,
        ..EventFilter::default()
    };
    let first = service.search_events_filtered(&page_one).unwrap();
    assert_eq!(first.len(), 1);
    let page_two = EventFilter {
        query: Some("denied".to_string()),
        after_timestamp: Some(first[0].timestamp_utc.clone()),
        after_id: Some(first[0].id),
        limit: 1,
        ..EventFilter::default()
    };
    let second = service.search_events_filtered(&page_two).unwrap();
    assert_eq!(second.len(), 1);
    assert_ne!(first[0].id, second[0].id);
}

#[test]
fn rejects_invalid_ranges_and_unpaired_pagination_cursors() {
    let db = Database::open_in_memory().unwrap();
    let service = SiemService::new(db);
    let invalid_time = EventFilter {
        from_utc: Some("yesterday".into()),
        limit: 1,
        ..EventFilter::default()
    };
    assert!(service.search_events_filtered(&invalid_time).is_err());
    let unpaired_cursor = EventFilter {
        after_id: Some(42),
        limit: 1,
        ..EventFilter::default()
    };
    assert!(service.search_events_filtered(&unpaired_cursor).is_err());
    let oversize = EventFilter {
        limit: 501,
        ..EventFilter::default()
    };
    assert!(service.count_events_filtered(&oversize).is_err());
}
