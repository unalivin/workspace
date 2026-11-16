use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::error::{AppError, Result};
use crate::files;

pub const FRONT_END_GUI: &str = "gui";
pub const FRONT_END_TUI: &str = "tui";
const FILE_NAME: &str = "config.json";
const MAX_FILE_SIZE: u64 = 64 * 1024;

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Config {
    pub database_path: String,
    pub front_end: String,
}

impl Config {
    pub fn validate(&self) -> Result<()> {
        if !matches!(self.front_end.as_str(), "" | FRONT_END_GUI | FRONT_END_TUI) {
            return Err(AppError::Usage(format!(
                "front_end must be \"{FRONT_END_GUI}\", \"{FRONT_END_TUI}\" or empty, found \"{}\"",
                self.front_end
            )));
        }
        if !self.database_path.is_empty() && !Path::new(&self.database_path).is_absolute() {
            return Err(AppError::Usage(format!(
                "database_path must be an absolute path, found \"{}\"",
                self.database_path
            )));
        }
        Ok(())
    }
}

pub fn file(directory: &Path) -> PathBuf {
    directory.join(FILE_NAME)
}

pub fn load(directory: &Path) -> Result<Config> {
    let path = file(directory);
    if !path.exists() {
        return Ok(Config::default());
    }
    let raw = files::read_limited(&path, MAX_FILE_SIZE)
        .map_err(|error| AppError::Io(format!("{error}: delete it to start with defaults")))?;
    let config: Config = serde_json::from_slice(&raw).map_err(|error| {
        AppError::Io(format!("\"{}\" is not valid JSON: delete it to start with defaults ({error})", path.display()))
    })?;
    config.validate().map_err(|error| AppError::Io(format!("\"{}\": {error}", path.display())))?;
    Ok(config)
}

pub fn save(directory: &Path, config: &Config) -> Result<()> {
    config.validate()?;
    files::create_private_dir(directory)?;
    let mut raw = serde_json::to_vec_pretty(config)?;
    raw.push(b'\n');
    files::write_private(&file(directory), &raw, true)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testkit::TempDir;
    use std::fs;

    #[test]
    fn a_missing_file_means_defaults_and_saved_values_come_back() {
        let directory = TempDir::new("config");
        let nested = directory.path().join("settings");
        assert_eq!(load(&nested).unwrap(), Config::default());
        let config = Config {
            database_path: directory.path().join("x.db").to_string_lossy().into_owned(),
            front_end: "tui".to_string(),
        };
        save(&nested, &config).unwrap();
        assert_eq!(load(&nested).unwrap(), config);
        let text = fs::read_to_string(file(&nested)).unwrap();
        assert!(text.contains("\"front_end\": \"tui\"") && text.ends_with('\n'), "{text}");
    }

    #[test]
    fn broken_files_are_reported_with_the_way_out() {
        let directory = TempDir::new("config-broken");
        fs::write(file(directory.path()), "{not json").unwrap();
        assert!(load(directory.path()).unwrap_err().to_string().contains("delete it to start with defaults"));
        fs::write(file(directory.path()), "{\"front_end\":\"web\"}").unwrap();
        assert!(load(directory.path()).unwrap_err().to_string().contains("front_end must be"));
        fs::write(file(directory.path()), "{\"database_path\":\"relative.db\"}").unwrap();
        assert!(load(directory.path()).unwrap_err().to_string().contains("absolute path"));
        fs::write(file(directory.path()), vec![b' '; (MAX_FILE_SIZE + 1) as usize]).unwrap();
        assert!(load(directory.path()).unwrap_err().to_string().contains("larger than"));
        fs::write(file(directory.path()), "{\"later_option\":1}").unwrap();
        assert_eq!(load(directory.path()).unwrap(), Config::default());
        assert!(save(directory.path(), &Config { front_end: "web".to_string(), ..Config::default() }).is_err());
    }
}
