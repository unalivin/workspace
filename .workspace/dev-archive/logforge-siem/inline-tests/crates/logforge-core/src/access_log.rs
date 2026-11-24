#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_rfc3164_priority_timezone_and_tag_pid() {
        let parsed =
            parse_rfc3164("<34>Oct 11 22:14:15 host app[42]: started", 2026, 3600).unwrap();
        assert_eq!(parsed.host, "host");
        assert_eq!(parsed.service, "app");
        assert!(parsed.timestamp_utc.starts_with("2026-10-11T21:14:15"));
    }

    #[test]
    fn parses_combined_access_record_with_quoted_fields() {
        let parsed = parse_combined_access("192.0.2.1 - alice [11/Oct/2026:22:14:15 +0200] \"GET /login HTTP/1.1\" 401 17 \"-\" \"client agent\"", "web-01").unwrap();
        assert_eq!(parsed.severity, Severity::Warn);
        assert_eq!(parsed.host, "web-01");
        assert!(parsed.fields_json.contains("client agent"));
    }

    #[test]
    fn arbitrary_truncated_inputs_never_panic() {
        for parser in [0, 1] {
            for len in 0..256 {
                let raw =
                    String::from_utf8_lossy(&(0..len).map(|i| (i * 31) as u8).collect::<Vec<_>>())
                        .into_owned();
                let result = std::panic::catch_unwind(|| {
                    if parser == 0 {
                        let _ = parse_rfc3164(&raw, 2026, 0);
                    } else {
                        let _ = parse_combined_access(&raw, "host");
                    }
                });
                assert!(result.is_ok());
            }
        }
    }
}
