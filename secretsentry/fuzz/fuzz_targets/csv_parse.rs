#![no_main]

use app_core::csv;
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    if let Ok(text) = std::str::from_utf8(data) {
        if let Ok(rows) = csv::parse(text, 64) {
            assert!(rows.len() <= 64);
        }
    }
});
