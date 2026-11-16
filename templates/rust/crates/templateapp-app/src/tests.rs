use std::cell::RefCell;
use std::collections::BTreeMap;
use std::io::Cursor;
use std::path::PathBuf;

use app_core::clock::{Clock, Zone};
use app_core::launcher::{PromptResult, Session};
use app_core::meta;
use app_core::notifier::Autostart;
use app_core::platform::{self, Os};
use app_core::service::{self, App, Options};
use app_core::testkit::{TempDir, fixed_time};
use app_core::{AppError, Result};

use crate::cli::{self, Env, Hooks, Kind, number, parse_flags, switch, tabulate, text};
use crate::{EXIT_FAILURE, EXIT_OK, EXIT_USAGE};

struct Harness {
    home: TempDir,
    variables: BTreeMap<String, String>,
    stdin: String,
    terminal: bool,
    gui: Option<Vec<&'static str>>,
    tui: Option<Vec<&'static str>>,
    chooser: Option<PromptResult>,
    window: bool,
    opened: RefCell<Vec<String>>,
    delivered: RefCell<Vec<String>>,
}

impl Harness {
    fn new(label: &str) -> Harness {
        let home = TempDir::new(label);
        let variables = BTreeMap::from([("HOME".to_string(), home.path().to_string_lossy().into_owned())]);
        Harness {
            home,
            variables,
            stdin: String::new(),
            terminal: false,
            gui: None,
            tui: None,
            chooser: None,
            window: false,
            opened: RefCell::new(Vec::new()),
            delivered: RefCell::new(Vec::new()),
        }
    }

    fn set(&mut self, name: &str, value: &str) {
        self.variables.insert(name.to_string(), value.to_string());
    }

    fn environment(&self) -> platform::Env {
        let pairs: Vec<(&str, &str)> = self.variables.iter().map(|(name, value)| (name.as_str(), value.as_str())).collect();
        platform::Env::from_pairs(&pairs)
    }

    fn options(&self) -> Options {
        Options {
            os: Os::Linux,
            env: self.environment(),
            config_dir: None,
            database_path: None,
            overrides: BTreeMap::new(),
            clock: Clock::fixed(fixed_time()),
            zone: Zone::Utc,
            read_only: false,
        }
    }

    fn executable(&self) -> PathBuf {
        self.home.path().join("bin").join(meta::SLUG)
    }

    fn path(&self, name: &str) -> String {
        self.home.path().join(name).to_string_lossy().into_owned()
    }

    fn run(&self, arguments: &[&str]) -> (String, String, i32) {
        let mut input = Cursor::new(self.stdin.clone().into_bytes());
        let mut output: Vec<u8> = Vec::new();
        let mut errors: Vec<u8> = Vec::new();
        let front_end = |name: &'static str, script: &'static [&'static str]| {
            move |_: &mut App| -> Result<String> {
                let visit = self.opened.borrow().iter().filter(|entry| entry.as_str() == name).count();
                self.opened.borrow_mut().push(name.to_string());
                Ok(script.get(visit).copied().unwrap_or("").to_string())
            }
        };
        let gui_script: &'static [&'static str] = self.gui.clone().map(|script| &*script.leak()).unwrap_or(&[]);
        let tui_script: &'static [&'static str] = self.tui.clone().map(|script| &*script.leak()).unwrap_or(&[]);
        let open_gui = front_end("gui", gui_script);
        let open_tui = front_end("tui", tui_script);
        let chooser = |default: &str| -> Result<PromptResult> {
            self.opened.borrow_mut().push(format!("chooser default {default}"));
            self.chooser.clone().ok_or_else(|| AppError::Io("no chooser answer".to_string()))
        };
        let window = |executable: &str, arguments: &[String]| -> Result<()> {
            self.opened.borrow_mut().push(format!("window {executable} {}", arguments.join(" ")));
            Ok(())
        };
        let deliver = |title: &str, body: &str| -> Result<()> {
            self.delivered.borrow_mut().push(format!("{title} | {body}"));
            Ok(())
        };
        let home = self.home.path().to_string_lossy().into_owned();
        let executable = self.executable().to_string_lossy().into_owned();
        let autostart = || Autostart {
            os: Os::Linux,
            home_dir: home.clone(),
            config_home: String::new(),
            app_data: String::new(),
            executable: "/opt/app/templateapp".to_string(),
        };
        let wait = |_: i64| false;
        let code = cli::run(&mut Env {
            args: arguments.iter().map(|argument| (*argument).to_string()).collect(),
            stdin: &mut input,
            stdout: &mut output,
            stderr: &mut errors,
            options: self.options(),
            session: Session {
                os: Os::Linux,
                env: self.environment(),
                stdin_terminal: self.terminal,
                stdout_terminal: self.terminal,
                stderr_terminal: self.terminal,
                console_owners: 0,
            },
            executable,
            hooks: Hooks {
                open_gui: if self.gui.is_some() { Some(&open_gui) } else { None },
                open_tui: if self.tui.is_some() { Some(&open_tui) } else { None },
                show_chooser: Some(&chooser),
                open_terminal_window: if self.window { Some(&window) } else { None },
                deliver: Some(&deliver),
                autostart: Some(&autostart),
                wait: Some(&wait),
            },
        });
        (String::from_utf8(output).unwrap(), String::from_utf8(errors).unwrap(), code)
    }

    fn ok(&self, arguments: &[&str]) -> String {
        let (output, errors, code) = self.run(arguments);
        assert_eq!(code, EXIT_OK, "{arguments:?}\nstdout:\n{output}\nstderr:\n{errors}");
        output
    }

    fn fails(&self, arguments: &[&str], code: i32, fragment: &str) -> String {
        let (output, errors, actual) = self.run(arguments);
        assert_eq!(actual, code, "{arguments:?}\nstdout:\n{output}\nstderr:\n{errors}");
        assert!(errors.contains(fragment), "{arguments:?} should mention {fragment:?}\nstderr:\n{errors}");
        errors
    }

    fn opened(&self) -> Vec<String> {
        self.opened.replace(Vec::new())
    }
}

