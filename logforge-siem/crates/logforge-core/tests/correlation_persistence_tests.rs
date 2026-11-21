use logforge_core::correlation_state::{evaluate_sequence, evaluate_threshold, PendingEvent};
use logforge_core::Database;

#[test]
fn threshold_state_survives_service_restart_and_deduplicates_event_ids() {
    let db = Database::open_in_memory().unwrap();
    let now = "2026-11-21T12:02:00Z";
    let first = PendingEvent {
        event_id: 41,
        timestamp_utc: "2026-11-21T12:00:30Z".into(),
        action: "failure".into(),
    };
    let second = PendingEvent {
        event_id: 42,
        timestamp_utc: "2026-11-21T12:01:30Z".into(),
        action: "failure".into(),
    };
    let one = evaluate_threshold(
        &db,
        "threshold-1",
        "192.0.2.1",
        120,
        2,
        &[first.clone()],
        now,
    )
    .unwrap();
    assert!(!one.emitted);
    let two = evaluate_threshold(
        &db,
        "threshold-1",
        "192.0.2.1",
        120,
        2,
        &[first, second],
        now,
    )
    .unwrap();
    assert!(two.emitted);
    assert_eq!(two.event_ids, vec![41, 42]);
    let repeat = evaluate_threshold(&db, "threshold-1", "192.0.2.1", 120, 2, &[], now).unwrap();
    assert!(!repeat.emitted);
}

#[test]
fn persisted_sequence_matches_only_ordered_steps_inside_window() {
    let db = Database::open_in_memory().unwrap();
    let failure = PendingEvent {
        event_id: 7,
        timestamp_utc: "2026-11-21T12:00:00Z".into(),
        action: "failure".into(),
    };
    let partial = evaluate_sequence(
        &db,
        "sequence-1",
        "host-a",
        "failure",
        "success",
        600,
        &[failure],
        "2026-11-21T12:01:00Z",
    )
    .unwrap();
    assert!(!partial.emitted);
    let success = PendingEvent {
        event_id: 8,
        timestamp_utc: "2026-11-21T12:05:00Z".into(),
        action: "success".into(),
    };
    let complete = evaluate_sequence(
        &db,
        "sequence-1",
        "host-a",
        "failure",
        "success",
        600,
        &[success],
        "2026-11-21T12:05:00Z",
    )
    .unwrap();
    assert!(complete.emitted);
    assert_eq!(complete.event_ids, vec![7, 8]);
}
