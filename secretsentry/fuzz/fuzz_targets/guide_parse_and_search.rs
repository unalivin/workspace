#![no_main]

use app_core::guide;
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    let Ok(text) = std::str::from_utf8(data) else {
        return;
    };
    let (query, markdown) = match text.split_once('\n') {
        Some(parts) => parts,
        None => ("", text),
    };
    let parsed = guide::parse(markdown);
    let _ = parsed.search(query);
    let _ = guide::plain(markdown);
    let mut anchors: Vec<&str> = parsed.sections.iter().map(|section| section.anchor.as_str()).collect();
    let total = anchors.len();
    anchors.sort_unstable();
    anchors.dedup();
    assert_eq!(anchors.len(), total);
});
