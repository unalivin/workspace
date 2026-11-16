use crate::platform::Env;

pub const MIN_TEXT_CONTRAST: f64 = 4.5;
pub const MIN_COMPONENT_CONTRAST: f64 = 3.0;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Color {
    pub r: u8,
    pub g: u8,
    pub b: u8,
}

pub const WHITE: Color = Color { r: 255, g: 255, b: 255 };
pub const BLACK: Color = Color { r: 0, g: 0, b: 0 };

const fn rgb(value: u32) -> Color {
    Color { r: (value >> 16) as u8, g: (value >> 8) as u8, b: value as u8 }
}

impl Color {
    pub fn parse(value: &str) -> Result<Color, String> {
        let trimmed = value.trim();
        let digits = trimmed.strip_prefix('#').unwrap_or(trimmed);
        if digits.len() != 6 || !digits.bytes().all(|byte| byte.is_ascii_hexdigit()) {
            return Err(format!("\"{value}\" is not a color: use the form #RRGGBB"));
        }
        u32::from_str_radix(digits, 16).map(rgb).map_err(|_| format!("\"{value}\" is not a color: use the form #RRGGBB"))
    }

    pub fn hex(self) -> String {
        format!("#{:02X}{:02X}{:02X}", self.r, self.g, self.b)
    }
}

fn channel(value: u8) -> f64 {
    let scaled = f64::from(value) / 255.0;
    if scaled <= 0.04045 { scaled / 12.92 } else { ((scaled + 0.055) / 1.055).powf(2.4) }
}

pub fn luminance(color: Color) -> f64 {
    0.2126 * channel(color.r) + 0.7152 * channel(color.g) + 0.0722 * channel(color.b)
}

pub fn contrast(first: Color, second: Color) -> f64 {
    let (a, b) = (luminance(first), luminance(second));
    let (lighter, darker) = if a < b { (b, a) } else { (a, b) };
    (lighter + 0.05) / (darker + 0.05)
}

pub fn mix(from: Color, to: Color, share: f64) -> Color {
    let blend = |x: u8, y: u8| (f64::from(x) * (1.0 - share) + f64::from(y) * share).round().clamp(0.0, 255.0) as u8;
    Color { r: blend(from.r, to.r), g: blend(from.g, to.g), b: blend(from.b, to.b) }
}

pub fn best_on(background: Color) -> Color {
    if contrast(WHITE, background) >= contrast(BLACK, background) { WHITE } else { BLACK }
}

fn min_contrast(color: Color, against: &[Color]) -> f64 {
    against.iter().map(|other| contrast(color, *other)).fold(f64::INFINITY, f64::min)
}

