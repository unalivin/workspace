#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn validator_counts_success_blank_and_malformed_records() {
        let input = concat!(
            "{\"timestamp\":\"2026-11-21T00:00:00Z\",\"host\":\"web\",\"message\":\"ok\"}\n",
            "  \n",
            "not-json\n",
        );
        let report = validate_reader(input.as_bytes(), LogFormat::Json, 512, 5).expect("validate");
        assert_eq!(report.lines_seen, 3);
        assert_eq!(report.parsed_lines, 1);
        assert_eq!(report.blank_lines, 1);
        assert_eq!(report.malformed_lines, 1);
        assert_eq!(report.error_samples[0].line_number, 3);
    }

    #[test]
    fn validator_caps_errors_and_rejects_oversized_lines_without_buffering_them() {
        let input = b"too-long-record\nnot-json\n";
        let report = validate_reader(input.as_slice(), LogFormat::Json, 4, 1).expect("validate");
        assert_eq!(report.lines_seen, 2);
        assert_eq!(report.malformed_lines, 2);
        assert_eq!(report.error_samples.len(), 1);
        assert_eq!(report.error_samples[0].line_number, 1);
    }

    #[test]
    fn explicit_dispatch_rejects_mismatched_formats_without_panicking() {
        for format in [
            LogFormat::Json,
            LogFormat::Rfc5424,
            LogFormat::Cef,
            LogFormat::Leef,
        ] {
            assert!(parse_with_format("not a log record", format).is_err());
        }
    }

    #[test]
    fn automatic_dispatch_selects_cef_and_rfc5424() {
        let cef = "CEF:0|Acme|Gateway|1|42|Denied|7|src=192.0.2.1";
        assert!(parse_with_format(cef, LogFormat::Auto).is_ok());
        let syslog = "<34>1 2026-11-21T10:00:00Z host app 123 ID47 - login failed";
        assert!(parse_with_format(syslog, LogFormat::Auto).is_ok());
    }
}
