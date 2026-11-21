use logforge_core::intake_queue::{BoundedIntake, FullPolicy};
use std::time::Duration;

#[test]
fn drop_newest_counts_every_item_not_accepted() {
    let queue = BoundedIntake::new(2);
    assert!(queue.push(1, FullPolicy::DropNewest, Duration::ZERO).is_ok());
    assert!(queue.push(2, FullPolicy::DropNewest, Duration::ZERO).is_ok());
    assert_eq!(queue.push(3, FullPolicy::DropNewest, Duration::ZERO), Err(3));
    assert_eq!(queue.counters().dropped, 1);
    assert_eq!(queue.pop(Duration::ZERO), Some(1));
    assert_eq!(queue.pop(Duration::ZERO), Some(2));
}

#[test]
fn blocking_policy_times_out_without_losing_the_item() {
    let queue = BoundedIntake::new(1);
    queue.push("first", FullPolicy::BlockReader, Duration::ZERO).unwrap();
    assert_eq!(queue.push("second", FullPolicy::BlockReader, Duration::ZERO), Err("second"));
    assert_eq!(queue.counters().blocked, 1);
    assert_eq!(queue.len(), 1);
}
