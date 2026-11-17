use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;

use crate::error::{AppError, Result};
use crate::meta;

type Lookup = dyn Fn(&str) -> Option<String> + Send + Sync;

#[derive(Clone)]
pub struct Env(Arc<Lookup>);

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
        return Err(AppError::Usage(format!(
            "only links under {} can be opened from the application",
            meta::REPOSITORY_URL
        )));
    }
    let (program, arguments) = browser_command(os, url);
    run(&program, &arguments)
        .map_err(|error| AppError::Io(format!("the browser could not be started, copy the link instead: {error}")))
}

fn shell_quote(word: &str) -> String {
    format!("'{}'", word.replace('\'', "'\\''"))
}

pub fn terminal_commands(os: Os, executable: &str, arguments: &[String]) -> Vec<Vec<String>> {
    let owned = |words: &[&str]| words.iter().map(|word| (*word).to_string()).collect::<Vec<String>>();
    match os {
        Os::Windows => {
            let mut command = owned(&["cmd", "/c", "start", "", executable]);
            command.extend(arguments.iter().cloned());
            vec![command]
        }
        Os::Macos => {
            let mut words = vec![shell_quote(executable)];
            words.extend(arguments.iter().map(|argument| shell_quote(argument)));
            let line = words.join(" ").replace('\\', "\\\\").replace('"', "\\\"");
            vec![vec![
                "osascript".to_string(),
                "-e".to_string(),
                format!("tell application \"Terminal\" to do script \"{line}\""),
                "-e".to_string(),
                "tell application \"Terminal\" to activate".to_string(),
            ]]
        }
        Os::Linux => {
            let terminals: [&[&str]; 8] = [
                &["x-terminal-emulator", "-e"],
                &["gnome-terminal", "--"],
                &["konsole", "-e"],
                &["xfce4-terminal", "-x"],
                &["alacritty", "-e"],
                &["kitty"],
                &["foot"],
                &["xterm", "-e"],
            ];
            terminals
                .iter()
                .map(|terminal| {
                    let mut command = owned(terminal);
                    command.push(executable.to_string());
                    command.extend(arguments.iter().cloned());
                    command
                })
                .collect()
        }
    }
}

pub fn open_terminal_window(
    os: Os,
    executable: &str,
    arguments: &[String],
    run: &dyn Fn(&str, &[String]) -> Result<()>,
) -> Result<()> {
    let mut failures = Vec::new();
    for command in terminal_commands(os, executable, arguments) {
        match run(&command[0], &command[1..]) {
            Ok(()) => return Ok(()),
            Err(error) => failures.push(error.to_string()),
        }
    }
    Err(AppError::Io(format!(
        "no terminal window could be opened, start the application from a terminal with --ui tui: {}",
        failures.join("; ")
    )))
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
        assert!(
            data_dir(Os::Macos, &env)
                .unwrap()
                .ends_with(PathBuf::from("Library").join("Application Support").join(meta::APP_NAME))
        );
        let env = Env::from_pairs(&[
            ("USERPROFILE", "C:/Users/ada"),
            ("LOCALAPPDATA", "C:/Local"),
            ("APPDATA", "C:/Roaming"),
        ]);
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
    fn terminal_windows_are_tried_in_order_and_arguments_are_quoted() {
        let arguments = vec!["--ui".to_string(), "tui".to_string()];
        let windows = terminal_commands(Os::Windows, "C:/Apps/app.exe", &arguments);
        assert_eq!(windows, vec![vec!["cmd", "/c", "start", "", "C:/Apps/app.exe", "--ui", "tui"]]);
        let mac = terminal_commands(Os::Macos, "/Applications/It's \"here\"/app", &arguments);
        assert_eq!(mac[0][0], "osascript");
        assert!(mac[0][2].contains("'/Applications/It'\\\\''s \\\"here\\\"/app' '--ui' 'tui'"), "{}", mac[0][2]);
        let linux = terminal_commands(Os::Linux, "/usr/bin/app", &arguments);
        assert_eq!(linux.len(), 8);
        assert_eq!(linux[0], vec!["x-terminal-emulator", "-e", "/usr/bin/app", "--ui", "tui"]);
        assert_eq!(linux[5], vec!["kitty", "/usr/bin/app", "--ui", "tui"]);
        let tried: RefCell<Vec<String>> = RefCell::new(Vec::new());
        let third_works = |program: &str, _: &[String]| -> Result<()> {
            tried.borrow_mut().push(program.to_string());
            if program == "konsole" { Ok(()) } else { Err(AppError::Io(format!("{program} missing"))) }
        };
        open_terminal_window(Os::Linux, "/usr/bin/app", &arguments, &third_works).unwrap();
        assert_eq!(*tried.borrow(), vec!["x-terminal-emulator", "gnome-terminal", "konsole"]);
        let none_work = |program: &str, _: &[String]| -> Result<()> { Err(AppError::Io(format!("{program} missing"))) };
        let error = open_terminal_window(Os::Linux, "/usr/bin/app", &arguments, &none_work).unwrap_err().to_string();
        assert!(error.contains("--ui tui") && error.contains("xterm missing"), "{error}");
    }

    #[test]
    fn a_browser_that_does_not_start_tells_the_user_to_copy_the_link() {
        let run = |_: &str, _: &[String]| -> Result<()> { Err(AppError::Io("not found".to_string())) };
        let error = open_repository_url(Os::Linux, meta::REPOSITORY_URL, &run).unwrap_err();
        assert!(error.to_string().contains("copy the link instead"), "{error}");
    }
}
