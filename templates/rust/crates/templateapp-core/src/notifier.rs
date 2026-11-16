use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::clock::{Clock, Zone};
use crate::error::{AppError, Result};
use crate::files;
use crate::meta;
use crate::notify::{self, Notifications};
use crate::platform::{Env, Os};
use crate::store::{Options, Store};

pub const MIN_INTERVAL_SECONDS: i64 = 15;
pub const MAX_INTERVAL_SECONDS: i64 = 3600;
const BATCH_LIMIT: i64 = 50;
const STATE_FILE_NAME: &str = "notifier-state.json";

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
struct State {
    last_id: i64,
    initialized: bool,
}

pub fn clamp_interval(seconds: i64) -> i64 {
    seconds.clamp(MIN_INTERVAL_SECONDS, MAX_INTERVAL_SECONDS)
}

pub type Deliver<'a> = dyn Fn(&str, &str) -> Result<()> + 'a;

pub struct Runner<'a> {
    pub database: PathBuf,
    pub state_dir: PathBuf,
    pub show_titles: bool,
    pub clock: Clock,
    pub deliver: &'a Deliver<'a>,
}

impl Runner<'_> {
    fn state_path(&self) -> PathBuf {
        self.state_dir.join(STATE_FILE_NAME)
    }

    fn load_state(&self) -> Result<State> {
        let path = self.state_path();
        if !path.exists() {
            return Ok(State::default());
        }
        let raw = fs::read(&path).map_err(|error| AppError::Io(format!("cannot read the notifier state: {error}")))?;
        if raw.len() > 4096 {
            return Ok(State::default());
        }
        Ok(serde_json::from_slice::<State>(&raw).ok().filter(|state| state.last_id >= 0).unwrap_or_default())
    }

    fn save_state(&self, state: State) -> Result<()> {
        files::create_private_dir(&self.state_dir)?;
        files::write_private(&self.state_path(), &serde_json::to_vec(&state)?, true)
    }

    pub fn tick(&self) -> Result<usize> {
        let store = Store::open(&self.database, Options { read_only: true, busy_timeout: None })
            .map_err(|error| AppError::Storage(format!("cannot open the database read-only: {error}")))?;
        let source = Notifications::new(store.conn(), self.clock.clone(), Zone::Utc);
        let current = self.load_state()?;
        if !current.initialized {
            self.save_state(State { last_id: source.highest_id()?, initialized: true })?;
            return Ok(0);
        }
        let items = source.after(current.last_id, BATCH_LIMIT)?;
        let Some(last) = items.last() else {
            return Ok(0);
        };
        let last_id = last.id;
        let fresh: Vec<_> = items.into_iter().filter(|item| item.unread()).collect();
        if !fresh.is_empty() {
            let (title, body) = notify::summary(&fresh, self.show_titles);
            (self.deliver)(&title, &body)
                .map_err(|error| AppError::Io(format!("cannot show the system notification: {error}")))?;
        }
        self.save_state(State { last_id, initialized: true })?;
        Ok(fresh.len())
    }

    pub fn run(&self, interval_seconds: i64, max_ticks: usize, wait: &dyn Fn(i64) -> bool, log: &dyn Fn(&str)) {
        let interval = clamp_interval(interval_seconds);
        let mut tick = 0;
        while max_ticks == 0 || tick < max_ticks {
            if let Err(error) = self.tick() {
                log(&error.to_string());
            }
            if !wait(interval) {
                return;
            }
            tick += 1;
        }
    }
}

const WINDOWS_SCRIPT: &str = "Add-Type -AssemblyName System.Windows.Forms; $n = New-Object System.Windows.Forms.NotifyIcon; $n.Icon = [System.Drawing.SystemIcons]::Information; $n.Visible = $true; $n.ShowBalloonTip(8000, $env:APP_NOTIFY_TITLE, $env:APP_NOTIFY_BODY, [System.Windows.Forms.ToolTipIcon]::Info); Start-Sleep -Seconds 9; $n.Dispose()";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeliveryCommand {
    pub program: String,
    pub arguments: Vec<String>,
    pub environment: Vec<(String, String)>,
}

