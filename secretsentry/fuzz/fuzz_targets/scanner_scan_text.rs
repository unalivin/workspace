#![no_main]

use std::sync::LazyLock;

use app_core::redact;
use app_core::rules;
use app_core::scanner::Detector;
use libfuzzer_sys::fuzz_target;

static DETECTOR: LazyLock<Detector> =
    LazyLock::new(|| Detector::new(&rules::builtin(), &[], &[5u8; 32]).expect("the built-in rules compile"));

fuzz_target!(|data: &[u8]| {
    let content = String::from_utf8_lossy(data);
    let report = DETECTOR.scan_text("src/fuzzed.txt", &content);
    for candidate in &report.candidates {
        assert!(candidate.line >= 1 && candidate.column >= 1);
        assert!(candidate.preview.ends_with(redact::MASK));
        assert!(candidate.preview.len() <= redact::MASK.len() + redact::PREFIX_LENGTH);
        assert!(candidate.context.chars().count() <= redact::MAX_CONTEXT_LENGTH + 2);
        assert!(!candidate.context.chars().any(char::is_control));
        assert!((1..=100).contains(&candidate.score));
    }
});