#[test]
fn help_version_and_unknown_commands() {
    let harness = Harness::new("cli-help");
    let help = harness.ok(&["--help"]);
    for expected in ["Usage:", "Commands:", "records", "Global flags:", "--db <value>", "Environment:", &meta::env_name("ui")] {
        assert!(help.contains(expected), "{expected}\n{help}");
    }
    assert_eq!(harness.ok(&["help"]), help);
    assert_eq!(harness.ok(&["-h"]), help);
    let version = format!("{} {}\n", meta::SLUG, meta::VERSION);
    assert_eq!(harness.ok(&["--version"]), version);
    assert_eq!(harness.ok(&["version"]), version);
    let records = harness.ok(&["help", "records"]);
    assert!(records.contains(&format!("{} records <command>", meta::SLUG)) && records.contains("import"), "{records}");
    let list = harness.ok(&["records", "list", "--help"]);
    assert!(list.contains("--limit <value>") && list.contains("(default 500)") && list.contains("[flags]"), "{list}");
    assert!(!harness.path(".config").is_empty());
    assert!(!harness.home.path().join(".config").exists(), "help must not create files");
    harness.fails(&["recrods"], EXIT_USAGE, "unknown command \"recrods\"");
    harness.fails(&["rec"], EXIT_USAGE, "Did you mean \"records\"?");
    harness.fails(&["records", "lst"], EXIT_USAGE, "unknown command \"records lst\"");
    harness.fails(&["zzz"], EXIT_USAGE, "Available: overview, records");
    harness.fails(&["--frobnicate"], EXIT_USAGE, "unknown flag --frobnicate");
    harness.fails(&["--set", "nokey", "overview"], EXIT_USAGE, "key=value");
    harness.fails(&["--set", "no.such=1", "overview"], EXIT_USAGE, "unknown setting \"no.such\"");
    harness.fails(&["records", "list", "--frobnicate"], EXIT_USAGE, "help records list");
    harness.fails(&["records", "list", "--limit", "many"], EXIT_USAGE, "whole number");
    let (output, errors, code) = harness.run(&["records"]);
    assert_eq!((code, output.as_str()), (EXIT_USAGE, ""));
    assert!(errors.contains("Commands:"), "{errors}");
}

