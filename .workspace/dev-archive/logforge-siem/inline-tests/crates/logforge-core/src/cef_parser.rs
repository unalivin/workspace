#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decodes_escaped_header_fields_and_multitoken_extensions() {
        let line = r"CEF:0|Vendor|Product|2|sig\|42|Blocked\| login|9|rt=2026-11-21T10:00:00Z dvchost=edge-1 msg=login failed\| user src=192.0.2.1 cs1Label=tenant cs1=blue";
        let event = parse_cef_line(line).unwrap();
        assert_eq!(event.severity, Severity::Critical);
        assert_eq!(event.host, "edge-1");
        assert!(event.message.contains("login failed| user"));
        let fields: BTreeMap<String, String> = serde_json::from_str(&event.fields_json).unwrap();
        assert_eq!(fields.get("cs1"), Some(&"blue".to_string()));
    }

    #[test]
    fn parses_epoch_time_and_rejects_duplicate_extension_keys() {
        let event = parse_cef_line("CEF:0|V|P|1|id|notice|3|rt=1795255200000 msg=ok").unwrap();
        assert!(event.timestamp_utc.starts_with("2026-11-21T10:00:00"));
        assert!(parse_cef_line("CEF:0|V|P|1|id|notice|3|src=1 src=2").is_err());
    }
}