pub fn adjust_for_contrast(color: Color, against: &[Color], minimum: f64) -> (Color, bool) {
    if min_contrast(color, against) >= minimum {
        return (color, false);
    }
    let target = if min_contrast(BLACK, against) > min_contrast(WHITE, against) { BLACK } else { WHITE };
    for step in 1..=100 {
        let candidate = mix(color, target, f64::from(step) / 100.0);
        if min_contrast(candidate, against) >= minimum {
            return (candidate, true);
        }
    }
    (target, true)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Severity {
    Info,
    Low,
    Medium,
    High,
    Critical,
}

impl Severity {
    pub const ALL: [Severity; 5] = [Severity::Info, Severity::Low, Severity::Medium, Severity::High, Severity::Critical];

    pub fn parse(text: &str) -> Option<Severity> {
        let wanted = text.trim().to_lowercase();
        Severity::ALL.into_iter().find(|severity| severity.as_str() == wanted)
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Severity::Info => "info",
            Severity::Low => "low",
            Severity::Medium => "medium",
            Severity::High => "high",
            Severity::Critical => "critical",
        }
    }

    pub fn rank(self) -> usize {
        self as usize
    }

    pub fn label(self) -> &'static str {
        match self {
            Severity::Info => "Info",
            Severity::Low => "Low",
            Severity::Medium => "Medium",
            Severity::High => "High",
            Severity::Critical => "Critical",
        }
    }

    pub fn glyph(self) -> &'static str {
        match self {
            Severity::Info => "i ",
            Severity::Low => "- ",
            Severity::Medium => "* ",
            Severity::High => "! ",
            Severity::Critical => "!!",
        }
    }

    pub fn names() -> Vec<&'static str> {
        Severity::ALL.iter().map(|severity| severity.as_str()).collect()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Palette {
    pub background: Color,
    pub surface: Color,
    pub surface_alt: Color,
    pub border: Color,
    pub divider: Color,
    pub text: Color,
    pub text_muted: Color,
    pub accent: Color,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Semantic {
    pub critical: Color,
    pub high: Color,
    pub medium: Color,
    pub low: Color,
    pub info: Color,
    pub success: Color,
    pub warning: Color,
    pub error: Color,
}

impl Semantic {
    pub fn severity(&self, level: Severity) -> Color {
        match level {
            Severity::Critical => self.critical,
            Severity::High => self.high,
            Severity::Medium => self.medium,
            Severity::Low => self.low,
            Severity::Info => self.info,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Spacing {
    pub xs: f32,
    pub s: f32,
    pub m: f32,
    pub l: f32,
    pub xl: f32,
    pub row_height: f32,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TypeScale {
    pub caption: f32,
    pub body: f32,
    pub subheading: f32,
    pub heading: f32,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Tokens {
    pub name: &'static str,
    pub label: &'static str,
    pub dark: bool,
    pub palette: Palette,
    pub on_accent: Color,
    pub focus: Color,
    pub selection: Color,
    pub semantic: Semantic,
    pub spacing: Spacing,
    pub type_scale: TypeScale,
    pub radius: f32,
    pub motion: bool,
    pub warnings: Vec<String>,
}

struct Definition {
    name: &'static str,
    label: &'static str,
    light: Palette,
    dark: Palette,
}

const fn palette(values: [u32; 8]) -> Palette {
    Palette {
        background: rgb(values[0]),
        surface: rgb(values[1]),
        surface_alt: rgb(values[2]),
        border: rgb(values[3]),
        divider: rgb(values[4]),
        text: rgb(values[5]),
        text_muted: rgb(values[6]),
        accent: rgb(values[7]),
    }
}

const THEMES: [Definition; 7] = [
    Definition {
        name: "slate",
        label: "Neutral slate",
        light: palette([0xF4F5F7, 0xFFFFFF, 0xE9ECF0, 0x6B7480, 0xD5DAE1, 0x1B1F24, 0x4F5965, 0x3D4C5E]),
        dark: palette([0x14171B, 0x1C2026, 0x262B33, 0x8894A3, 0x343B45, 0xE8EBEF, 0xA9B3C0, 0x9FB4CC]),
    },
    Definition {
        name: "blue",
        label: "Blue",
        light: palette([0xF2F6FC, 0xFFFFFF, 0xE3ECF9, 0x64758F, 0xCFDCEF, 0x14202E, 0x475569, 0x1257C4]),
        dark: palette([0x0F1620, 0x16202E, 0x1E2C40, 0x7F93B0, 0x2A3B55, 0xE6EDF7, 0xA5B4C8, 0x6FA8FF]),
    },
    Definition {
        name: "green",
        label: "Green",
        light: palette([0xF2F8F4, 0xFFFFFF, 0xE1F0E6, 0x5F7A69, 0xCBE2D3, 0x14241A, 0x46584C, 0x136B38]),
        dark: palette([0x0F1913, 0x15231A, 0x1D3125, 0x7FA08B, 0x294535, 0xE6F2EA, 0xA3BBAC, 0x5FD08A]),
    },
    Definition {
        name: "crimson",
        label: "Crimson",
        light: palette([0xFBF4F5, 0xFFFFFF, 0xF5E3E6, 0x86656A, 0xEBCFD4, 0x2A1418, 0x5E464A, 0xA8112F]),
        dark: palette([0x1C1113, 0x27171A, 0x352024, 0xB08A90, 0x4A2D33, 0xF7E8EA, 0xC9AAB0, 0xFF7A90]),
    },
    Definition {
        name: "amber",
        label: "Amber",
        light: palette([0xFBF7EF, 0xFFFFFF, 0xF4EAD3, 0x7D6C48, 0xE8DAB8, 0x261D0B, 0x5A4D33, 0x8A5200]),
        dark: palette([0x1A150B, 0x241D10, 0x322916, 0xA8956A, 0x47391F, 0xF6EEDD, 0xC4B595, 0xF2B544]),
    },
    Definition {
        name: "purple",
        label: "Purple",
        light: palette([0xF7F4FC, 0xFFFFFF, 0xEBE3F8, 0x73658F, 0xDCD0F0, 0x1E1630, 0x504668, 0x6230C2]),
        dark: palette([0x15111E, 0x1D182A, 0x29223B, 0x9A8CB8, 0x3A3053, 0xEEE9F8, 0xB5A9CF, 0xB79BFF]),
    },
    Definition {
        name: "contrast",
        label: "High contrast",
        light: palette([0xFFFFFF, 0xFFFFFF, 0xE6E6E6, 0x000000, 0x000000, 0x000000, 0x1A1A1A, 0x0000C8]),
        dark: palette([0x000000, 0x000000, 0x1F1F1F, 0xFFFFFF, 0xFFFFFF, 0xFFFFFF, 0xE6E6E6, 0xFFD60A]),
    },
];

const LIGHT_SEMANTIC: Semantic = Semantic {
    critical: rgb(0xB0003A),
    high: rgb(0x7A3000),
    medium: rgb(0x636300),
    low: rgb(0x1456B8),
    info: rgb(0x525B66),
    success: rgb(0x166534),
    warning: rgb(0x7A4A00),
    error: rgb(0xB0003A),
};

const DARK_SEMANTIC: Semantic = Semantic {
    critical: rgb(0xFF7B88),
    high: rgb(0xFFA057),
    medium: rgb(0xF2CC60),
    low: rgb(0x7AB7FF),
    info: rgb(0xB6BEC9),
    success: rgb(0x56D364),
    warning: rgb(0xE3B341),
    error: rgb(0xFF7B88),
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Mode {
    #[default]
    System,
    Light,
    Dark,
}

impl Mode {
    pub fn parse(text: &str) -> Mode {
        match text {
            "light" => Mode::Light,
            "dark" => Mode::Dark,
            _ => Mode::System,
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Options {
    pub name: String,
    pub mode: Mode,
    pub system_dark: bool,
    pub accent: String,
    pub density: String,
    pub text_scale: i64,
    pub reduced_motion: bool,
}

pub fn names() -> Vec<&'static str> {
    THEMES.iter().map(|theme| theme.name).collect()
}

pub fn label(name: &str) -> &'static str {
    THEMES.iter().find(|theme| theme.name == name).map(|theme| theme.label).unwrap_or("")
}

pub fn resolve(options: &Options) -> Tokens {
    let mut warnings = Vec::new();
    let selected = match THEMES.iter().find(|theme| theme.name == options.name) {
        Some(found) => found,
        None => {
            if !options.name.is_empty() {
                warnings.push(format!("unknown theme \"{}\", using {}", options.name, THEMES[0].name));
            }
            &THEMES[0]
        }
    };
    let dark = options.mode == Mode::Dark || (options.mode != Mode::Light && options.system_dark);
    let (mut colors, semantic) = if dark { (selected.dark, DARK_SEMANTIC) } else { (selected.light, LIGHT_SEMANTIC) };
    if !options.accent.is_empty() {
        match Color::parse(&options.accent) {
            Err(problem) => warnings.push(format!("custom accent ignored: {problem}")),
            Ok(custom) => {
                let (adjusted, changed) =
                    adjust_for_contrast(custom, &[colors.background, colors.surface, colors.surface_alt], MIN_TEXT_CONTRAST);
                if changed {
                    warnings.push(format!(
                        "custom accent {} was adjusted to {} to keep a {MIN_TEXT_CONTRAST:.1}:1 contrast",
                        custom.hex(),
                        adjusted.hex()
                    ));
                }
                colors.accent = adjusted;
            }
        }
    }
    let scale = if (80..=200).contains(&options.text_scale) { options.text_scale as f32 / 100.0 } else { 1.0 };
    let spacing = if options.density == "compact" {
        Spacing { xs: 2.0, s: 4.0, m: 6.0, l: 10.0, xl: 16.0, row_height: 24.0 * scale }
    } else {
        Spacing { xs: 4.0, s: 8.0, m: 12.0, l: 16.0, xl: 24.0, row_height: 32.0 * scale }
    };
    Tokens {
        name: selected.name,
        label: selected.label,
        dark,
        palette: colors,
        on_accent: best_on(colors.accent),
        focus: colors.accent,
        selection: mix(colors.surface, colors.accent, 0.12),
        semantic,
        spacing,
        type_scale: TypeScale { caption: 11.0 * scale, body: 13.0 * scale, subheading: 15.0 * scale, heading: 20.0 * scale },
        radius: if selected.name == "contrast" { 0.0 } else { 4.0 },
        motion: !options.reduced_motion,
        warnings,
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Profile {
    Mono,
    Ansi16,
    Ansi256,
    TrueColor,
}

impl Profile {
    pub fn label(self) -> &'static str {
        match self {
            Profile::TrueColor => "truecolor",
            Profile::Ansi256 => "256 colors",
            Profile::Ansi16 => "16 colors",
            Profile::Mono => "monochrome",
        }
    }

    pub fn detect(env: &Env, monochrome: bool) -> Profile {
        let term = env.get("TERM").unwrap_or_default().to_lowercase();
        if monochrome || env.get("NO_COLOR").is_some() || term == "dumb" {
            return Profile::Mono;
        }
        let color_term = env.get("COLORTERM").unwrap_or_default().to_lowercase();
        if color_term == "truecolor" || color_term == "24bit" || env.get("WT_SESSION").is_some() {
            return Profile::TrueColor;
        }
        if term.contains("256color") { Profile::Ansi256 } else { Profile::Ansi16 }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum TermColor {
    #[default]
    Default,
    Ansi(u8),
    Rgb(Color),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Style {
    pub foreground: TermColor,
    pub background: TermColor,
    pub bold: bool,
    pub underline: bool,
    pub reverse: bool,
}

impl Style {
    fn fg(color: TermColor) -> Style {
        Style { foreground: color, ..Style::default() }
    }

    fn bold(self) -> Style {
        Style { bold: true, ..self }
    }

    fn underline(self) -> Style {
        Style { underline: true, ..self }
    }

    fn on(self, background: TermColor) -> Style {
        Style { background, ..self }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Terminal {
    pub profile: Profile,
    pub text: Style,
    pub muted: Style,
    pub accent: Style,
    pub heading: Style,
    pub selected: Style,
    pub border: Style,
    pub status_bar: Style,
    pub success: Style,
    pub warning: Style,
    pub error: Style,
    severity: [Style; 5],
}

impl Terminal {
    pub fn severity(&self, level: Severity) -> Style {
        self.severity[level.rank()]
    }
}

fn ansi_accent(name: &str, dark: bool) -> u8 {
    let index = THEMES.iter().position(|theme| theme.name == name).unwrap_or(0);
    if dark { [7, 12, 10, 9, 11, 13, 15][index] } else { [8, 4, 2, 1, 3, 5, 0][index] }
}

pub fn for_terminal(tokens: &Tokens, profile: Profile) -> Terminal {
    let plain = Style::default();
    let reverse = Style { reverse: true, ..plain };
    match profile {
        Profile::Mono => Terminal {
            profile,
            text: plain,
            muted: plain,
            accent: plain.bold(),
            heading: plain.bold().underline(),
            selected: reverse,
            border: plain,
            status_bar: reverse,
            success: plain,
            warning: plain.bold(),
            error: plain.bold().underline(),
            severity: [plain, plain, plain.bold(), plain.bold(), plain.bold().underline()],
        },
        Profile::Ansi16 => {
            let accent = TermColor::Ansi(ansi_accent(tokens.name, tokens.dark));
            let (critical, high, medium, low, success, selected_foreground) =
                if tokens.dark { (9, 11, 3, 12, 10, 0) } else { (1, 5, 3, 4, 2, 15) };
            let ansi = |code: u8| Style::fg(TermColor::Ansi(code));
            Terminal {
                profile,
                text: plain,
                muted: ansi(8),
                accent: Style::fg(accent).bold(),
                heading: Style::fg(accent).bold(),
                selected: ansi(selected_foreground).on(accent).bold(),
                border: ansi(8),
                status_bar: reverse,
                success: ansi(success),
                warning: ansi(medium).bold(),
                error: ansi(critical).bold(),
                severity: [plain, ansi(low), ansi(medium), ansi(high).bold(), ansi(critical).bold()],
            }
        }
        Profile::Ansi256 | Profile::TrueColor => {
            let colors = tokens.palette;
            let rgb = |color: Color| Style::fg(TermColor::Rgb(color));
            Terminal {
                profile,
                text: rgb(colors.text),
                muted: rgb(colors.text_muted),
                accent: rgb(colors.accent).bold(),
                heading: rgb(colors.accent).bold(),
                selected: rgb(tokens.on_accent).on(TermColor::Rgb(colors.accent)).bold(),
                border: rgb(colors.border),
                status_bar: rgb(colors.text).on(TermColor::Rgb(colors.surface_alt)),
                success: rgb(tokens.semantic.success),
                warning: rgb(tokens.semantic.warning).bold(),
                error: rgb(tokens.semantic.error).bold(),
                severity: [
                    rgb(tokens.semantic.info),
                    rgb(tokens.semantic.low),
                    rgb(tokens.semantic.medium),
                    rgb(tokens.semantic.high).bold(),
                    rgb(tokens.semantic.critical).bold(),
                ],
            }
        }
    }
}

pub fn ansi256(color: Color) -> u8 {
    let cube = |value: u8| -> u8 {
        if value < 48 {
            0
        } else if value < 115 {
            1
        } else {
            ((u16::from(value) - 35) / 40) as u8
        }
    };
    16 + 36 * cube(color.r) + 6 * cube(color.g) + cube(color.b)
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;

    fn every_variant() -> Vec<Tokens> {
        let mut variants = Vec::new();
        for name in names() {
            for mode in [Mode::Light, Mode::Dark] {
                variants.push(resolve(&Options { name: name.to_string(), mode, text_scale: 100, ..Options::default() }));
            }
        }
        variants
    }

    #[test]
    fn every_theme_meets_wcag_aa_in_light_and_dark() {
        let variants = every_variant();
        assert_eq!(variants.len(), 14);
        for tokens in variants {
            let id = format!("{} dark={}", tokens.name, tokens.dark);
            let colors = tokens.palette;
            let surfaces = [colors.background, colors.surface, colors.surface_alt];
            for surface in surfaces {
                assert!(contrast(colors.text, surface) >= MIN_TEXT_CONTRAST, "{id} text");
                assert!(contrast(colors.text_muted, surface) >= MIN_TEXT_CONTRAST, "{id} muted text");
                assert!(contrast(colors.accent, surface) >= MIN_TEXT_CONTRAST, "{id} accent");
                assert!(contrast(colors.border, surface) >= MIN_COMPONENT_CONTRAST, "{id} border");
                for severity in Severity::ALL {
                    assert!(
                        contrast(tokens.semantic.severity(severity), surface) >= MIN_TEXT_CONTRAST,
                        "{id} severity {}",
                        severity.as_str()
                    );
                }
                for color in [tokens.semantic.success, tokens.semantic.warning, tokens.semantic.error] {
                    assert!(contrast(color, surface) >= MIN_TEXT_CONTRAST, "{id} status color {}", color.hex());
                }
            }
            assert!(contrast(tokens.on_accent, colors.accent) >= MIN_TEXT_CONTRAST, "{id} text on accent");
            assert!(contrast(colors.text, tokens.selection) >= MIN_TEXT_CONTRAST, "{id} text on selection");
            assert!(contrast(tokens.focus, colors.surface) >= MIN_COMPONENT_CONTRAST, "{id} focus ring");
        }
    }

    #[test]
    fn mode_follows_the_system_only_when_asked() {
        let system = |dark| resolve(&Options { mode: Mode::System, system_dark: dark, ..Options::default() }).dark;
        assert!(system(true));
        assert!(!system(false));
        assert!(!resolve(&Options { mode: Mode::Light, system_dark: true, ..Options::default() }).dark);
        assert!(resolve(&Options { mode: Mode::Dark, system_dark: false, ..Options::default() }).dark);
        assert_eq!(Mode::parse("dark"), Mode::Dark);
        assert_eq!(Mode::parse("anything"), Mode::System);
    }

    #[test]
    fn a_weak_custom_accent_is_adjusted_and_reported() {
        let tokens = resolve(&Options { name: "slate".to_string(), mode: Mode::Light, accent: "#FFFF66".to_string(), ..Options::default() });
        assert_ne!(tokens.palette.accent.hex(), "#FFFF66");
        assert!(contrast(tokens.palette.accent, tokens.palette.surface) >= MIN_TEXT_CONTRAST);
        assert!(tokens.warnings.iter().any(|warning| warning.contains("was adjusted")), "{:?}", tokens.warnings);
        let kept = resolve(&Options { name: "slate".to_string(), mode: Mode::Light, accent: "#1257C4".to_string(), ..Options::default() });
        assert_eq!(kept.palette.accent.hex(), "#1257C4");
        assert!(kept.warnings.is_empty());
        let ignored = resolve(&Options { accent: "blue".to_string(), ..Options::default() });
        assert!(ignored.warnings.iter().any(|warning| warning.contains("ignored")));
    }

    #[test]
    fn density_text_scale_and_unknown_names_resolve_to_safe_values() {
        let compact = resolve(&Options { density: "compact".to_string(), text_scale: 150, ..Options::default() });
        let comfortable = resolve(&Options { density: "comfortable".to_string(), text_scale: 100, ..Options::default() });
        assert!(compact.spacing.m < comfortable.spacing.m);
        assert!((compact.type_scale.body - 19.5).abs() < 0.001);
        assert!((resolve(&Options { text_scale: 999, ..Options::default() }).type_scale.body - 13.0).abs() < 0.001);
        let unknown = resolve(&Options { name: "neon".to_string(), ..Options::default() });
        assert_eq!(unknown.name, "slate");
        assert_eq!(unknown.warnings.len(), 1);
        assert_eq!(resolve(&Options { name: "contrast".to_string(), ..Options::default() }).radius, 0.0);
        assert!(!resolve(&Options { reduced_motion: true, ..Options::default() }).motion);
        assert_eq!(label("contrast"), "High contrast");
        assert_eq!(label("neon"), "");
    }

    #[test]
    fn severities_are_ordered_named_and_never_color_only() {
        assert_eq!(Severity::names(), vec!["info", "low", "medium", "high", "critical"]);
        assert!(Severity::Critical > Severity::High && Severity::High.rank() == 3);
        assert_eq!(Severity::parse(" HIGH "), Some(Severity::High));
        assert_eq!(Severity::parse("urgent"), None);
        let mut glyphs: Vec<&str> = Severity::ALL.iter().map(|severity| severity.glyph()).collect();
        glyphs.dedup();
        assert_eq!(glyphs.len(), 5);
        assert!(Severity::ALL.iter().all(|severity| !severity.label().is_empty()));
    }

    #[test]
    fn terminal_profiles_are_detected_from_the_environment() {
        let detect = |pairs: &[(&str, &str)], monochrome| Profile::detect(&Env::from_pairs(pairs), monochrome);
        assert_eq!(detect(&[("TERM", "xterm-256color"), ("COLORTERM", "truecolor")], false), Profile::TrueColor);
        assert_eq!(detect(&[("TERM", "xterm-256color")], false), Profile::Ansi256);
        assert_eq!(detect(&[("TERM", "xterm")], false), Profile::Ansi16);
        assert_eq!(detect(&[("TERM", "dumb")], false), Profile::Mono);
        assert_eq!(detect(&[("TERM", "xterm-256color"), ("NO_COLOR", "1")], false), Profile::Mono);
        assert_eq!(detect(&[("TERM", "xterm-256color")], true), Profile::Mono);
        assert_eq!(detect(&[("WT_SESSION", "abc")], false), Profile::TrueColor);
        assert_eq!(Profile::Mono.label(), "monochrome");
    }

    #[test]
    fn monochrome_terminals_use_attributes_instead_of_color() {
        let tokens = resolve(&Options::default());
        let mono = for_terminal(&tokens, Profile::Mono);
        assert!(mono.selected.reverse && mono.heading.bold);
        for severity in Severity::ALL {
            let style = mono.severity(severity);
            assert_eq!((style.foreground, style.background), (TermColor::Default, TermColor::Default));
        }
        assert_ne!(mono.severity(Severity::Critical), mono.severity(Severity::Info));
        let basic = for_terminal(&tokens, Profile::Ansi16);
        assert!(matches!(basic.accent.foreground, TermColor::Ansi(_)));
        let full = for_terminal(&tokens, Profile::TrueColor);
        assert_eq!(full.text.foreground, TermColor::Rgb(tokens.palette.text));
        assert_eq!(full.severity(Severity::High).foreground, TermColor::Rgb(tokens.semantic.high));
    }

    #[test]
    fn colors_parse_print_and_map_to_the_256_color_cube() {
        assert_eq!(Color::parse("#1257c4").unwrap().hex(), "#1257C4");
        assert_eq!(Color::parse(" 1257C4 ").unwrap(), Color { r: 0x12, g: 0x57, b: 0xC4 });
        for bad in ["", "#12", "#GGGGGG", "#1257C4FF", "+12345"] {
            assert!(Color::parse(bad).is_err(), "{bad}");
        }
        assert!((contrast(WHITE, BLACK) - 21.0).abs() < 0.001);
        assert_eq!(ansi256(BLACK), 16);
        assert_eq!(ansi256(WHITE), 231);
        assert_eq!(ansi256(Color { r: 255, g: 0, b: 0 }), 196);
    }

    proptest! {
        #[test]
        fn any_accent_ends_up_readable(red in 0u8..=255, green in 0u8..=255, blue in 0u8..=255, dark in proptest::bool::ANY) {
            let accent = Color { r: red, g: green, b: blue }.hex();
            let mode = if dark { Mode::Dark } else { Mode::Light };
            for name in names() {
                let tokens = resolve(&Options { name: name.to_string(), mode, accent: accent.clone(), ..Options::default() });
                for surface in [tokens.palette.background, tokens.palette.surface, tokens.palette.surface_alt] {
                    prop_assert!(contrast(tokens.palette.accent, surface) >= MIN_TEXT_CONTRAST);
                }
                prop_assert!(contrast(tokens.on_accent, tokens.palette.accent) >= MIN_TEXT_CONTRAST);
            }
        }

        #[test]
        fn color_parsing_never_panics(text in ".{0,12}") {
            let _ = Color::parse(&text);
        }
    }
}