#[test]
fn records_are_added_listed_changed_and_removed() {
    let harness = Harness::new("cli-records");
    let (output, errors, code) = harness.run(&["records", "add", "--name", "Renew the lease", "--severity", "high"]);
    assert_eq!(code, EXIT_OK, "{errors}");
    assert_eq!(output, "Added record 1: Renew the lease\n");
    assert!(errors.contains("Created a new database at"), "{errors}");
    harness.ok(&["records", "add", "--name=Archive", "--status", "done", "--disabled", "--note", "kept\nfor later"]);
    let listed = harness.ok(&["records", "list"]);
    assert!(listed.starts_with("ID  NAME"), "{listed}");
    assert!(listed.contains("Renew the lease") && !listed.contains("Archive"), "{listed}");
    let all = harness.ok(&["records", "list", "--all", "--sort", "name", "--desc"]);
    assert!(all.find("Renew the lease").unwrap() < all.find("Archive").unwrap(), "{all}");
    let json: serde_json::Value = serde_json::from_str(&harness.ok(&["records", "list", "--all", "--format", "json"])).unwrap();
    assert_eq!(json.as_array().unwrap().len(), 2);
    assert_eq!(json[0]["name"], "Archive");
    assert_eq!(json[0]["enabled"], false);
    assert_eq!(json[1]["created_at"], "2026-03-02T09:30:00Z");
    let csv = harness.ok(&["records", "list", "--format", "csv"]);
    assert!(csv.starts_with("name,status,severity,note,enabled"), "{csv}");
    let shown = harness.ok(&["records", "show", "2"]);
    assert!(shown.contains("kept / for later") && shown.contains("Enabled   no"), "{shown}");
    let shown: serde_json::Value = serde_json::from_str(&harness.ok(&["records", "show", "1", "--format", "json"])).unwrap();
    assert_eq!((shown["severity"].as_str(), shown["revision"].as_i64()), (Some("high"), Some(1)));
    assert_eq!(harness.ok(&["records", "edit", "1", "--status", "in_progress"]), "Updated record 1 (revision 2)\n");
    assert_eq!(harness.ok(&["records", "disable", "1"]), "Record 1 is disabled\n");
    assert_eq!(harness.ok(&["records", "enable", "1"]), "Record 1 is enabled\n");
    assert!(harness.ok(&["records", "list", "--search", "lease", "--status", "in_progress"]).contains("Renew the lease"));
    assert!(harness.ok(&["records", "list", "--search", "nothing"]).contains("No records match"));
    harness.fails(&["records", "add"], EXIT_USAGE, "a name is required");
    harness.fails(&["records", "add", "--name", ""], EXIT_USAGE, "name");
    harness.fails(&["records", "add", "--name", "x", "--severity", "urgent"], EXIT_USAGE, "severity");
    harness.fails(&["records", "add", "--name", "Renew the lease"], EXIT_USAGE, "already exists");
    harness.fails(&["records", "edit", "1"], EXIT_USAGE, "nothing to change");
    harness.fails(&["records", "show"], EXIT_USAGE, "records list");
    harness.fails(&["records", "show", "abc"], EXIT_USAGE, "is not a record identifier");
    harness.fails(&["records", "show", "99"], EXIT_FAILURE, "was not found");
    harness.fails(&["records", "list", "--format", "xml"], EXIT_USAGE, "use table, json or csv");
    harness.fails(&["records", "list", "--sort", "color"], EXIT_USAGE, "sort");
    harness.fails(&["records", "list", "extra"], EXIT_USAGE, "unexpected argument \"extra\"");
    harness.fails(&["records", "remove", "1"], EXIT_USAGE, "--yes");
    assert!(harness.ok(&["records", "remove", "1", "--dry-run"]).contains("Nothing was changed"));
    assert_eq!(harness.ok(&["records", "remove", "--yes", "1"]), "Removed record 1\n");
    harness.fails(&["records", "show", "1"], EXIT_FAILURE, "was not found");
}

#[test]
fn removal_asks_on_an_interactive_terminal_unless_confirmation_is_off() {
    let mut harness = Harness::new("cli-confirm");
    harness.ok(&["records", "add", "--name", "First"]);
    harness.ok(&["records", "add", "--name", "Second"]);
    harness.terminal = true;
    harness.stdin = "no\n".to_string();
    let errors = harness.fails(&["records", "remove", "1"], EXIT_FAILURE, "cancelled, nothing was changed");
    assert!(errors.contains("Type yes to continue"), "{errors}");
    harness.stdin = "YES\n".to_string();
    assert_eq!(harness.ok(&["records", "remove", "1"]), "Removed record 1\n");
    harness.set("CI", "true");
    harness.fails(&["records", "remove", "2"], EXIT_USAGE, "--yes");
    harness.terminal = false;
    assert_eq!(harness.ok(&["--set", "safety.confirm_destructive=false", "records", "remove", "2"]), "Removed record 2\n");
}

