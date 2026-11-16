#![no_main]

use app_core::theme::{self, Color, Mode, Options, MIN_TEXT_CONTRAST};
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    if let Ok(text) = std::str::from_utf8(data) {
        let _ = Color::parse(text);
    }
    let [red, green, blue, dark, ..] = *data else {
        return;
    };
    let accent = Color { r: red, g: green, b: blue }.hex();
    let mode = if dark % 2 == 1 { Mode::Dark } else { Mode::Light };
    for name in theme::names() {
        let tokens = theme::resolve(&Options { name: name.to_string(), mode, accent: accent.clone(), ..Options::default() });
        for surface in [tokens.palette.background, tokens.palette.surface, tokens.palette.surface_alt] {
            assert!(theme::contrast(tokens.palette.accent, surface) >= MIN_TEXT_CONTRAST);
        }
        assert!(theme::contrast(tokens.on_accent, tokens.palette.accent) >= MIN_TEXT_CONTRAST);
    }
});
