use crate::ingest::{parse_line, IngestError, NormalizedEvent};
use chrono::Datelike;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LogFormat {
    Auto,
    Json,
    Rfc3164,
    Rfc5424,
    Nginx,
    Apache,
    Cef,
    Leef,
}

pub fn parse_with_format(raw: &str, format: LogFormat) -> Result<NormalizedEvent, IngestError> {
    match format {
        LogFormat::Auto => {
            if raw.trim_start().starts_with("CEF:") {
                crate::cef_parser::parse_cef_line(raw)
            } else if raw.trim_start().starts_with("<") {
                if raw
                    .trim_start()
                    .split_once('>')
                    .is_some_and(|(_, rest)| rest.trim_start().starts_with("1 "))
                {
                    crate::syslog_rfc5424::parse_rfc5424(raw)
                } else {
                    let year = chrono::Utc::now().year();
                    crate::access_log::parse_rfc3164(raw, year, 0)
                }
            } else if raw.contains("] \"") && raw.contains("HTTP/") {
                crate::access_log::parse_combined_access(raw, "unknown")
            } else if raw.trim_start().starts_with("LEEF:") {
                crate::leef_parser::parse_leef_line(raw)
            } else {
                parse_line(raw)
            }
        }
        LogFormat::Json => parse_line(raw),
        LogFormat::Rfc3164 => crate::access_log::parse_rfc3164(raw, chrono::Utc::now().year(), 0),
        LogFormat::Rfc5424 => crate::syslog_rfc5424::parse_rfc5424(raw),
        LogFormat::Nginx | LogFormat::Apache => crate::access_log::parse_combined_access(raw, "unknown"),
        LogFormat::Cef => crate::cef_parser::parse_cef_line(raw),
        LogFormat::Leef => crate::leef_parser::parse_leef_line(raw),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