#[test]
fn records_are_imported_and_exported() {
    let mut harness = Harness::new("cli-transfer");
    let source = harness.path("in.csv");
    std::fs::write(&source, "name,status,severity,note,enabled\nAlpha,open,high,first,true\nBeta,done,low,,false\n,open,low,,true\n").unwrap();
    let (output, errors, code) = harness.run(&["records", "import", &source, "--dry-run"]);
    assert_eq!(code, EXIT_OK, "{errors}");
    assert!(output.starts_with("Dry run: would have created 2"), "{output}");
    assert!(errors.contains("rejected row 4"), "{errors}");
    assert!(harness.ok(&["records", "list", "--all"]).contains("No records yet"));
    assert!(harness.ok(&["records", "import", &source]).starts_with("Created 2, updated 0, unchanged 0, rejected 1 row"));
    assert!(harness.ok(&["records", "import", &source]).starts_with("Created 0, updated 0, unchanged 2"));
    let exported = harness.ok(&["records", "export"]);
    assert!(exported.contains("Alpha,open,high,first,true") && exported.contains("Beta,done,low,,false"), "{exported}");
    let json = harness.ok(&["records", "export", "--format", "json"]);
    assert_eq!(serde_json::from_str::<serde_json::Value>(&json).unwrap().as_array().unwrap().len(), 2);
    let target = harness.path("out.json");
    assert_eq!(harness.ok(&["records", "export", "--output", &target]), format!("Exported 2 records to {target}\n"));
    assert!(std::fs::read_to_string(&target).unwrap().trim_start().starts_with('['));
    harness.fails(&["records", "export", "--output", &target], EXIT_USAGE, "--force");
    harness.ok(&["records", "export", "--output", &target, "--force", "--format", "csv"]);
    assert!(std::fs::read_to_string(&target).unwrap().starts_with("name,"));
    harness.fails(&["records", "export", "--format", "xml"], EXIT_USAGE, "use csv or json");
    harness.stdin = "[{\"name\":\"Gamma\",\"status\":\"open\",\"severity\":\"info\",\"note\":\"\",\"enabled\":true}]".to_string();
    assert!(harness.ok(&["records", "import", "-", "--format", "json"]).starts_with("Created 1"));
    harness.fails(&["records", "import", "-"], EXIT_USAGE, "--format csv or --format json");
    harness.fails(&["records", "import"], EXIT_USAGE, "give one file to import");
    harness.fails(&["records", "import", &harness.path("missing.csv")], EXIT_FAILURE, "cannot open the import file");
    let broken = harness.path("broken.csv");
    std::fs::write(&broken, "name,status,severity,note,enabled\n,open,low,,true\n").unwrap();
    harness.fails(&["records", "import", &broken], EXIT_FAILURE, "no row could be imported");
}

#[test]
fn demo_data_feeds_the_overview_and_the_notifications() {
    let harness = Harness::new("cli-demo");
    assert!(harness.ok(&["overview"]).contains("No records yet"));
    let added = harness.ok(&["demo", "load"]);
    assert!(added.starts_with("Added ") && added.contains("Demo:"), "{added}");
    assert_eq!(harness.ok(&["demo", "load"]), "Demo data is already loaded.\n");
    let overview = harness.ok(&["overview"]);
    for expected in ["MEASURE", "Open records", "Unread notifications", "SEVERITY", "Critical", "Demo data is loaded"] {
        assert!(overview.contains(expected), "{expected}\n{overview}");
    }
    let json: serde_json::Value = serde_json::from_str(&harness.ok(&["overview", "--format", "json"])).unwrap();
    let demo = json["demo_records"].as_i64().unwrap();
    assert!(demo > 0 && json["breakdown"].as_array().is_some_and(|rows| !rows.is_empty()), "{json}");
    harness.fails(&["overview", "--format", "csv"], EXIT_USAGE, "use table or json");
    let unread = json["unread_notifications"].as_i64().unwrap();
    assert!(unread > 0, "{json}");
    let listed = harness.ok(&["notifications", "list"]);
    assert!(listed.starts_with("ID  WHEN") && listed.contains("unread") && listed.contains("record "), "{listed}");
    let rows: serde_json::Value = serde_json::from_str(&harness.ok(&["notifications", "list", "--unread", "--format", "json"])).unwrap();
    assert_eq!(rows.as_array().unwrap().len() as i64, unread);
    let first = rows[0]["id"].as_i64().unwrap().to_string();
    assert_eq!(harness.ok(&["notifications", "read", &first]), format!("Marked notification {first} as read\n"));
    assert_eq!(harness.ok(&["notifications", "read", "all"]), format!("Marked {} notifications as read\n", unread - 1));
    assert!(harness.ok(&["notifications", "list", "--unread"]).contains("No notifications"));
    assert!(harness.ok(&["notifications", "list", "--severity", "critical", "--event", "record.urgent"]).contains("read"));
    harness.fails(&["notifications", "read"], EXIT_USAGE, "the word all");
    harness.fails(&["notifications", "read", "x"], EXIT_USAGE, "is not a notification identifier");
    harness.fails(&["notifications", "read", "9999"], EXIT_FAILURE, "was not found");
    harness.fails(&["notifications", "list", "--severity", "loud"], EXIT_USAGE, "severity");
    harness.fails(&["notifications", "list", "--event", "nope"], EXIT_USAGE, "unknown event type");
    harness.fails(&["demo", "remove"], EXIT_USAGE, "--yes");
    assert!(harness.ok(&["demo", "remove", "--dry-run"]).contains(&format!("This removes {demo} demo records")));
    assert_eq!(harness.ok(&["demo", "remove", "--yes"]), format!("Removed {demo} demo records\n"));
    assert_eq!(harness.ok(&["demo", "remove", "--yes"]), "There is no demo data to remove.\n");
}

