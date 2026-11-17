use app_core::records::Status;
use app_core::theme::{self, Color, Severity, Tokens};
use egui::{
    Color32, Context, CornerRadius, FontData, FontDefinitions, FontFamily, FontId, Margin, RichText, Shadow, Stroke,
    Style, TextStyle, Theme, Visuals, vec2,
};

pub const SUBHEADING: &str = "subheading";
pub const MEDIUM: &str = "medium";
pub const ICON_SCALE: f32 = 1.3;
pub const REGULAR_FACE: &str = "NotoSans-Regular";
pub const MEDIUM_FACE: &str = "NotoSans-Medium";
static REGULAR_BYTES: &[u8] = include_bytes!("../fonts/NotoSans-Regular.ttf");
static MEDIUM_BYTES: &[u8] = include_bytes!("../fonts/NotoSans-Medium.ttf");

pub fn medium() -> FontFamily {
    FontFamily::Name(MEDIUM.into())
}

pub fn strong(text: impl Into<String>) -> RichText {
    RichText::new(text).family(medium()).strong()
}

pub fn fonts() -> FontDefinitions {
    let mut fonts = FontDefinitions::default();
    fonts.font_data.insert(REGULAR_FACE.to_string(), std::sync::Arc::new(FontData::from_static(REGULAR_BYTES)));
    fonts.font_data.insert(MEDIUM_FACE.to_string(), std::sync::Arc::new(FontData::from_static(MEDIUM_BYTES)));
    let fallback = fonts.families.get(&FontFamily::Proportional).cloned().unwrap_or_default();
    let mut emphasis = vec![MEDIUM_FACE.to_string()];
    emphasis.extend(fallback);
    fonts.families.insert(medium(), emphasis);
    fonts.families.entry(FontFamily::Proportional).or_default().insert(0, REGULAR_FACE.to_string());
    fonts
}

pub fn install(ctx: &Context) {
    ctx.set_fonts(fonts());
}

pub fn color(value: Color) -> Color32 {
    Color32::from_rgb(value.r, value.g, value.b)
}

pub fn alpha(value: Color, share: u8) -> Color32 {
    Color32::from_rgba_unmultiplied(value.r, value.g, value.b, share)
}

pub fn subheading() -> TextStyle {
    TextStyle::Name(SUBHEADING.into())
}

pub fn severity_icon(level: Severity) -> &'static str {
    match level {
        Severity::Critical => "octagon-alert",
        Severity::High => "triangle-alert",
        Severity::Medium => "circle-alert",
        Severity::Low => "circle-minus",
        Severity::Info => "info",
    }
}

pub fn status_icon(status: Status) -> &'static str {
    match status {
        Status::Open => "status-open",
        Status::InProgress => "status-progress",
        Status::Done => "status-done",
    }
}

fn corner(tokens: &Tokens) -> CornerRadius {
    CornerRadius::same(tokens.radius.round().clamp(0.0, 32.0) as u8)
}

fn inset(value: f32) -> i8 {
    value.round().clamp(0.0, 64.0) as i8
}

