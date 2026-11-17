#![no_main]

use app_core::settings::{self, DEFINITIONS};
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    let Some((first, rest)) = data.split_first() else {
        return;
    };
    let Ok(raw) = std::str::from_utf8(rest) else {
        return;
    };
    let definition = &DEFINITIONS[usize::from(*first) % DEFINITIONS.len()];
    if let Ok(once) = settings::normalize(definition, raw) {
        let twice = settings::normalize(definition, &once).expect("a normalized value stays valid");
        assert_eq!(twice, once);
    }
});