#[test]
fn notification_rules_are_listed_changed_and_reset() {
    let harness = Harness::new("cli-rules");
    let listed = harness.ok(&["notifications", "rules", "list"]);
    assert!(listed.contains("record.urgent") && listed.contains("default") && listed.contains("none"), "{listed}");
    let changed = harness.ok(&[
        "notifications", "rules", "set", "record.urgent", "--enabled", "no", "--min-severity", "HIGH", "--quiet-start", "22:00",
        "--quiet-end", "07:00", "--dedup-minutes", "5",
    ]);
    assert_eq!(
        changed,
        "Rule for record.urgent: enabled no, minimum severity High, quiet hours 22:00 to 07:00, de-duplication 5 minutes\n"
    );
    let listed = harness.ok(&["notifications", "rules", "list"]);
    assert!(listed.contains("22:00 to 07:00") && listed.contains("changed"), "{listed}");
    harness.fails(&["notifications", "rules", "set", "record.urgent"], EXIT_USAGE, "nothing to change");
    harness.fails(&["notifications", "rules", "set", "record.urgent", "--enabled", "maybe"], EXIT_USAGE, "true or false");
    harness.fails(&["notifications", "rules", "set", "record.urgent", "--quiet-start", "25:00"], EXIT_USAGE, "time of day");
    harness.fails(&["notifications", "rules", "set", "record.urgent", "--dedup-minutes", "x"], EXIT_USAGE, "whole number");
    harness.fails(&["notifications", "rules", "set", "record.urgent", "--min-severity", "loud"], EXIT_USAGE, "info, low");
    harness.fails(&["notifications", "rules", "set", "nope", "--enabled", "true"], EXIT_USAGE, "unknown event type");
    harness.fails(&["notifications", "rules", "set"], EXIT_USAGE, "give one event type: record.urgent");
    assert_eq!(harness.ok(&["notifications", "rules", "reset", "record.urgent"]), "Rule for record.urgent is back to its defaults\n");
    assert!(!harness.ok(&["notifications", "rules", "list"]).contains("changed"));
}

#[test]
fn the_notifier_delivers_new_items_and_manages_the_sign_in_entry() {
    let harness = Harness::new("cli-notifier");
    assert_eq!(harness.ok(&["notifier", "run", "--once"]), "Checked once: 0 new notifications shown\n");
    harness.ok(&["records", "add", "--name", "Server down", "--severity", "critical"]);
    assert_eq!(harness.ok(&["notifier", "run", "--once"]), "Checked once: 1 new notifications shown\n");
    assert_eq!(*harness.delivered.borrow(), vec![format!("{} | 1 new notification", meta::APP_NAME)]);
    harness.ok(&["config", "set", "notifier.show_titles", "true"]);
    harness.ok(&["records", "add", "--name", "Disk full", "--severity", "critical"]);
    let (output, errors, code) = harness.run(&["notifier", "run"]);
    assert_eq!((code, output.as_str()), (EXIT_OK, ""), "{errors}");
    assert!(errors.contains("checking every 60 seconds") && errors.contains("makes no network requests"), "{errors}");
    assert_eq!(harness.delivered.borrow().len(), 2);
    assert!(harness.delivered.borrow()[1].ends_with("A record needs attention"));
    let status = harness.ok(&["notifier", "status"]);
    assert!(status.starts_with("Start at sign-in: off") && status.contains("not verified"), "{status}");
    let planned = harness.ok(&["notifier", "install", "--dry-run"]);
    assert!(planned.starts_with("Dry run: would write") && planned.contains("/opt/app/templateapp notifier run"), "{planned}");
    assert!(harness.ok(&["notifier", "uninstall"]).contains("is not set to start at sign-in"));
    assert!(harness.ok(&["notifier", "install"]).starts_with("Wrote "));
    let entry = harness.home.path().join(".config").join("autostart").join(format!("{}-notifier.desktop", meta::SLUG));
    assert!(std::fs::read_to_string(&entry).unwrap().contains("Exec=\"/opt/app/templateapp\" notifier run"));
    assert!(harness.ok(&["notifier", "status"]).starts_with("Start at sign-in: on"));
    assert!(harness.ok(&["notifier", "uninstall", "--dry-run"]).starts_with("Dry run: would remove"));
    assert!(entry.exists());
    assert!(harness.ok(&["notifier", "uninstall"]).starts_with("Removed "));
    assert!(!entry.exists());
}

