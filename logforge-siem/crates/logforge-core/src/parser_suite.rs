use crate::ingest::{parse_line, IngestError, NormalizedEvent};
use chrono::Datelike;
use serde::Serialize;
use std::collections::BTreeMap;
use std::io::{self, BufRead};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
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

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ParseErrorSample {
    pub line_number: u64,
    pub message: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ParserValidationReport {
    pub format: LogFormat,
    pub lines_seen: u64,
    pub blank_lines: u64,
    pub parsed_lines: u64,
    pub malformed_lines: u64,
    pub severity_counts: BTreeMap<String, u64>,
    pub error_samples: Vec<ParseErrorSample>,
}

/// Validates a sample stream without writing it to the database. Oversized
/// records are drained to their line boundary and counted as malformed, while
/// only a bounded number of explanatory errors are retained in the report.
pub fn validate_reader<R: BufRead>(
    mut reader: R,
    format: LogFormat,
    max_line_bytes: usize,
    max_error_samples: usize,
) -> io::Result<ParserValidationReport> {
    let max_line_bytes = max_line_bytes.max(1);
    let mut report = ParserValidationReport {
        format,
        lines_seen: 0,
        blank_lines: 0,
        parsed_lines: 0,
        malformed_lines: 0,
        severity_counts: BTreeMap::new(),
        error_samples: Vec::new(),
    };
    let mut buffer = Vec::new();
    loop {
        buffer.clear();
        let mut too_long = false;
        let mut saw_bytes = false;
        loop {
            let available = reader.fill_buf()?;
            if available.is_empty() {
                break;
            }
            saw_bytes = true;
            let newline = available.iter().position(|byte| *byte == b'\n');
            let consumed = newline.map_or(available.len(), |index| index + 1);
            let content = newline.unwrap_or(consumed);
            if !too_long {
                let room = max_line_bytes
                    .saturating_add(1)
                    .saturating_sub(buffer.len());
                let copy = content.min(room);
                buffer.extend_from_slice(&available[..copy]);
                too_long = copy < content || buffer.len() > max_line_bytes;
            }
            reader.consume(consumed);
            if newline.is_some() {
                break;
            }
        }
        if !saw_bytes && buffer.is_empty() {
            break;
        }
        report.lines_seen += 1;
        if too_long {
            report.malformed_lines += 1;
            record_error(
                &mut report,
                max_error_samples,
                "record exceeds configured line limit",
            );
            continue;
        }
        while matches!(buffer.last(), Some(b'\n' | b'\r')) {
            buffer.pop();
        }
        let line = String::from_utf8_lossy(&buffer);
        let line = line.trim_start_matches('\u{feff}');
        if line.trim().is_empty() {
            report.blank_lines += 1;
            continue;
        }
        match parse_with_format(line, format) {
            Ok(event) => {
                report.parsed_lines += 1;
                *report
                    .severity_counts
                    .entry(event.severity.as_str().to_string())
                    .or_default() += 1;
            }
            Err(error) => {
                report.malformed_lines += 1;
                record_error(&mut report, max_error_samples, &error.to_string());
            }
        }
    }
    Ok(report)
}

fn record_error(report: &mut ParserValidationReport, max_samples: usize, message: &str) {
    if report.error_samples.len() < max_samples {
        report.error_samples.push(ParseErrorSample {
            line_number: report.lines_seen,
            message: message.to_string(),
        });
    }
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
        LogFormat::Nginx | LogFormat::Apache => {
            crate::access_log::parse_combined_access(raw, "unknown")
        }
        LogFormat::Cef => crate::cef_parser::parse_cef_line(raw),
        LogFormat::Leef => crate::leef_parser::parse_leef_line(raw),
    }
}

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
