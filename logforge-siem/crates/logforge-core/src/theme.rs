use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ThemeMode {
    Light,
    Dark,
    System,
    HighContrast,
}

impl ThemeMode {
    pub fn parse(s: &str) -> Option<Self> {
        match s.trim().to_ascii_lowercase().as_str() {
            "light" => Some(ThemeMode::Light),
            "dark" => Some(ThemeMode::Dark),
            "system" => Some(ThemeMode::System),
            "high-contrast" | "high_contrast" => Some(ThemeMode::HighContrast),
            _ => None,
        }
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            ThemeMode::Light => "light",
            ThemeMode::Dark => "dark",
            ThemeMode::System => "system",
            ThemeMode::HighContrast => "high-contrast",
        }
    }
}

pub const CATEGORICAL_LIGHT: [&str; 8] = [
    "#2B6CD4", "#D9730D", "#1B9A66", "#C42E67", "#7449D1", "#0E94A3", "#A88400", "#66727F",
];

pub const CATEGORICAL_DARK: [&str; 8] = [
    "#6EA2F2", "#F2A04D", "#4CC796", "#F07AA3", "#A58AF0", "#4CC4D1", "#E0BC3A", "#A1ABB6",
];

pub fn severity_color(severity: &str, dark: bool) -> &'static str {
    match (severity, dark) {
        ("critical", false) => "#B3261E",
        ("high" | "error", false) => "#C2410C",
        ("medium" | "warn", false) => "#A16207",
        ("low", false) => "#2F7D4F",
        (_, false) => "#2B6CB0",
        ("critical", true) => "#FF8A80",
        ("high" | "error", true) => "#FFA463",
        ("medium" | "warn", true) => "#F2C94C",
        ("low", true) => "#6FD39A",
        (_, true) => "#7DB4F0",
    }
}