#[test]
fn settings_themes_and_the_launcher_preference_are_managed() {
    let mut harness = Harness::new("cli-config");
    let listed = harness.ok(&["config", "list"]);
    assert!(listed.starts_with("KEY") && listed.contains("appearance.theme") && listed.contains("default"), "{listed}");
    assert_eq!(harness.ok(&["config", "get", "appearance.density"]), "comfortable\n");
    assert_eq!(harness.ok(&["config", "set", "appearance.density", "compact"]), "appearance.density = compact\n");
    assert_eq!(harness.ok(&["config", "get", "appearance.density"]), "compact\n");
    let (output, errors, code) = harness.run(&["--set", "appearance.density=comfortable", "config", "get", "appearance.density"]);
    assert_eq!((code, output.as_str()), (EXIT_OK, "comfortable\n"));
    assert!(errors.contains("overridden by command-line flag --set appearance.density") && errors.contains("\"compact\""), "{errors}");
    let variable = app_core::settings::env_name("appearance.density");
    harness.set(&variable, "nonsense");
    let (_, errors, code) = harness.run(&["config", "list"]);
    assert_eq!(code, EXIT_OK);
    assert!(errors.contains("note: appearance.density:") && errors.contains("is ignored"), "{errors}");
    harness.set(&variable, "comfortable");
    let (output, errors, _) = harness.run(&["config", "set", "appearance.density", "compact"]);
    assert_eq!(output, "appearance.density = compact\n");
    assert!(errors.contains("saved, but the environment variable"), "{errors}");
    let json: serde_json::Value = serde_json::from_str(&harness.ok(&["config", "list", "--format", "json"])).unwrap();
    let density = json.as_array().unwrap().iter().find(|row| row["key"] == "appearance.density").unwrap();
    assert_eq!((density["value"].as_str(), density["source"].as_str()), (Some("comfortable"), Some("environment")));
    assert!(density["overridden_by"].as_str().unwrap().contains(&variable));
    harness.variables.remove(&variable);
    assert_eq!(harness.ok(&["config", "reset", "appearance.density"]), "appearance.density is back to its default: comfortable\n");
    harness.fails(&["config", "set", "appearance.density", "huge"], EXIT_USAGE, "compact");
    harness.fails(&["config", "get", "no.such"], EXIT_USAGE, "no.such");
    harness.fails(&["config", "set", "appearance.density"], EXIT_USAGE, "give a key and a value");
    harness.fails(&["config", "get"], EXIT_USAGE, "give one setting key");
    let themes = harness.ok(&["config", "theme", "list"]);
    assert!(themes.contains("slate") && themes.contains("in use") && themes.contains("LIGHT ACCENT"), "{themes}");
    assert!(harness.ok(&["config", "theme", "set", "green"]).starts_with("Color theme: green ("));
    harness.fails(&["config", "theme", "set", "neon"], EXIT_USAGE, "slate");
    harness.fails(&["config", "theme", "set"], EXIT_USAGE, "give one theme name: slate");
    let shown = harness.ok(&["config", "theme", "show"]);
    for expected in ["Theme", "green", "system (the terminal cannot ask", "Colors listed    light", "Terminal colors", "contrast"] {
        assert!(shown.contains(expected), "{expected}\n{shown}");
    }
    assert!(harness.ok(&["config", "launcher", "show"]).contains("not set, the application asks at start"));
    assert_eq!(harness.ok(&["config", "launcher", "set", "tui"]), "Open with: tui (terminal UI)\n");
    assert_eq!(harness.ok(&["config", "launcher", "show"]), "Open with: tui (terminal UI)\n");
    harness.fails(&["config", "launcher", "set", "web"], EXIT_USAGE, "give gui or tui");
    harness.set(&meta::env_name("ui"), "gui");
    let (_, errors, _) = harness.run(&["config", "launcher", "show"]);
    assert!(errors.contains(&format!("overridden by environment variable {}=gui", meta::env_name("ui"))), "{errors}");
    assert!(harness.ok(&["config", "launcher", "reset"]).contains("not set"));
}

