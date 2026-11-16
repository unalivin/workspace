#![no_main]

use app_core::transfer::{self, Format};
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    let _ = transfer::parse(data, Format::Csv);
    let _ = transfer::parse(data, Format::Json);
});
