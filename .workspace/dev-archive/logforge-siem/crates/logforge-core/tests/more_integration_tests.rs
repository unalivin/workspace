use logforge_core::config::{validate_density, validate_text_scale, AppConfig};
use logforge_core::schema_mapper::{apply_field_mappings, FieldMapping};
use std::collections::HashMap;

#[test]
fn test_default_config_values() {
    let cfg = AppConfig::default();
    assert_eq!(cfg.density, "compact");
    assert_eq!(cfg.text_scale, 100);
    assert!(!cfg.reduced_motion);
}

#[test]
fn test_validate_density_valid() {
    assert!(validate_density("compact"));
    assert!(validate_density("comfortable"));
    assert!(!validate_density("huge"));
}

#[test]
fn test_validate_text_scale_bounds() {
    assert!(validate_text_scale(80));
    assert!(validate_text_scale(100));
    assert!(validate_text_scale(150));
    assert!(!validate_text_scale(79));
    assert!(!validate_text_scale(151));
}

#[test]
fn test_apply_field_mappings() {
    let mut input = HashMap::new();
    input.insert("src_ip".to_string(), "192.0.2.1".to_string());
    let mappings = vec![FieldMapping {
        source_field: "src_ip".to_string(),
        target_field: "client_ip".to_string(),
    }];
    let out = apply_field_mappings(&input, &mappings);
    assert_eq!(out.get("client_ip").map(String::as_str), Some("192.0.2.1"));
}
