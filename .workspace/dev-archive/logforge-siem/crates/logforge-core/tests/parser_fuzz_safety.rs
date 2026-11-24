use logforge_core::access_log::{parse_combined_access, parse_rfc3164};
use logforge_core::cef_parser::parse_cef_line;
use logforge_core::ingest::parse_line;
use logforge_core::leef_parser::parse_leef_line;
use logforge_core::syslog_rfc5424::parse_rfc5424;
use std::panic::AssertUnwindSafe;

#[test]
fn malformed_and_truncated_payloads_never_unwind_any_parser() {
    let mut state = 0x9e3779b97f4a7c15u64;
    for length in 0..2048 {
        let mut bytes = Vec::with_capacity(length);
        for _ in 0..length {
            state ^= state << 7;
            state ^= state >> 9;
            state ^= state << 8;
            bytes.push((state >> 24) as u8);
        }
        let value = String::from_utf8_lossy(&bytes).into_owned();
        for parser in [0u8, 1, 2, 3, 4, 5] {
            let result = std::panic::catch_unwind(AssertUnwindSafe(|| match parser {
                0 => {
                    let _ = parse_line(&value);
                }
                1 => {
                    let _ = parse_rfc5424(&value);
                }
                2 => {
                    let _ = parse_rfc3164(&value, 2026, 0);
                }
                3 => {
                    let _ = parse_cef_line(&value);
                }
                4 => {
                    let _ = parse_leef_line(&value);
                }
                _ => {
                    let _ = parse_combined_access(&value, "fuzz-host");
                }
            }));
            assert!(
                result.is_ok(),
                "parser {parser} panicked for input length {length}"
            );
        }
    }
}