pub fn style(tokens: &Tokens) -> Style {
    let palette = &tokens.palette;
    let sizes = &tokens.type_scale;
    let gaps = &tokens.spacing;
    let text_styles = [
        (TextStyle::Small, FontId::new(sizes.caption, FontFamily::Proportional)),
        (TextStyle::Body, FontId::new(sizes.body, FontFamily::Proportional)),
        (TextStyle::Button, FontId::new(sizes.body, FontFamily::Proportional)),
        (TextStyle::Monospace, FontId::new(sizes.body, FontFamily::Monospace)),
        (TextStyle::Heading, FontId::new(sizes.heading, medium())),
        (subheading(), FontId::new(sizes.subheading, medium())),
    ]
    .into();
    let mut style = Style { text_styles, animation_time: if tokens.motion { 0.12 } else { 0.0 }, ..Style::default() };
    style.spacing.item_spacing = vec2(gaps.s, gaps.xs + 1.0);
    style.spacing.button_padding = vec2(gaps.m, gaps.xs);
    style.spacing.interact_size.y = gaps.row_height;
    style.spacing.window_margin = Margin::same(inset(gaps.l));
    style.spacing.menu_margin = Margin::same(inset(gaps.s));
    style.spacing.icon_width = sizes.body * 1.1;
    style.spacing.icon_width_inner = sizes.body * 0.6;
    style.interaction.selectable_labels = false;

    let radius = corner(tokens);
    let mut visuals = if tokens.dark { Visuals::dark() } else { Visuals::light() };
    visuals.dark_mode = tokens.dark;
    visuals.override_text_color = None;
    visuals.weak_text_color = Some(color(palette.text_muted));
    visuals.panel_fill = color(palette.background);
    visuals.window_fill = color(palette.surface);
    visuals.window_stroke = Stroke::new(1.0, color(palette.border));
    visuals.window_corner_radius = radius;
    visuals.menu_corner_radius = radius;
    visuals.faint_bg_color = color(palette.surface_alt);
    visuals.extreme_bg_color = color(palette.surface);
    visuals.text_edit_bg_color = Some(color(palette.surface));
    visuals.code_bg_color = color(palette.surface_alt);
    visuals.hyperlink_color = color(palette.accent);
    visuals.warn_fg_color = color(tokens.semantic.warning);
    visuals.error_fg_color = color(tokens.semantic.error);
    visuals.selection.bg_fill = color(tokens.selection);
    visuals.selection.stroke = Stroke::new(1.0, color(palette.text));
    visuals.text_cursor.stroke = Stroke::new(2.0, color(palette.text));
    visuals.text_cursor.blink = tokens.motion;
    let shade = Color32::from_black_alpha(if tokens.dark { 0x60 } else { 0x30 });
    visuals.window_shadow = Shadow { offset: [0, 6], blur: 18, spread: 0, color: shade };
    visuals.popup_shadow = Shadow { offset: [0, 3], blur: 10, spread: 0, color: shade };
    visuals.striped = false;
    visuals.button_frame = true;

    let text = Stroke::new(1.0, color(palette.text));
    let widgets = &mut visuals.widgets;
    widgets.noninteractive.bg_fill = color(palette.surface);
    widgets.noninteractive.weak_bg_fill = color(palette.surface);
    widgets.noninteractive.bg_stroke = Stroke::new(1.0, color(palette.divider));
    widgets.noninteractive.fg_stroke = text;
    widgets.noninteractive.corner_radius = radius;
    widgets.inactive.bg_fill = color(palette.surface_alt);
    widgets.inactive.weak_bg_fill = color(palette.surface_alt);
    widgets.inactive.bg_stroke = Stroke::new(1.0, color(palette.border));
    widgets.inactive.fg_stroke = text;
    widgets.inactive.corner_radius = radius;
    widgets.inactive.expansion = 0.0;
    let hover = color(theme::mix(palette.surface_alt, palette.text, 0.08));
    widgets.hovered.bg_fill = hover;
    widgets.hovered.weak_bg_fill = hover;
    widgets.hovered.bg_stroke = Stroke::new(1.0, color(palette.text_muted));
    widgets.hovered.fg_stroke = text;
    widgets.hovered.corner_radius = radius;
    widgets.hovered.expansion = 0.0;
    let pressed = color(theme::mix(palette.surface_alt, palette.text, 0.16));
    widgets.active.bg_fill = pressed;
    widgets.active.weak_bg_fill = pressed;
    widgets.active.bg_stroke = Stroke::new(2.0, color(tokens.focus));
    widgets.active.fg_stroke = text;
    widgets.active.corner_radius = radius;
    widgets.active.expansion = 0.0;
    widgets.open = widgets.hovered;
    style.visuals = visuals;
    style
}

pub fn apply(ctx: &Context, tokens: &Tokens) {
    let style = std::sync::Arc::new(style(tokens));
    ctx.set_style_of(Theme::Dark, style.clone());
    ctx.set_style_of(Theme::Light, style);
}

#[cfg(test)]
mod tests {
    use super::*;
    use app_core::theme::{Mode, Options};

    fn tokens(name: &str, mode: Mode) -> Tokens {
        theme::resolve(&Options {
            name: name.to_string(),
            mode,
            system_dark: false,
            accent: String::new(),
            density: "comfortable".to_string(),
            text_scale: 100,
            reduced_motion: false,
        })
    }

    fn back(value: Color32) -> Color {
        Color { r: value.r(), g: value.g(), b: value.b() }
    }

