use std::path::PathBuf;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AppConfig {
    pub db_path: PathBuf,
    pub ui_preference: Option<String>,
    pub density: String,
    pub text_scale: u8,
    pub reduced_motion: bool,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            db_path: PathBuf::from("logforge-siem.db"),
            ui_preference: None,
            density: "compact".to_string(),
            text_scale: 100,
            reduced_motion: false,
        }
    }
}

pub fn validate_density(density: &str) -> bool {
    matches!(density, "compact" | "comfortable")
}

pub fn validate_text_scale(scale: u8) -> bool {
    (80..=150).contains(&scale)
}
