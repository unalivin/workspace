#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_empty_and_oversized_lines() {
        assert_eq!(parse_line("   "), Err(IngestError::Empty));
        let big = "a".repeat(MAX_LINE_BYTES + 1);
        assert_eq!(parse_line(&big), Err(IngestError::Oversized));
    }

    #[test]
    fn normalizes_timezone_to_utc() {
        let line = r#"{"timestamp":"2026-11-21T12:00:00+02:00","severity":"warn","host":"web-01","service":"sshd","message":"Failed login"}"#;
        let ev = parse_line(line).expect("parse json");
        assert!(ev.timestamp_utc.starts_with("2026-11-21T10:00:00"));
        assert!(ev.timestamp_utc.ends_with("+00:00") || ev.timestamp_utc.ends_with('Z'));
        assert_eq!(ev.severity, Severity::Warn);
        assert!(!ev.redacted);
    }

    #[test]
    fn redacts_secrets_and_marks_flag() {
        let line = "2026-11-21T10:00:00Z api-01 auth: login failed token=abc12345 Authorization: Bearer xyz987";
        let ev = parse_line(line).expect("parse syslog");
        assert!(ev.redacted);
        assert!(!ev.message.contains("abc12345"));
        assert!(!ev.message.contains("xyz987"));
        assert!(ev.message.contains("token=[REDACTED]"));
        assert!(ev.message.contains("Bearer [REDACTED]"));
    }

    #[test]
    fn rejects_malformed_json_and_syslog() {
        assert!(matches!(
            parse_line("{bad json"),
            Err(IngestError::Malformed(_))
        ));
        assert!(matches!(
            parse_line("only two"),
            Err(IngestError::Malformed(_))
        ));
    }
}