#[test]
fn the_database_is_created_located_moved_and_checked() {
    let mut harness = Harness::new("cli-database");
    let (output, errors, code) = harness.run(&["db", "path"]);
    assert_eq!(code, EXIT_OK);
    assert!(output.trim_end().ends_with(&format!("{}.db", meta::SLUG)), "{output}");
    assert!(errors.contains("set by: default location, not created yet"), "{errors}");
    assert!(!PathBuf::from(output.trim_end()).exists());
    assert!(harness.ok(&["db", "init"]).starts_with("Database ready at"));
    assert!(harness.ok(&["db", "init"]).starts_with("The database already exists at"));
    assert_eq!(harness.ok(&["db", "check"]), "Database is healthy: integrity check ok, foreign keys ok, schema version 1\n");
    harness.ok(&["records", "add", "--name", "Travels"]);
    let target = harness.path("moved/copy.db");
    harness.fails(&["db", "move", &target], EXIT_USAGE, "--yes");
    let planned = harness.ok(&["db", "move", &target, "--dry-run"]);
    assert!(planned.starts_with("Dry run: Copy the current database to") && planned.ends_with("Nothing was changed.\n"), "{planned}");
    assert!(harness.ok(&["db", "move", &target, "--yes"]).starts_with("Now using "));
    let (output, errors, _) = harness.run(&["db", "path"]);
    assert!(output.contains("copy.db") && errors.contains("set by: saved location, exists"), "{output}{errors}");
    assert!(harness.ok(&["records", "list"]).contains("Travels"));
    harness.fails(&["db", "move", &target, "--yes"], EXIT_USAGE, "already at this location");
    harness.fails(&["db", "move"], EXIT_USAGE, "give the new database path");
    let other = harness.path("other.db");
    assert!(harness.ok(&["db", "move", &other, "--switch", "--yes"]).contains("other.db"));
    assert!(harness.ok(&["records", "list"]).contains("No records yet"));
    let (output, errors, _) = harness.run(&["--db", &target, "db", "path"]);
    assert!(output.contains("copy.db") && errors.contains("command-line flag --db"), "{output}{errors}");
    assert!(harness.ok(&["--db", &target, "records", "list"]).contains("Travels"));
    harness.fails(&["--db", &target, "db", "move", &other, "--switch", "--yes"], EXIT_USAGE, "remove the override first");
    harness.set(&meta::env_name("db"), &target);
    let (_, errors, _) = harness.run(&["db", "path"]);
    assert!(errors.contains(&format!("environment variable {}", meta::env_name("db"))), "{errors}");
    std::fs::write(harness.path("notes.txt"), "this is not a database, it is a text file with words").unwrap();
    let wrong = harness.path("notes.txt");
    let (_, errors, code) = harness.run(&["--db", &wrong, "db", "check"]);
    assert_eq!(code, EXIT_FAILURE, "{errors}");
    assert_eq!(std::fs::read_to_string(&wrong).unwrap(), "this is not a database, it is a text file with words");
}

#[test]
fn diagnostics_the_guide_about_and_licenses_are_available() {
    let harness = Harness::new("cli-about");
    let diagnostics = harness.ok(&["diagnostics"]);
    for expected in ["ITEM", "Version", "Schema version", "Integrity check", "Dashboard in this build", "Problems"] {
        assert!(diagnostics.contains(expected), "{expected}\n{diagnostics}");
    }
    let contents = harness.ok(&["guide"]);
    assert!(contents.starts_with(&format!("{} usage guide\n\nContents:\n", meta::APP_NAME)), "{contents}");
    harness.fails(&["guide", "zzzunlikelywordzzz"], EXIT_FAILURE, "no section of the usage guide mentions");
    let about = harness.ok(&["about"]);
    for expected in ["Application", meta::REPOSITORY_URL, "Report a vulnerability", "Nothing is opened or sent"] {
        assert!(about.contains(expected), "{expected}\n{about}");
    }
    let issue = harness.ok(&["about", "--issue"]);
    assert!(issue.contains("### Environment") && issue.contains("/issues/new?"), "{issue}");
    assert!(!issue.contains(&harness.path("")), "the report must not contain local paths\n{issue}");
    let licenses = harness.ok(&["licenses"]);
    assert!(licenses.contains("MIT License") && licenses.contains("Permission is hereby granted"), "{licenses}");
    harness.fails(&["licenses", "extra"], EXIT_USAGE, "unexpected argument");
}

