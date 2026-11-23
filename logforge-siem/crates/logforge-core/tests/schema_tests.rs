use logforge_core::{Database, SiemService};

#[test]
fn event_schema_is_read_from_the_migrated_sqlite_database() {
    let service = SiemService::new(Database::open_in_memory().expect("database"));
    let fields = service.event_schema().expect("event schema");

    let id = fields
        .iter()
        .find(|field| field.name == "id")
        .expect("id field");
    assert!(id.primary_key);
    assert!(fields.iter().any(|field| {
        field.name == "timestamp_utc" && field.required && field.data_type == "TEXT"
    }));
    assert_eq!(fields.len(), 12);
}