    #[test]
    fn every_theme_replaces_the_default_look_and_keeps_text_readable() {
        let stock = [Style::default().visuals.panel_fill, Visuals::light().panel_fill, Visuals::dark().panel_fill];
        let mut fills = std::collections::HashSet::new();
        for name in theme::names() {
            for mode in [Mode::Light, Mode::Dark] {
                let tokens = tokens(name, mode);
                let look = style(&tokens);
                let visuals = &look.visuals;
                assert_eq!(visuals.dark_mode, tokens.dark, "{name}");
                assert_eq!(visuals.panel_fill, color(tokens.palette.background), "{name}");
                assert!(!stock.contains(&visuals.panel_fill) || name == "contrast", "{name} keeps a stock fill");
                fills.insert(visuals.panel_fill);
                let pairs = [
                    (visuals.widgets.noninteractive.fg_stroke.color, visuals.panel_fill),
                    (visuals.widgets.noninteractive.fg_stroke.color, visuals.window_fill),
                    (visuals.widgets.inactive.fg_stroke.color, visuals.widgets.inactive.weak_bg_fill),
                    (visuals.widgets.hovered.fg_stroke.color, visuals.widgets.hovered.weak_bg_fill),
                    (visuals.widgets.active.fg_stroke.color, visuals.widgets.active.weak_bg_fill),
                    (visuals.selection.stroke.color, visuals.selection.bg_fill),
                ];
                for (ink, paper) in pairs {
                    let ratio = theme::contrast(back(ink), back(paper));
                    assert!(ratio >= theme::MIN_TEXT_CONTRAST, "{name} {mode:?}: {ink:?} on {paper:?} is {ratio:.2}");
                }
                let ring = theme::contrast(back(visuals.widgets.active.bg_stroke.color), back(visuals.window_fill));
                assert!(ring >= theme::MIN_COMPONENT_CONTRAST, "{name} {mode:?}: focus ring {ring:.2}");
            }
        }
        assert!(fills.len() >= 7, "only {} distinct backgrounds", fills.len());
    }

    #[test]
    fn text_scale_density_and_motion_reach_the_style() {
        let base = tokens("slate", Mode::Light);
        let mut options = Options {
            name: "slate".to_string(),
            mode: Mode::Light,
            system_dark: false,
            accent: String::new(),
            density: "compact".to_string(),
            text_scale: 150,
            reduced_motion: true,
        };
        let changed = theme::resolve(&options);
        let (plain, large) = (style(&base), style(&changed));
        assert!(large.text_styles[&TextStyle::Body].size > plain.text_styles[&TextStyle::Body].size);
        assert!(large.text_styles[&TextStyle::Heading].size > large.text_styles[&subheading()].size);
        assert_eq!(large.animation_time, 0.0);
        assert!(plain.animation_time > 0.0);
        assert!(!plain.interaction.selectable_labels);
        options.density = "comfortable".to_string();
        options.text_scale = 100;
        let roomy = style(&theme::resolve(&options));
        options.density = "compact".to_string();
        let tight = style(&theme::resolve(&options));
        assert!(tight.spacing.interact_size.y < roomy.spacing.interact_size.y);
    }

    #[test]
    fn the_embedded_faces_lead_their_families_and_headings_use_the_medium_weight() {
        let fonts = fonts();
        for face in [REGULAR_FACE, MEDIUM_FACE] {
            assert!(fonts.font_data.contains_key(face), "{face} is not embedded");
        }
        let body = &fonts.families[&FontFamily::Proportional];
        let emphasis = &fonts.families[&medium()];
        assert_eq!(body.first().map(String::as_str), Some(REGULAR_FACE));
        assert_eq!(emphasis.first().map(String::as_str), Some(MEDIUM_FACE));
        assert_eq!(body[1..], emphasis[1..], "the two families fall back to different faces");
        assert!(body.len() > 1, "no fallback face for symbols");
        for names in fonts.families.values() {
            for name in names {
                assert!(fonts.font_data.contains_key(name), "{name} has no font data");
            }
        }
        assert_ne!(REGULAR_BYTES, MEDIUM_BYTES);
        let license = include_str!("../fonts/OFL.txt");
        assert!(license.contains("SIL OPEN FONT LICENSE Version 1.1"));
        assert!(license.contains("The Noto Project Authors"));
        let notices = app_core::meta::NOTICES_TEXT;
        assert!(notices.contains("## Fonts embedded by the application"), "the notices do not name the fonts");
        assert!(notices.contains(license.trim_end()), "the notices do not reproduce the font license");
        let look = style(&tokens("slate", Mode::Light));
        assert_eq!(look.text_styles[&TextStyle::Heading].family, medium());
        assert_eq!(look.text_styles[&subheading()].family, medium());
        assert_eq!(look.text_styles[&TextStyle::Body].family, FontFamily::Proportional);
        assert!(look.text_styles[&TextStyle::Heading].size < tokens("slate", Mode::Light).type_scale.figure);
    }

    #[test]
    fn severity_and_status_icons_exist_and_differ() {
        let mut names = std::collections::HashSet::new();
        for level in Severity::ALL {
            let name = severity_icon(level);
            assert!(crate::icons::PATHS.iter().any(|(known, _)| *known == name), "{name}");
            names.insert(name);
        }
        for status in Status::ALL {
            let name = status_icon(status);
            assert!(crate::icons::PATHS.iter().any(|(known, _)| *known == name), "{name}");
            names.insert(name);
        }
        assert_eq!(names.len(), Severity::ALL.len() + Status::ALL.len());
    }
}