#[test]
fn starting_without_a_command_follows_the_launch_decision() {
    let mut harness = Harness::new("cli-launch");
    let (output, errors, code) = harness.run(&[]);
    assert_eq!((code, output.as_str()), (EXIT_OK, ""));
    assert!(errors.contains("nothing was opened") && errors.contains("Usage:"), "{errors}");
    harness.fails(&["--ui", "web"], EXIT_USAGE, "use --ui gui or --ui tui");
    harness.fails(&["--ui", "gui"], EXIT_USAGE, "this build has no dashboard");
    harness.fails(&["--ui", "tui"], EXIT_USAGE, "not an interactive terminal");
    harness.terminal = true;
    harness.tui = Some(Vec::new());
    let (_, errors, code) = harness.run(&[]);
    assert_eq!(code, EXIT_OK, "{errors}");
    assert!(errors.contains("no display is available"), "{errors}");
    assert_eq!(harness.opened(), vec!["tui"]);
    harness.set("DISPLAY", ":0");
    harness.ok(&[]);
    assert_eq!(harness.opened(), vec!["tui"], "without a dashboard the terminal UI opens directly");
    harness.gui = Some(vec!["tui"]);
    harness.tui = Some(vec!["gui", ""]);
    harness.stdin = "r\n1\n".to_string();
    let prompt = harness.ok(&[]);
    assert!(prompt.contains("How do you want to open") && prompt.contains("remember: on"), "{prompt}");
    assert_eq!(harness.opened(), vec!["gui", "tui", "gui"]);
    assert_eq!(harness.ok(&["config", "launcher", "show"]), "Open with: gui (dashboard window)\n");
    harness.stdin = String::new();
    harness.ok(&[]);
    assert_eq!(harness.opened(), vec!["gui", "tui", "gui"], "the remembered choice opens without asking");
    harness.stdin = "q\n".to_string();
    harness.ok(&["--ask"]);
    assert!(harness.opened().is_empty());
    harness.ok(&["--ui", "tui"]);
    assert_eq!(harness.opened()[0], "tui");
    harness.set(&meta::env_name("ui"), "tui");
    harness.ok(&[]);
    assert_eq!(harness.opened()[0], "tui");
    harness.variables.remove(&meta::env_name("ui"));
    harness.terminal = false;
    harness.set("TERM", "dumb");
    harness.ok(&["config", "launcher", "reset"]);
    harness.chooser = Some(PromptResult { mode: "tui".to_string(), remember: true, quit: false });
    harness.fails(&[], EXIT_FAILURE, "no terminal window can be opened from here");
    assert_eq!(harness.opened(), vec!["chooser default gui"]);
    assert_eq!(harness.ok(&["config", "launcher", "show"]), "Open with: tui (terminal UI)\n");
    harness.window = true;
    harness.ok(&["--db", &harness.path("w.db"), "--set", "appearance.density=compact"]);
    let opened = harness.opened();
    assert_eq!(opened.len(), 1, "{opened:?}");
    assert!(opened[0].starts_with("window ") && opened[0].contains(" --ui tui --db "), "{opened:?}");
    assert!(opened[0].ends_with("--set appearance.density=compact"), "{opened:?}");
    harness.chooser = Some(PromptResult { quit: true, ..PromptResult::default() });
    harness.ok(&["--ask"]);
    assert_eq!(harness.opened(), vec!["chooser default tui"]);
    harness.set("CI", "1");
    let (_, errors, code) = harness.run(&["--ask"]);
    assert_eq!(code, EXIT_OK);
    assert!(errors.contains("automation environment"), "{errors}");
}

#[test]
fn every_capability_names_a_real_command_and_every_command_has_help() {
    let harness = Harness::new("cli-capabilities");
    let paths = cli::command_paths();
    for capability in service::CAPABILITIES {
        let known = paths.iter().any(|path| path == capability.cli) || capability.cli == "--ui";
        assert!(known, "{} names the unknown command {:?}", capability.id, capability.cli);
    }
    for path in &paths {
        let mut arguments: Vec<&str> = vec!["help"];
        arguments.extend(path.split(' '));
        let help = harness.ok(&arguments);
        assert!(help.contains(&format!("{} {path}", meta::SLUG)) && help.contains("Global flags:"), "{path}\n{help}");
        assert!(paths.iter().any(|other| other == path));
    }
    let covered: Vec<&str> = service::CAPABILITIES.iter().map(|capability| capability.cli).collect();
    for path in &paths {
        let utility = ["version", "db path", "config get"].contains(&path.as_str());
        assert!(utility || covered.contains(&path.as_str()), "the command {path:?} has no capability row");
    }
    let reference = cli::reference();
    assert_eq!(reference.lines().count(), paths.len() + 2);
    assert!(reference.contains(&format!("| `{} records import` | `<file\\|->` `--dry-run` `--format` |", meta::SLUG)), "{reference}");
    assert!(!harness.home.path().join(".config").exists());
}

#[test]
fn flags_and_positional_arguments_can_be_mixed() {
    let flags = [switch("all", ""), text("format", "table", ""), number("limit", "10", "")];
    assert_eq!(flags[2].kind, Kind::Number);
    let words = |list: &[&str]| list.iter().map(|word| (*word).to_string()).collect::<Vec<String>>();
    let (values, given, positional) = parse_flags(&flags, &words(&["7", "--all", "-format=json", "x", "--", "--limit", "3"])).unwrap();
    assert_eq!(positional, words(&["7", "x", "--limit", "3"]));
    assert_eq!((values["all"].as_str(), values["format"].as_str(), values["limit"].as_str()), ("true", "json", "10"));
    assert!(given.contains("all") && given.contains("format") && !given.contains("limit"));
    let (values, _, positional) = parse_flags(&flags, &words(&["--limit", "3", "--all=false", "-"])).unwrap();
    assert_eq!((values["limit"].as_str(), values["all"].as_str(), positional), ("3", "false", words(&["-"])));
    assert_eq!(parse_flags(&flags, &words(&["--format"])).unwrap_err(), "flag --format needs a value");
    assert_eq!(parse_flags(&flags, &words(&["--all=perhaps"])).unwrap_err(), "flag --all needs true or false, found \"perhaps\"");
    assert_eq!(parse_flags(&flags, &words(&["--nope"])).unwrap_err(), "unknown flag --nope");
    assert_eq!(tabulate("a\tbbb\tc\nlong\tb\tc\nplain\n\tx\ty"), "a     bbb  c\nlong  b    c\nplain\n  x  y");
}
