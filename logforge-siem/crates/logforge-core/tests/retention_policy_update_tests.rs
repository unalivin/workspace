use logforge_core::retention::{get_default_policy, update_default_policy};
use logforge_core::Database;

#[test]
fn default_retention_policy_can_be_updated_with_valid_bounded_limits() {
    let db = Database::open_in_memory().expect("database");
    assert!(update_default_policy(
        &db,
        "  SOC default  ",
        180,
        2_000_000,
        2_147_483_648,
        false,
        "2026-11-21T12:00:00Z",
    )
    .expect("update policy"));
    let policy = get_default_policy(&db).expect("policy");
    assert_eq!(policy.name, "SOC default");
    assert_eq!(policy.max_days, 180);
    assert_eq!(policy.max_events, 2_000_000);
    assert_eq!(policy.max_bytes, 2_147_483_648);
    assert!(!policy.enabled);
}

#[test]
fn default_retention_policy_rejects_invalid_bounds_and_name() {
    let db = Database::open_in_memory().expect("database");
    assert!(update_default_policy(
        &db,
        " ",
        30,
        100,
        1000,
        true,
        "2026-11-21T00:00:00Z",
    )
    .is_err());
    assert!(update_default_policy(
        &db,
        "policy",
        0,
        100,
        1000,
        true,
        "2026-11-21T00:00:00Z",
    )
    .is_err());
}
