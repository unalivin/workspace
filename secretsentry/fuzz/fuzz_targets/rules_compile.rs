#![no_main]

use app_core::rules::{Rule, RuleSpec};
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    let Some((first, rest)) = data.split_first() else {
        return;
    };
    let Ok(pattern) = std::str::from_utf8(rest) else {
        return;
    };
    let spec = RuleSpec {
        id: "fuzzed".to_string(),
        name: "Fuzzed".to_string(),
        pattern: pattern.to_string(),
        secret_group: usize::from(*first % 4),
        ..RuleSpec::default()
    };
    if let Ok(rule) = Rule::compile(&spec) {
        assert!(!rule.matcher().is_match(""));
        assert!(rule.spec().secret_group < rule.matcher().captures_len());
    }
});
