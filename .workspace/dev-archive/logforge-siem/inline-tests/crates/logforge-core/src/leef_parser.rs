#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_leef_attributes_delimiter_timestamp_severity_and_host() {
        let raw = "LEEF:2.0|Vendor|Firewall|3.1|evt-4|\\t|devTime=2026-11-21T10:00:00Z\tdevName=edge-7\tsev=9\tmsg=Blocked\\trequest";
        let event = parse_leef_line(raw).unwrap();
        assert_eq!(event.host, "edge-7");
        assert_eq!(event.severity, Severity::Critical);
        assert!(event.message.contains("Blocked\trequest"));
        assert!(event.fields_json.contains("devName"));
    }

    #[test]
    fn rejects_invalid_versions_and_duplicate_keys() {
        assert!(parse_leef_line("LEEF:9.0|V|P|1|E|\\t|sev=5").is_err());
        assert!(parse_leef_line("LEEF:2.0|V|P|1|E|^|sev=3^sev=4").is_err());
    }
}