pub fn delivery_command(os: Os, title: &str, body: &str) -> DeliveryCommand {
    let (title, body) = (notify::safe_title(title), notify::safe_title(body));
    let owned = |words: &[&str]| words.iter().map(|word| (*word).to_string()).collect::<Vec<String>>();
    match os {
        Os::Windows => DeliveryCommand {
            program: "powershell".to_string(),
            arguments: owned(&["-NoProfile", "-NonInteractive", "-WindowStyle", "Hidden", "-Command", WINDOWS_SCRIPT]),
            environment: vec![("APP_NOTIFY_TITLE".to_string(), title), ("APP_NOTIFY_BODY".to_string(), body)],
        },
        Os::Macos => DeliveryCommand {
            program: "osascript".to_string(),
            arguments: owned(&[
                "-e",
                "on run argv",
                "-e",
                "display notification (item 2 of argv) with title (item 1 of argv)",
                "-e",
                "end run",
                "--",
                &title,
                &body,
            ]),
            environment: Vec::new(),
        },
        Os::Linux => DeliveryCommand {
            program: "notify-send".to_string(),
            arguments: owned(&["--app-name", &title, "--", &title, &body]),
            environment: Vec::new(),
        },
    }
}

pub fn deliver_with_system(os: Os, title: &str, body: &str) -> Result<()> {
    let command = delivery_command(os, title, body);
    let status = std::process::Command::new(&command.program)
        .args(&command.arguments)
        .envs(command.environment.iter().map(|(name, value)| (name.as_str(), value.as_str())))
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .map_err(|error| {
            AppError::Io(format!(
                "{} could not be started, so system notifications cannot be shown on this computer: {error}",
                command.program
            ))
        })?;
    if status.success() { Ok(()) } else { Err(AppError::Io(format!("{} ended with {status}", command.program))) }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Autostart {
    pub os: Os,
    pub home_dir: String,
    pub config_home: String,
    pub app_data: String,
    pub executable: String,
}

impl Autostart {
    pub fn for_system(os: Os, env: &Env, executable: &Path) -> Autostart {
        Autostart {
            os,
            home_dir: env.get("HOME").or_else(|| env.get("USERPROFILE")).unwrap_or_default(),
            config_home: env.get("XDG_CONFIG_HOME").unwrap_or_default(),
            app_data: env.get("APPDATA").unwrap_or_default(),
            executable: executable.to_string_lossy().into_owned(),
        }
    }

    fn label(&self) -> String {
        format!("{}.notifier", meta::APP_ID)
    }

    pub fn path(&self) -> Result<PathBuf> {
        let unknown =
            || AppError::Io("the home directory is unknown, so the autostart entry cannot be placed".to_string());
        match self.os {
            Os::Windows => {
                let base = if !self.app_data.is_empty() {
                    PathBuf::from(&self.app_data)
                } else if !self.home_dir.is_empty() {
                    Path::new(&self.home_dir).join("AppData").join("Roaming")
                } else {
                    return Err(unknown());
                };
                Ok(base
                    .join("Microsoft")
                    .join("Windows")
                    .join("Start Menu")
                    .join("Programs")
                    .join("Startup")
                    .join(format!("{}-notifier.cmd", meta::SLUG)))
            }
            Os::Macos if self.home_dir.is_empty() => Err(unknown()),
            Os::Macos => Ok(Path::new(&self.home_dir)
                .join("Library")
                .join("LaunchAgents")
                .join(format!("{}.plist", self.label()))),
            Os::Linux if self.home_dir.is_empty() => Err(unknown()),
            Os::Linux => {
                let base = if Path::new(&self.config_home).is_absolute() {
                    PathBuf::from(&self.config_home)
                } else {
                    Path::new(&self.home_dir).join(".config")
                };
                Ok(base.join("autostart").join(format!("{}-notifier.desktop", meta::SLUG)))
            }
        }
    }

    fn check_executable(&self) -> Result<()> {
        let bytes = self.executable.as_bytes();
        let drive = self.os == Os::Windows && bytes.len() > 2 && bytes[0].is_ascii_alphabetic() && bytes[1] == b':';
        if self.executable.is_empty() || !(self.executable.starts_with('/') || drive) {
            return Err(AppError::Usage(format!("the executable path \"{}\" must be absolute", self.executable)));
        }
        if self.executable.chars().any(|letter| letter.is_control() || matches!(letter, '"' | '`' | '$' | '%')) {
            return Err(AppError::Usage(format!(
                "the executable path \"{}\" contains a character that cannot be used in an autostart entry: move the application to a simpler path",
                self.executable
            )));
        }
        Ok(())
    }

    pub fn content(&self) -> Result<String> {
        self.check_executable()?;
        Ok(match self.os {
            Os::Windows => format!("@echo off\r\nstart \"\" /min \"{}\" notifier run\r\n", self.executable),
            Os::Macos => {
                let escaped = self.executable.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;");
                format!(
                    "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<!DOCTYPE plist PUBLIC \"-//Apple//DTD PLIST 1.0//EN\" \"http://www.apple.com/DTDs/PropertyList-1.0.dtd\">\n<plist version=\"1.0\">\n<dict>\n  <key>Label</key>\n  <string>{}</string>\n  <key>ProgramArguments</key>\n  <array>\n    <string>{escaped}</string>\n    <string>notifier</string>\n    <string>run</string>\n  </array>\n  <key>RunAtLoad</key>\n  <true/>\n  <key>ProcessType</key>\n  <string>Background</string>\n</dict>\n</plist>\n",
                    self.label()
                )
            }
            Os::Linux => format!(
                "[Desktop Entry]\nType=Application\nName={name} notifier\nComment=Shows {name} notifications\nExec=\"{}\" notifier run\nTerminal=false\nNoDisplay=true\nX-GNOME-Autostart-enabled=true\n",
                self.executable.replace('\\', "\\\\"),
                name = meta::APP_NAME
            ),
        })
    }

    pub fn install(&self, dry_run: bool) -> Result<PathBuf> {
        let path = self.path()?;
        let content = self.content()?;
        if !dry_run {
            if let Some(parent) = path.parent() {
                fs::create_dir_all(parent)
                    .map_err(|error| AppError::Io(format!("cannot create \"{}\": {error}", parent.display())))?;
            }
            files::write_private(&path, content.as_bytes(), true)?;
        }
        Ok(path)
    }

    pub fn uninstall(&self, dry_run: bool) -> Result<(bool, PathBuf)> {
        let path = self.path()?;
        if !path.exists() {
            return Ok((false, path));
        }
        if !dry_run {
            fs::remove_file(&path)
                .map_err(|error| AppError::Io(format!("cannot remove \"{}\": {error}", path.display())))?;
        }
        Ok((true, path))
    }

    pub fn status(&self) -> Result<(bool, PathBuf)> {
        let path = self.path()?;
        Ok((path.is_file(), path))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::records::{Input, Records, Status};
    use crate::testkit::{TempDir, fixed_time};
    use crate::theme::Severity;
    use std::cell::RefCell;

    fn urgent(records: &Records<'_>, name: &str) {
        records
            .create(&Input {
                name: name.to_string(),
                status: Status::Open,
                severity: Severity::Critical,
                ..Input::default()
            })
            .unwrap();
    }

    #[test]
    fn the_first_tick_only_records_the_position_then_new_items_are_announced_once() {
        let directory = TempDir::new("notifier");
        let database = directory.path().join("app.db");
        let store = Store::open_ready(&database).unwrap();
        let clock = Clock::fixed(fixed_time());
        let records = Records::new(store.conn(), clock.clone(), Zone::Utc);
        urgent(&records, "before the notifier started");
        let shown: RefCell<Vec<(String, String)>> = RefCell::new(Vec::new());
        let deliver = |title: &str, body: &str| -> Result<()> {
            shown.borrow_mut().push((title.to_string(), body.to_string()));
            Ok(())
        };
        let runner = Runner {
            database: database.clone(),
            state_dir: directory.path().join("state"),
            show_titles: false,
            clock,
            deliver: &deliver,
        };
        assert_eq!(runner.tick().unwrap(), 0);
        assert_eq!(runner.tick().unwrap(), 0);
        assert!(shown.borrow().is_empty());
        urgent(&records, "second");
        urgent(&records, "third");
        assert_eq!(runner.tick().unwrap(), 2);
        assert_eq!(runner.tick().unwrap(), 0);
        assert_eq!(*shown.borrow(), vec![(meta::APP_NAME.to_string(), "2 new notifications".to_string())]);
        urgent(&records, "fourth");
        Notifications::new(store.conn(), Clock::fixed(fixed_time()), Zone::Utc).mark_all_read().unwrap();
        assert_eq!(runner.tick().unwrap(), 0);
        assert_eq!(shown.borrow().len(), 1);
    }

    #[test]
    fn a_failed_delivery_is_retried_and_a_broken_state_file_starts_over() {
        let directory = TempDir::new("notifier-retry");
        let database = directory.path().join("app.db");
        let store = Store::open_ready(&database).unwrap();
        let clock = Clock::fixed(fixed_time());
        let records = Records::new(store.conn(), clock.clone(), Zone::Utc);
        let failing = |_: &str, _: &str| -> Result<()> { Err(AppError::Io("no notification service".to_string())) };
        let state_dir = directory.path().join("state");
        let broken = Runner {
            database: database.clone(),
            state_dir: state_dir.clone(),
            show_titles: true,
            clock: clock.clone(),
            deliver: &failing,
        };
        broken.tick().unwrap();
        urgent(&records, "needs delivery");
        let error = broken.tick().unwrap_err().to_string();
        assert!(error.contains("cannot show the system notification"), "{error}");
        let delivered: RefCell<Vec<String>> = RefCell::new(Vec::new());
        let working = |_: &str, body: &str| -> Result<()> {
            delivered.borrow_mut().push(body.to_string());
            Ok(())
        };
        let fixed = Runner {
            database: database.clone(),
            state_dir: state_dir.clone(),
            show_titles: true,
            clock: clock.clone(),
            deliver: &working,
        };
        assert_eq!(fixed.tick().unwrap(), 1);
        assert_eq!(*delivered.borrow(), vec!["A record needs attention".to_string()]);
        fs::write(state_dir.join(STATE_FILE_NAME), "{broken").unwrap();
        assert_eq!(fixed.tick().unwrap(), 0);
        assert_eq!(delivered.borrow().len(), 1);
        let missing = Runner {
            database: directory.path().join("missing.db"),
            state_dir,
            show_titles: true,
            clock,
            deliver: &working,
        };
        assert!(missing.tick().unwrap_err().to_string().contains("read-only"));
    }

    #[test]
    fn the_loop_clamps_the_interval_logs_errors_and_stops_when_told() {
        let directory = TempDir::new("notifier-loop");
        let deliver = |_: &str, _: &str| -> Result<()> { Ok(()) };
        let runner = Runner {
            database: directory.path().join("missing.db"),
            state_dir: directory.path().to_path_buf(),
            show_titles: false,
            clock: Clock::fixed(fixed_time()),
            deliver: &deliver,
        };
        let waits: RefCell<Vec<i64>> = RefCell::new(Vec::new());
        let logged: RefCell<Vec<String>> = RefCell::new(Vec::new());
        let wait = |seconds: i64| {
            waits.borrow_mut().push(seconds);
            true
        };
        let log = |line: &str| logged.borrow_mut().push(line.to_string());
        runner.run(1, 3, &wait, &log);
        assert_eq!(*waits.borrow(), vec![15, 15, 15]);
        assert_eq!(logged.borrow().len(), 3);
        let stop = |_: i64| false;
        runner.run(99999, 0, &stop, &log);
        assert_eq!(logged.borrow().len(), 4);
        assert_eq!((clamp_interval(5), clamp_interval(60), clamp_interval(99999)), (15, 60, 3600));
    }

    #[test]
    fn delivery_commands_pass_text_as_data_never_as_code() {
        let hostile = "\"; rm -rf ~ #$(id)`id`";
        let linux = delivery_command(Os::Linux, "Title", hostile);
        assert_eq!(linux.program, "notify-send");
        assert_eq!(linux.arguments[2], "--");
        assert_eq!(linux.arguments[4], notify::safe_title(hostile));
        let mac = delivery_command(Os::Macos, "Title", hostile);
        assert!(mac.arguments[..6].iter().all(|argument| !argument.contains("rm -rf")));
        assert_eq!(mac.arguments[6], "--");
        let windows = delivery_command(Os::Windows, "Title", hostile);
        assert!(windows.arguments.iter().all(|argument| !argument.contains("rm -rf")));
        assert_eq!(windows.environment[1], ("APP_NOTIFY_BODY".to_string(), notify::safe_title(hostile)));
    }

    fn autostart(os: Os, home: &str, executable: &str) -> Autostart {
        Autostart {
            os,
            home_dir: home.to_string(),
            config_home: String::new(),
            app_data: String::new(),
            executable: executable.to_string(),
        }
    }

    #[test]
    fn autostart_entries_are_placed_and_written_per_platform() {
        let linux = autostart(Os::Linux, "/home/ada", "/opt/app/templateapp");
        assert!(
            linux
                .path()
                .unwrap()
                .ends_with(Path::new(".config").join("autostart").join(format!("{}-notifier.desktop", meta::SLUG)))
        );
        assert!(linux.content().unwrap().contains("Exec=\"/opt/app/templateapp\" notifier run\n"));
        let mac = autostart(Os::Macos, "/Users/ada", "/Applications/A&B/app");
        assert!(
            mac.path()
                .unwrap()
                .ends_with(Path::new("Library").join("LaunchAgents").join(format!("{}.notifier.plist", meta::APP_ID)))
        );
        assert!(mac.content().unwrap().contains("<string>/Applications/A&amp;B/app</string>"));
        let windows = autostart(Os::Windows, "", "C:\\Apps\\app.exe");
        assert!(windows.path().is_err());
        let windows = Autostart { app_data: "C:/Roaming".to_string(), ..windows };
        assert!(windows.path().unwrap().ends_with(Path::new("Startup").join(format!("{}-notifier.cmd", meta::SLUG))));
        assert_eq!(windows.content().unwrap(), "@echo off\r\nstart \"\" /min \"C:\\Apps\\app.exe\" notifier run\r\n");
        assert!(autostart(Os::Linux, "", "/opt/app").path().is_err());
        for bad in ["", "relative/app", "/opt/a\"b", "/opt/$HOME/app", "/opt/a`b", "/opt/100%/app", "/opt/a\nb"] {
            assert!(autostart(Os::Linux, "/home/ada", bad).content().is_err(), "{bad}");
        }
    }

    #[test]
    fn install_status_and_uninstall_follow_the_file() {
        let directory = TempDir::new("autostart");
        let home = directory.path().to_string_lossy().into_owned();
        let entry = autostart(Os::Linux, &home, "/opt/app/templateapp");
        assert!(!entry.status().unwrap().0);
        let planned = entry.install(true).unwrap();
        assert!(!planned.exists());
        let written = entry.install(false).unwrap();
        assert_eq!(written, planned);
        assert_eq!(fs::read_to_string(&written).unwrap(), entry.content().unwrap());
        assert_eq!(entry.status().unwrap(), (true, written.clone()));
        assert_eq!(entry.uninstall(true).unwrap(), (true, written.clone()));
        assert!(written.exists());
        assert_eq!(entry.uninstall(false).unwrap(), (true, written.clone()));
        assert_eq!(entry.uninstall(false).unwrap(), (false, written));
    }
}
