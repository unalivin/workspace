use logforge_core::config::{validate_density, validate_text_scale};
use logforge_core::export::export_events_csv;
use logforge_core::model::LogEvent;
use logforge_core::notifier::{is_quiet_hour, severity_rank};
use logforge_core::query_parser::parse_search_query;
use logforge_core::theme::{severity_color, ThemeMode};

#[test]
fn test_export_csv_escapes_quotes() {
    let ev = LogEvent {
        id: 1,
        source_id: 1,
        timestamp_utc: "2026-11-21T00:00:00Z".to_string(),
        received_at: "2026-11-21T00:00:00Z".to_string(),
        severity: "info".to_string(),
        host: "host,\"quoted\"".to_string(),
        service: "s1".to_string(),
        message: "say \"hi\"".to_string(),
        raw_line: "raw".to_string(),
        dedup_hash: "h".to_string(),
        redacted: false,
        fields_json: "{}".to_string(),
    };
    let csv = export_events_csv(&[ev]);
    assert!(csv.contains("\"say \"\"hi\"\"\""));
    assert!(csv.contains("\"host,\"\"quoted\"\"\""));
    assert!(csv.contains("source_id,timestamp_utc,received_at"));
}

#[test]
fn test_validate_density_rejects_empty() {
    assert!(!validate_density(""));
}

#[test]
fn test_validate_text_scale_zero_rejected() {
    assert!(!validate_text_scale(0));
}

#[test]
fn test_theme_mode_high_contrast_alias() {
    assert_eq!(
        ThemeMode::parse("high_contrast"),
        Some(ThemeMode::HighContrast)
    );
}

#[test]
fn test_severity_color_warn_light() {
    assert_eq!(severity_color("warn", false), "#A16207");
}

#[test]
fn test_severity_color_low_dark() {
    assert_eq!(severity_color("low", true), "#6FD39A");
}

#[test]
fn test_query_parser_multiple_filters() {
    let q = parse_search_query("host:a severity:error timeout");
    assert_eq!(q.host_filter.as_deref(), Some("a"));
    assert_eq!(q.severity_filter.as_deref(), Some("error"));
    assert_eq!(q.fts_terms, vec!["timeout"]);
}

#[test]
fn test_is_quiet_hour_boundary_end_exclusive() {
    assert!(!is_quiet_hour(6, 22, 6));
}

#[test]
fn test_severity_rank_emerg() {
    assert_eq!(severity_rank("emerg"), 5);
}

#[test]
fn test_severity_rank_warning() {
    assert_eq!(severity_rank("warning"), 3);
}
