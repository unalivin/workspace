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
