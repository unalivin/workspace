use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;

use crate::error::{AppError, Result};
use crate::meta;

#[derive(Clone)]
pub struct Env(Arc<dyn Fn(&str) -> Option<String> + Send + Sync>);

impl Env {
    pub fn system() -> Self {
        Env(Arc::new(|name| std::env::var(name).ok()))
    }

    pub fn from_pairs(pairs: &[(&str, &str)]) -> Self {
        let values: HashMap<String, String> =
            pairs.iter().map(|(name, value)| ((*name).to_string(), (*value).to_string())).collect();
        Env(Arc::new(move |name| values.get(name).cloned()))
    }

    pub fn get(&self, name: &str) -> Option<String> {
        (self.0)(name).filter(|value| !value.trim().is_empty())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Os {
    Linux,
    Macos,
    Windows,
}

impl Os {
    pub fn current() -> Self {
        if cfg!(target_os = "windows") {
            Os::Windows
        } else if cfg!(target_os = "macos") {
            Os::Macos
        } else {
            Os::Linux
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Os::Linux => "linux",
            Os::Macos => "macos",
            Os::Windows => "windows",
        }
    }
}

fn home(env: &Env) -> Result<PathBuf> {
    env.get("HOME")
        .or_else(|| env.get("USERPROFILE"))
        .map(PathBuf::from)
        .ok_or_else(|| AppError::Io("the home directory is unknown: set HOME, or pass --db <file>".to_string()))
}

pub fn data_dir(os: Os, env: &Env) -> Result<PathBuf> {
    match os {
        Os::Linux => match env.get("XDG_DATA_HOME") {
            Some(base) => Ok(PathBuf::from(base).join(meta::SLUG)),
            None => Ok(home(env)?.join(".local").join("share").join(meta::SLUG)),
        },
        Os::Macos => Ok(home(env)?.join("Library").join("Application Support").join(meta::APP_NAME)),
        Os::Windows => match env.get("LOCALAPPDATA") {
            Some(base) => Ok(PathBuf::from(base).join(meta::APP_NAME)),
            None => Ok(home(env)?.join("AppData").join("Local").join(meta::APP_NAME)),
        },
    }
}

pub fn config_dir(os: Os, env: &Env) -> Result<PathBuf> {
    if let Some(chosen) = env.get(&meta::env_name("config_dir")) {
        return Ok(PathBuf::from(chosen));
    }
    match os {
        Os::Linux => match env.get("XDG_CONFIG_HOME") {
            Some(base) => Ok(PathBuf::from(base).join(meta::SLUG)),
            None => Ok(home(env)?.join(".config").join(meta::SLUG)),
        },
        Os::Macos => Ok(home(env)?.join("Library").join("Application Support").join(meta::APP_NAME)),
        Os::Windows => match env.get("APPDATA") {
            Some(base) => Ok(PathBuf::from(base).join(meta::APP_NAME)),
            None => Ok(home(env)?.join("AppData").join("Roaming").join(meta::APP_NAME)),
        },
    }
}

pub fn default_database_path(os: Os, env: &Env) -> Result<PathBuf> {
    Ok(data_dir(os, env)?.join(format!("{}.db", meta::SLUG)))
}

pub fn browser_command(os: Os, url: &str) -> (String, Vec<String>) {
    match os {
        Os::Windows => ("rundll32".to_string(), vec!["url.dll,FileProtocolHandler".to_string(), url.to_string()]),
        Os::Macos => ("open".to_string(), vec![url.to_string()]),
        Os::Linux => ("xdg-open".to_string(), vec![url.to_string()]),
    }
}

pub fn open_repository_url(os: Os, url: &str, run: &dyn Fn(&str, &[String]) -> Result<()>) -> Result<()> {
    if !meta::is_repository_url(url) {
        return Err(AppError::Usage(format!("only links under {} can be opened from the application", meta::REPOSITORY_URL)));
    }
    let (program, arguments) = browser_command(os, url);
    run(&program, &arguments)
        .map_err(|error| AppError::Io(format!("the browser could not be started, copy the link instead: {error}")))
}

pub fn spawn_detached(program: &str, arguments: &[String]) -> Result<()> {
    std::process::Command::new(program)
        .args(arguments)
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn()
        .map(|_| ())
        .map_err(|error| AppError::Io(format!("{program} could not be started: {error}")))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::RefCell;

    #[test]
    fn linux_follows_the_xdg_variables_and_falls_back_to_home() {
        let env = Env::from_pairs(&[("HOME", "/home/ada")]);
        assert_eq!(data_dir(Os::Linux, &env).unwrap(), PathBuf::from("/home/ada/.local/share").join(meta::SLUG));
        assert_eq!(config_dir(Os::Linux, &env).unwrap(), PathBuf::from("/home/ada/.config").join(meta::SLUG));
        let env = Env::from_pairs(&[("HOME", "/home/ada"), ("XDG_DATA_HOME", "/data"), ("XDG_CONFIG_HOME", "/config")]);
        assert_eq!(data_dir(Os::Linux, &env).unwrap(), PathBuf::from("/data").join(meta::SLUG));
        assert_eq!(config_dir(Os::Linux, &env).unwrap(), PathBuf::from("/config").join(meta::SLUG));
        assert_eq!(
            default_database_path(Os::Linux, &env).unwrap(),
            PathBuf::from("/data").join(meta::SLUG).join(format!("{}.db", meta::SLUG))
        );
    }

    #[test]
    fn macos_and_windows_use_their_application_folders() {
        let env = Env::from_pairs(&[("HOME", "/Users/ada")]);
        assert!(data_dir(Os::Macos, &env).unwrap().ends_with(PathBuf::from("Library").join("Application Support").join(meta::APP_NAME)));
        let env = Env::from_pairs(&[("USERPROFILE", "C:/Users/ada"), ("LOCALAPPDATA", "C:/Local"), ("APPDATA", "C:/Roaming")]);
        assert_eq!(data_dir(Os::Windows, &env).unwrap(), PathBuf::from("C:/Local").join(meta::APP_NAME));
        assert_eq!(config_dir(Os::Windows, &env).unwrap(), PathBuf::from("C:/Roaming").join(meta::APP_NAME));
    }

    #[test]
    fn the_configuration_directory_can_be_overridden_and_empty_values_are_ignored() {
        let name = meta::env_name("config_dir");
        let env = Env::from_pairs(&[("HOME", "/home/ada"), (name.as_str(), "/elsewhere")]);
        assert_eq!(config_dir(Os::Linux, &env).unwrap(), PathBuf::from("/elsewhere"));
        let env = Env::from_pairs(&[("HOME", "/home/ada"), (name.as_str(), "  "), ("XDG_CONFIG_HOME", "")]);
        assert_eq!(config_dir(Os::Linux, &env).unwrap(), PathBuf::from("/home/ada/.config").join(meta::SLUG));
    }

    #[test]
    fn a_missing_home_is_reported_with_the_way_out() {
        let error = data_dir(Os::Linux, &Env::from_pairs(&[])).unwrap_err();
        assert!(error.to_string().contains("--db"), "{error}");
    }

    #[test]
    fn only_repository_links_reach_the_browser() {
        let started: RefCell<Vec<String>> = RefCell::new(Vec::new());
        let run = |program: &str, arguments: &[String]| -> Result<()> {
            started.borrow_mut().push(format!("{program} {}", arguments.join(" ")));
            Ok(())
        };
        open_repository_url(Os::Linux, &meta::advisory_url(), &run).unwrap();
        open_repository_url(Os::Macos, meta::REPOSITORY_URL, &run).unwrap();
        open_repository_url(Os::Windows, meta::REPOSITORY_URL, &run).unwrap();
        assert!(open_repository_url(Os::Linux, "https://example.test/x", &run).is_err());
        let started = started.borrow();
        assert_eq!(started.len(), 3);
        assert!(started[0].starts_with("xdg-open https://"));
        assert!(started[1].starts_with("open https://"));
        assert!(started[2].starts_with("rundll32 url.dll,FileProtocolHandler https://"));
    }

    #[test]
    fn a_browser_that_does_not_start_tells_the_user_to_copy_the_link() {
        let run = |_: &str, _: &[String]| -> Result<()> { Err(AppError::Io("not found".to_string())) };
        let error = open_repository_url(Os::Linux, meta::REPOSITORY_URL, &run).unwrap_err();
        assert!(error.to_string().contains("copy the link instead"), "{error}");
    }
}
