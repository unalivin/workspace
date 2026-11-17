#![no_main]

use app_core::notify;
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    if let Ok(text) = std::str::from_utf8(data) {
        let cleaned = notify::safe_title(text);
        assert!(!cleaned.is_empty());
        assert!(cleaned.chars().count() <= notify::MAX_TITLE_LENGTH);
        assert!(!cleaned.chars().any(char::is_control));
    }
});
