#![no_main]

use app_core::allow::{self, Allowlist, EntrySpec, Kind};
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    let Ok(text) = std::str::from_utf8(data) else {
        return;
    };
    let (glob, path) = text.split_once('\n').unwrap_or((text, "src/main.rs"));
    let spec = EntrySpec { id: 1, kind: Kind::Path, pattern: glob.to_string(), ..EntrySpec::default() };
    match allow::glob_to_pattern(glob) {
        Ok(_) => {
            let list = Allowlist::compile(&[spec]).expect("a translated pattern compiles");
            let _ = list.matching("rule", path, "value", "");
        }
        Err(_) => assert!(Allowlist::compile(&[spec]).is_err()),
    }
});
