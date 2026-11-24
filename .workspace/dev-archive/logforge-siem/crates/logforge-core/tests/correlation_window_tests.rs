use chrono::{TimeZone, Utc};
use logforge_core::correlation::{ordered_sequence, threshold_window};

#[test]
fn threshold_uses_sliding_windows_and_rearms_after_expiry() {
    let start = Utc.with_ymd_and_hms(2026, 1, 1, 0, 0, 0).unwrap();
    let hits = threshold_window(
        [
            ("host".into(), start, 1),
            ("host".into(), start + chrono::Duration::seconds(1), 2),
            ("host".into(), start + chrono::Duration::seconds(2), 3),
            ("host".into(), start + chrono::Duration::seconds(12), 4),
            ("host".into(), start + chrono::Duration::seconds(13), 5),
        ],
        10,
        2,
    );
    assert_eq!(hits.len(), 2);
    assert_eq!(hits[0].event_ids, vec![1, 2]);
    assert_eq!(hits[1].event_ids, vec![4, 5]);
}

#[test]
fn sequence_requires_first_then_second_within_the_window() {
    let start = Utc.with_ymd_and_hms(2026, 1, 1, 0, 0, 0).unwrap();
    let hits = ordered_sequence(
        [
            ("a".into(), "success".into(), start, 1),
            ("b".into(), "failure".into(), start, 2),
            (
                "a".into(),
                "failure".into(),
                start + chrono::Duration::seconds(3),
                3,
            ),
            (
                "b".into(),
                "success".into(),
                start + chrono::Duration::seconds(8),
                4,
            ),
        ],
        "failure",
        "success",
        10,
    );
    assert_eq!(hits.len(), 1);
    assert_eq!((hits[0].first_event_id, hits[0].second_event_id), (2, 4));
}
