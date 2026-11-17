use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use crate::allowlists::Allowlists;
use crate::clock::{Clock, Timestamp, Zone};
use crate::error::{AppError, Result};
use crate::files;
use crate::findings::Findings;
use crate::guide::{self, Guide};
use crate::meta::{self, BuildInfo};
use crate::notify::Notifications;
use crate::platform::{self, Env, Os};
use crate::records::{Query, Records};
use crate::rulebook::Rulebook;
use crate::scans::Scans;
use crate::settings::{self, Settings};
use crate::store::{Options as StoreOptions, Store};
use crate::theme::{self, Tokens};
use crate::transfer::{self, Format, ImportResult};
use crate::userconfig::{self, Config};

const SQL_COPY_INTO: &str = "VACUUM INTO ?1";
const EXPORT_LIMIT: i64 = 10000;

#[derive(Clone)]
pub struct Options {
    pub os: Os,
    pub env: Env,
    pub config_dir: Option<PathBuf>,
    pub database_path: Option<PathBuf>,
    pub overrides: BTreeMap<String, String>,
    pub clock: Clock,
    pub zone: Zone,
    pub read_only: bool,
}

impl Options {
    pub fn system() -> Options {
        Options {
            os: Os::current(),
            env: Env::system(),
            config_dir: None,
            database_path: None,
            overrides: BTreeMap::new(),
            clock: Clock::system(),
            zone: Zone::Local,
            read_only: false,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Location {
    pub path: PathBuf,
    pub source: &'static str,
    pub overridden_by: String,
    pub exists: bool,
    pub config_dir: PathBuf,
    pub config: Config,
}

fn absolute(path: &Path) -> Result<PathBuf> {
    std::path::absolute(path)
        .map_err(|error| AppError::Usage(format!("the path \"{}\" cannot be resolved: {error}", path.display())))
}

fn has_content(path: &Path) -> bool {
    std::fs::metadata(path).map(|info| info.is_file() && info.len() > 0).unwrap_or(false)
}

pub fn locate(options: &Options) -> Result<Location> {
    let config_dir = match &options.config_dir {
        Some(directory) => directory.clone(),
        None => platform::config_dir(options.os, &options.env)?,
    };
    let config = userconfig::load(&config_dir)?;
    let variable = meta::env_name("db");
    let (path, source, overridden_by) = if let Some(path) = &options.database_path {
        (path.clone(), "flag", "command-line flag --db".to_string())
    } else if let Some(path) = options.env.get(&variable) {
        (PathBuf::from(path), "environment", format!("environment variable {variable}"))
    } else if !config.database_path.is_empty() {
        (PathBuf::from(&config.database_path), "saved", String::new())
    } else {
        (platform::default_database_path(options.os, &options.env)?, "default", String::new())
    };
    let path = absolute(&path)?;
    let exists = has_content(&path);
    Ok(Location { path, source, overridden_by, exists, config_dir, config })
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MoveMode {
    Copy,
    Switch,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MovePlan {
    pub from: PathBuf,
    pub to: PathBuf,
    pub mode: MoveMode,
    pub target_exists: bool,
    pub statement: String,
}

pub struct App {
    store: Store,
    location: Location,
    options: Options,
    created: bool,
    seen: i64,
}

fn open_store(options: &Options, location: &Location) -> Result<Store> {
    if options.read_only && !location.exists {
        return Err(AppError::Storage(format!(
            "the database \"{}\" does not exist yet: start the application once to create it",
            location.path.display()
        )));
    }
    let store = Store::open(&location.path, StoreOptions { read_only: options.read_only, busy_timeout: None })?;
    if !options.read_only {
        store.migrate(&crate::store::embedded_migrations()?)?;
    }
    Ok(store)
}

impl App {
    pub fn open(options: Options) -> Result<App> {
        let location = locate(&options)?;
        App::open_at(options, location)
    }

    fn open_at(options: Options, mut location: Location) -> Result<App> {
        let store = open_store(&options, &location)?;
        let created = !location.exists;
        location.exists = true;
        let seen = store.data_version()?;
        Ok(App { store, location, options, created, seen })
    }

    pub fn store(&self) -> &Store {
        &self.store
    }

    pub fn settings(&self) -> Settings<'_> {
        Settings::new(self.store.conn(), self.options.clock.clone(), self.options.env.clone(), &self.options.overrides)
    }

    pub fn records(&self) -> Records<'_> {
        Records::new(self.store.conn(), self.options.clock.clone(), self.options.zone)
    }

    pub fn rulebook(&self) -> Rulebook<'_> {
        Rulebook::new(self.store.conn(), self.options.clock.clone())
    }

    pub fn allowlists(&self) -> Allowlists<'_> {
        Allowlists::new(self.store.conn(), self.options.clock.clone())
    }

    pub fn findings(&self) -> Findings<'_> {
        Findings::new(self.store.conn(), self.options.clock.clone())
    }

    pub fn scans(&self) -> Scans<'_> {
        Scans::new(self.store.conn(), self.options.clock.clone(), self.options.zone)
    }

    pub fn notifications(&self) -> Notifications<'_> {
        Notifications::new(self.store.conn(), self.options.clock.clone(), self.options.zone)
    }

    pub fn location(&self) -> &Location {
        &self.location
    }

    pub fn options(&self) -> &Options {
        &self.options
    }

    pub fn created_database(&self) -> bool {
        self.created
    }

    pub fn dismiss_welcome(&mut self) {
        self.created = false;
    }

    pub fn now(&self) -> Timestamp {
        self.options.clock.now()
    }

    pub fn front_end(&self) -> &str {
        &self.location.config.front_end
    }

    pub fn remember_front_end(&mut self, mode: &str) -> Result<()> {
        if !matches!(mode, "" | userconfig::FRONT_END_GUI | userconfig::FRONT_END_TUI) {
            return Err(AppError::Usage(format!(
                "unknown front end \"{mode}\": use gui, tui or an empty value to ask every time"
            )));
        }
        let config = Config { front_end: mode.to_string(), ..self.location.config.clone() };
        userconfig::save(&self.location.config_dir, &config)?;
        self.location.config = config;
        Ok(())
    }

    pub fn plan_move(&self, target: &str, mode: MoveMode) -> Result<MovePlan> {
        if !self.location.overridden_by.is_empty() {
            return Err(AppError::Usage(format!(
                "the database location is set by the {}, so the saved location would not take effect: remove the override first",
                self.location.overridden_by
            )));
        }
        if target.trim().is_empty() {
            return Err(AppError::Usage("the new database path is empty: enter a file path".to_string()));
        }
        let to = absolute(Path::new(target.trim()))?;
        let from = self.location.path.clone();
        if to == from {
            return Err(AppError::Usage("the database is already at this location".to_string()));
        }
        if to.is_dir() {
            return Err(AppError::Usage(format!(
                "\"{}\" is a directory: add a file name such as {}.db",
                to.display(),
                meta::SLUG
            )));
        }
        let target_exists = to.exists();
        let untouched = format!("The file at {} stays untouched.", from.display());
        let statement = match (mode, target_exists) {
            (MoveMode::Copy, true) => {
                return Err(AppError::Conflict(format!(
                    "\"{}\" already exists and would be overwritten: choose a new file name, or switch to the existing database instead",
                    to.display()
                )));
            }
            (MoveMode::Copy, false) => {
                format!("Copy the current database to {} and use the copy from now on. {untouched}", to.display())
            }
            (MoveMode::Switch, true) => {
                format!("Use the existing database at {} from now on. {untouched}", to.display())
            }
            (MoveMode::Switch, false) => {
                format!("Create a new empty database at {} and use it from now on. {untouched}", to.display())
            }
        };
        Ok(MovePlan { from, to, mode, target_exists, statement })
    }

    pub fn move_database(&mut self, plan: &MovePlan) -> Result<()> {
        if plan.from != self.location.path || plan.to.as_os_str().is_empty() {
            return Err(AppError::Usage(
                "the plan does not match the open database: plan the change again".to_string(),
            ));
        }
        if plan.mode == MoveMode::Copy {
            if let Some(parent) = plan.to.parent() {
                files::create_private_dir(parent)?;
            }
            self.store.conn().execute(SQL_COPY_INTO, [plan.to.to_string_lossy().as_ref()]).map_err(|error| {
                AppError::Storage(format!("cannot copy the database to \"{}\": {error}", plan.to.display()))
            })?;
            files::restrict(&plan.to, false)?;
        }
        let target = Location {
            path: plan.to.clone(),
            source: "saved",
            overridden_by: String::new(),
            exists: has_content(&plan.to),
            config_dir: self.location.config_dir.clone(),
            config: self.location.config.clone(),
        };
        let mut candidate = App::open_at(self.options.clone(), target)?;
        let config = Config { database_path: plan.to.to_string_lossy().into_owned(), ..self.location.config.clone() };
        userconfig::save(&self.location.config_dir, &config)?;
        candidate.location.config = config;
        *self = candidate;
        Ok(())
    }

    pub fn poll(&mut self) -> Result<bool> {
        let version = self.store.data_version()?;
        let changed = version != self.seen;
        self.seen = version;
        Ok(changed)
    }

    pub fn appearance(&self, system_dark: bool) -> Result<Tokens> {
        let values = self.settings().effective()?;
        let text = |key: &str| values.get(key).map(|value| value.raw.clone()).unwrap_or_default();
        Ok(theme::resolve(&theme::Options {
            name: text(settings::KEY_THEME),
            mode: theme::Mode::parse(&text(settings::KEY_MODE)),
            system_dark,
            accent: text(settings::KEY_ACCENT),
            density: text(settings::KEY_DENSITY),
            text_scale: values.get(settings::KEY_TEXT_SCALE).map(settings::Value::int).unwrap_or(100),
            reduced_motion: values.get(settings::KEY_REDUCED_MOTION).is_some_and(settings::Value::bool),
        }))
    }

    pub fn display_zone(&self) -> Zone {
        match self.settings().text(settings::KEY_TIME_ZONE) {
            Ok(value) if value == "utc" => Zone::Utc,
            _ => self.options.zone,
        }
    }

    pub fn format_time(&self, moment: Timestamp) -> String {
        self.display_zone().display(moment)
    }

    pub fn import_file(&self, path: &Path, format: &str, dry_run: bool) -> Result<ImportResult> {
        let format = Format::for_file(&path.to_string_lossy(), format)?;
        let raw = files::read_limited(path, transfer::MAX_IMPORT_BYTES as u64)
            .map_err(|error| AppError::Io(format!("cannot open the import file: {error}")))?;
        transfer::import(&self.records(), &raw, format, dry_run)
    }

    pub fn export_text(&self, format: Format) -> Result<(String, usize)> {
        let list = self.records().list(&Query { include_disabled: true, limit: EXPORT_LIMIT, ..Query::default() })?;
        Ok((transfer::export(format, &list)?, list.len()))
    }

    pub fn export_file(&self, path: &Path, format: &str, overwrite: bool) -> Result<usize> {
        if path.as_os_str().is_empty() {
            return Err(AppError::Usage("give the file to write, for example records.csv".to_string()));
        }
        let format = Format::for_file(&path.to_string_lossy(), format)?;
        let (text, count) = self.export_text(format)?;
        files::write_private(path, text.as_bytes(), overwrite)?;
        Ok(count)
    }

    pub fn diagnostics(&self, dashboard: bool) -> Diagnostics {
        let mut report = Diagnostics {
            build: meta::build(dashboard),
            database_path: self.location.path.display().to_string(),
            database_source: self.location.source.to_string(),
            config_dir: self.location.config_dir.display().to_string(),
            schema_version: 0,
            journal_mode: String::new(),
            foreign_keys: false,
            full_text_search: false,
            integrity: "ok".to_string(),
            records: 0,
            front_end: if self.front_end().is_empty() {
                "ask every time".to_string()
            } else {
                self.front_end().to_string()
            },
            changed: Vec::new(),
            overridden: Vec::new(),
            problems: Vec::new(),
        };
        let mut problems = Vec::new();
        let mut note = |what: &str, error: AppError| problems.push(format!("{what}: {error}"));
        match self.store.schema_version() {
            Ok(version) => report.schema_version = version,
            Err(error) => note("schema version", error),
        }
        match self.store.journal_mode() {
            Ok(mode) => report.journal_mode = mode,
            Err(error) => note("journal mode", error),
        }
        match self.store.foreign_keys_enabled() {
            Ok(enabled) => report.foreign_keys = enabled,
            Err(error) => note("foreign keys", error),
        }
        match self.store.has_full_text_search() {
            Ok(present) => report.full_text_search = present,
            Err(error) => note("full-text search", error),
        }
        if let Err(error) = self.store.integrity_check() {
            report.integrity = "failed".to_string();
            note("integrity check", error);
        }
        if let Err(error) = self.store.foreign_key_check() {
            note("foreign key check", error);
        }
        match self.records().count() {
            Ok(count) => report.records = count,
            Err(error) => note("record count", error),
        }
        match self.settings().list() {
            Err(error) => note("settings", error),
            Ok(values) => {
                for value in values.iter().filter(|value| !value.unknown) {
                    if !value.overridden_by.is_empty() {
                        report.overridden.push(value.key.clone());
                    } else if value.stored.is_some() && value.raw != value.definition.default {
                        report.changed.push(value.key.clone());
                    }
                    if !value.override_error.is_empty() {
                        problems.push(format!("{}: {}", value.key, value.override_error));
                    }
                }
            }
        }
        report.changed.sort();
        report.overridden.sort();
        report.problems = problems;
        report
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Diagnostics {
    pub build: BuildInfo,
    pub database_path: String,
    pub database_source: String,
    pub config_dir: String,
    pub schema_version: i64,
    pub journal_mode: String,
    pub foreign_keys: bool,
    pub full_text_search: bool,
    pub integrity: String,
    pub records: i64,
    pub front_end: String,
    pub changed: Vec<String>,
    pub overridden: Vec<String>,
    pub problems: Vec<String>,
}

fn yes_no(value: bool) -> &'static str {
    if value { "yes" } else { "no" }
}

fn list_or_none(items: &[String]) -> String {
    if items.is_empty() { "none".to_string() } else { items.join(", ") }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IssueReport {
    pub title: String,
    pub body: String,
    pub url: String,
}

impl Diagnostics {
    pub fn lines(&self) -> Vec<(&'static str, String)> {
        vec![
            ("Version", self.build.version.clone()),
            ("Build profile", self.build.profile.clone()),
            ("Platform", format!("{}/{}", self.build.os, self.build.arch)),
            ("Dashboard in this build", yes_no(self.build.dashboard).to_string()),
            ("Database", self.database_path.clone()),
            ("Database location set by", self.database_source.clone()),
            ("Configuration directory", self.config_dir.clone()),
            ("Schema version", self.schema_version.to_string()),
            ("Journal mode", self.journal_mode.clone()),
            ("Foreign keys", yes_no(self.foreign_keys).to_string()),
            ("Full-text search", yes_no(self.full_text_search).to_string()),
            ("Integrity check", self.integrity.clone()),
            ("Records", self.records.to_string()),
            ("Remembered front end", self.front_end.clone()),
            ("Settings changed from default", list_or_none(&self.changed)),
            ("Settings overridden", list_or_none(&self.overridden)),
            ("Problems", list_or_none(&self.problems)),
        ]
    }

    pub fn issue_report(&self) -> IssueReport {
        let body = format!(
            "### What happened\n\n\n### What you expected\n\n\n### Steps to reproduce\n\n\n### Environment\n\n- {} {}\n- Operating system: {}\n- Architecture: {}\n- Build: {}, dashboard {}\n- Schema version: {}, integrity check: {}\n- Settings changed from default: {}\n- Settings overridden: {}\n",
            meta::APP_NAME,
            self.build.version,
            self.build.os,
            self.build.arch,
            self.build.profile,
            yes_no(self.build.dashboard),
            self.schema_version,
            self.integrity,
            list_or_none(&self.changed),
            list_or_none(&self.overridden),
        );
        let title = "Bug: ".to_string();
        let url = meta::new_issue_url(&title, &body);
        IssueReport { title, body, url }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NavItem {
    pub id: &'static str,
    pub label: &'static str,
    pub hint: &'static str,
    pub bottom: bool,
}

pub const NAV_OVERVIEW: &str = "overview";
pub const NAV_RECORDS: &str = "records";
pub const NAV_NOTIFICATIONS: &str = "notifications";
pub const NAV_SETTINGS: &str = "settings";
pub const NAV_ABOUT: &str = "about";

pub const NAVIGATION: &[NavItem] = &[
    NavItem { id: NAV_OVERVIEW, label: "Overview", hint: "What needs attention", bottom: false },
    NavItem { id: NAV_RECORDS, label: "Records", hint: "Everything the application tracks", bottom: false },
    NavItem { id: NAV_NOTIFICATIONS, label: "Notifications", hint: "Events and notification rules", bottom: false },
    NavItem { id: NAV_SETTINGS, label: "Settings", hint: "Appearance, data and notifier", bottom: true },
    NavItem { id: NAV_ABOUT, label: "About", hint: "Guide, shortcuts, diagnostics and licenses", bottom: true },
];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Shortcut {
    pub keys: &'static str,
    pub terminal: &'static str,
    pub action: &'static str,
}

pub const SHORTCUTS: &[Shortcut] = &[
    Shortcut { keys: "Ctrl/Cmd+1 to 5", terminal: "1 to 5", action: "Open a category" },
    Shortcut { keys: "Ctrl/Cmd+F", terminal: "/", action: "Search" },
    Shortcut { keys: "Ctrl/Cmd+,", terminal: ",", action: "Open Settings" },
    Shortcut { keys: "F1", terminal: "F1 or ?", action: "Open the usage guide" },
    Shortcut { keys: "Ctrl/Cmd+N", terminal: "n", action: "Add a record" },
    Shortcut { keys: "Enter", terminal: "Enter", action: "Open the selected row" },
    Shortcut { keys: "Up and Down", terminal: "Up, Down, j, k", action: "Move between rows" },
    Shortcut { keys: "Ctrl/Cmd+R", terminal: "r", action: "Refresh now" },
    Shortcut { keys: "Ctrl/Cmd+B", terminal: "b", action: "Collapse or expand the sidebar" },
    Shortcut { keys: "Delete", terminal: "d", action: "Remove the selected record after confirmation" },
    Shortcut { keys: "Escape", terminal: "Esc", action: "Close a panel or cancel" },
    Shortcut { keys: "Ctrl/Cmd+Q", terminal: "q or Ctrl+C", action: "Quit" },
];

pub fn usage_guide() -> Guide {
    guide::parse(meta::USAGE_TEXT)
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Link {
    pub label: String,
    pub url: String,
    pub allowed: bool,
    pub reason: String,
}

pub fn external_link(label: &str, url: &str) -> Link {
    let allowed = meta::is_repository_url(url);
    let reason = if allowed {
        String::new()
    } else {
        format!("Only links under {} can be opened from the application.", meta::REPOSITORY_URL)
    };
    Link { label: label.to_string(), url: url.to_string(), allowed, reason }
}

pub fn source_link() -> Link {
    external_link("Source Code", &meta::source_url())
}

pub fn advisory_link() -> Link {
    external_link("Report a vulnerability privately", &meta::advisory_url())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Capability {
    pub id: &'static str,
    pub name: &'static str,
    pub dashboard: &'static str,
    pub terminal: &'static str,
    pub cli: &'static str,
}

pub const NOT_APPLICABLE: &str = "n/a";

const fn capability(
    id: &'static str,
    name: &'static str,
    dashboard: &'static str,
    terminal: &'static str,
    cli: &'static str,
) -> Capability {
    Capability { id, name, dashboard, terminal, cli }
}

pub const CAPABILITIES: &[Capability] = &[
    capability(
        "database.create",
        "Create the database on first start",
        "First-start dialog",
        "First-start screen",
        "db init",
    ),
    capability(
        "database.location",
        "Choose the database location",
        "Settings, Data, Database location",
        "Settings, Database location",
        "db move",
    ),
    capability("database.check", "Check database health", "About, Diagnostics", "About, Diagnostics", "db check"),
    capability("overview.show", "See what needs attention", "Overview", "Overview", "overview"),
    capability("scan.run", "Scan a directory, its Git history or both", NOT_APPLICABLE, NOT_APPLICABLE, "scan"),
    capability("scan.list", "See earlier scan runs", NOT_APPLICABLE, NOT_APPLICABLE, "scans list"),
    capability("scan.show", "Show one scan run", NOT_APPLICABLE, NOT_APPLICABLE, "scans show"),
    capability("finding.list", "List, search, filter and sort findings", NOT_APPLICABLE, NOT_APPLICABLE, "findings list"),
    capability("finding.show", "Show one finding with its reasons and places", NOT_APPLICABLE, NOT_APPLICABLE, "findings show"),
    capability("finding.decide", "Record a decision and a note for a finding", NOT_APPLICABLE, NOT_APPLICABLE, "findings decide"),
    capability("finding.export", "Export findings as JSON or SARIF", NOT_APPLICABLE, NOT_APPLICABLE, "export"),
    capability("history.list", "See findings by the commit that added them", NOT_APPLICABLE, NOT_APPLICABLE, "history list"),
    capability("baseline.status", "Count new and baseline findings", NOT_APPLICABLE, NOT_APPLICABLE, "baseline status"),
    capability("baseline.accept", "Accept the current findings as the baseline", NOT_APPLICABLE, NOT_APPLICABLE, "baseline accept"),
    capability("baseline.reset", "Empty the baseline", NOT_APPLICABLE, NOT_APPLICABLE, "baseline reset"),
    capability("rule.list", "List detector rules", NOT_APPLICABLE, NOT_APPLICABLE, "rules list"),
    capability("rule.show", "Show one detector rule", NOT_APPLICABLE, NOT_APPLICABLE, "rules show"),
    capability("rule.add", "Add a custom rule", NOT_APPLICABLE, NOT_APPLICABLE, "rules add"),
    capability("rule.edit", "Edit a custom rule", NOT_APPLICABLE, NOT_APPLICABLE, "rules edit"),
    capability("rule.enable", "Enable a rule", NOT_APPLICABLE, NOT_APPLICABLE, "rules enable"),
    capability("rule.disable", "Disable a rule", NOT_APPLICABLE, NOT_APPLICABLE, "rules disable"),
    capability("rule.restore", "Return a built-in rule to its built-in state", NOT_APPLICABLE, NOT_APPLICABLE, "rules restore"),
    capability("rule.remove", "Remove a custom rule", NOT_APPLICABLE, NOT_APPLICABLE, "rules remove"),
    capability("rule.try", "Try a rule on sample text", NOT_APPLICABLE, NOT_APPLICABLE, "rules try"),
    capability("allowlist.list", "List allowlist entries", NOT_APPLICABLE, NOT_APPLICABLE, "allowlist list"),
    capability("allowlist.add", "Add an allowlist entry", NOT_APPLICABLE, NOT_APPLICABLE, "allowlist add"),
    capability("allowlist.edit", "Edit an allowlist entry", NOT_APPLICABLE, NOT_APPLICABLE, "allowlist edit"),
    capability("allowlist.enable", "Enable an allowlist entry", NOT_APPLICABLE, NOT_APPLICABLE, "allowlist enable"),
    capability("allowlist.disable", "Disable an allowlist entry", NOT_APPLICABLE, NOT_APPLICABLE, "allowlist disable"),
    capability("allowlist.remove", "Remove an allowlist entry", NOT_APPLICABLE, NOT_APPLICABLE, "allowlist remove"),
    capability("record.list", "List, search, filter and sort records", "Records", "Records", "records list"),
    capability(
        "record.columns",
        "Choose the columns of the record table",
        "Records, Columns",
        "Records, c",
        "config set",
    ),
    capability("record.show", "Show one record", "Records, row detail panel", "Records, Enter", "records show"),
    capability("record.add", "Add a record", "Records, Add record", "Records, n", "records add"),
    capability("record.edit", "Edit a record", "Records, row detail panel, Edit", "Records, e", "records edit"),
    capability("record.enable", "Enable a record", "Records, row detail panel, Enable", "Records, x", "records enable"),
    capability(
        "record.disable",
        "Disable a record",
        "Records, row detail panel, Disable",
        "Records, x",
        "records disable",
    ),
    capability("record.remove", "Remove a record", "Records, row detail panel, Remove", "Records, d", "records remove"),
    capability("record.import", "Import records from CSV or JSON", "Records, Import", "Records, i", "records import"),
    capability("record.export", "Export records to CSV or JSON", "Records, Export", "Records, o", "records export"),
    capability("demo.load", "Load labeled demo data", "Overview, Load demo data", "Overview, l", "demo load"),
    capability(
        "demo.remove",
        "Remove demo data",
        "Settings, Data, Remove demo data",
        "Settings, Remove demo data",
        "demo remove",
    ),
    capability(
        "notification.list",
        "List and filter notifications",
        "Notifications",
        "Notifications",
        "notifications list",
    ),
    capability(
        "notification.read",
        "Mark notifications as read",
        "Notifications, Mark as read",
        "Notifications, m and M",
        "notifications read",
    ),
    capability(
        "notification.rule.list",
        "See the rule of every event type",
        "Notifications, Rules",
        "Notifications, Rules",
        "notifications rules list",
    ),
    capability(
        "notification.rule.set",
        "Change a notification rule",
        "Notifications, Rules",
        "Notifications, Rules, e",
        "notifications rules set",
    ),
    capability(
        "notification.rule.reset",
        "Reset a notification rule",
        "Notifications, Rules, Reset",
        "Notifications, Rules, u",
        "notifications rules reset",
    ),
    capability(
        "notifier.install",
        "Start the notifier at sign-in",
        "Settings, Notifications, Start at sign-in",
        "Settings, Notifier at sign-in",
        "notifier install",
    ),
    capability(
        "notifier.uninstall",
        "Stop starting the notifier at sign-in",
        "Settings, Notifications, Start at sign-in",
        "Settings, Notifier at sign-in",
        "notifier uninstall",
    ),
    capability(
        "notifier.status",
        "See whether the notifier starts at sign-in",
        "Settings, Notifications",
        "Settings, Notifier at sign-in",
        "notifier status",
    ),
    capability("notifier.run", "Run the background notifier", NOT_APPLICABLE, NOT_APPLICABLE, "notifier run"),
    capability("setting.list", "See every setting and what overrides it", "Settings", "Settings", "config list"),
    capability("setting.set", "Change a setting", "Settings", "Settings, Enter", "config set"),
    capability("setting.reset", "Reset one setting", "Settings, Reset", "Settings, u", "config reset"),
    capability(
        "theme.list",
        "List color themes",
        "Settings, Appearance, Color theme",
        "Settings, Color theme",
        "config theme list",
    ),
    capability(
        "theme.set",
        "Choose a color theme",
        "Settings, Appearance, Color theme",
        "Settings, Color theme",
        "config theme set",
    ),
    capability(
        "theme.show",
        "See the theme in use",
        "Settings, Appearance",
        "Settings, Color theme",
        "config theme show",
    ),
    capability(
        "launcher.show",
        "See the remembered front end",
        "Settings, Launcher",
        "Settings, Open with",
        "config launcher show",
    ),
    capability(
        "launcher.set",
        "Choose the front end to open by default",
        "Settings, Launcher",
        "Settings, Open with",
        "config launcher set",
    ),
    capability(
        "launcher.reset",
        "Ask again which front end to open",
        "Settings, Launcher",
        "Settings, Open with",
        "config launcher reset",
    ),
    capability(
        "launcher.switch",
        "Reopen in the other front end",
        "Menu, Open terminal UI",
        "Menu, Open dashboard",
        "--ui",
    ),
    capability(
        "guide.read",
        "Read and search the usage guide offline",
        "About, Usage guide",
        "About, Usage guide",
        "guide",
    ),
    capability("about.diagnostics", "Show diagnostics", "About, Diagnostics", "About, Diagnostics", "diagnostics"),
    capability(
        "about.licenses",
        "Show licenses and notices",
        "About, Licenses and notices",
        "About, Licenses",
        "licenses",
    ),
    capability("about.links", "Show the source and bug report links", "About, Overview", "About, Overview", "about"),
];

pub fn capability_table() -> String {
    let mut table = String::from("| Capability | Dashboard | Terminal UI | CLI |\n|---|---|---|---|\n");
    for capability in CAPABILITIES {
        let cli =
            if capability.cli == NOT_APPLICABLE { capability.cli.to_string() } else { format!("`{}`", capability.cli) };
        table.push_str(&format!(
            "| {} | {} | {} | {cli} |\n",
            capability.name, capability.dashboard, capability.terminal
        ));
    }
    table
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::records::Input;
    use crate::testkit::{TempDir, fixed_time};

    fn options(directory: &TempDir, pairs: &[(&str, &str)]) -> Options {
        let home = directory.path().join("home").to_string_lossy().into_owned();
        let mut all: Vec<(&str, &str)> = vec![("HOME", home.as_str())];
        all.extend_from_slice(pairs);
        Options {
            os: Os::Linux,
            env: Env::from_pairs(&all),
            config_dir: Some(directory.path().join("config")),
            database_path: None,
            overrides: BTreeMap::new(),
            clock: Clock::fixed(fixed_time()),
            zone: Zone::Fixed(120),
            read_only: false,
        }
    }

    #[test]
    fn the_database_location_follows_flag_environment_saved_then_default() {
        let directory = TempDir::new("service-locate");
        let base = options(&directory, &[]);
        let default = locate(&base).unwrap();
        assert_eq!(default.source, "default");
        assert!(
            default
                .path
                .ends_with(Path::new(".local").join("share").join(meta::SLUG).join(format!("{}.db", meta::SLUG)))
        );
        assert!(!default.exists && default.overridden_by.is_empty());
        let saved_path = directory.path().join("saved.db");
        userconfig::save(
            base.config_dir.as_ref().unwrap(),
            &Config { database_path: saved_path.to_string_lossy().into_owned(), front_end: String::new() },
        )
        .unwrap();
        assert_eq!((locate(&base).unwrap().source, locate(&base).unwrap().path), ("saved", saved_path));
        let variable = meta::env_name("db");
        let env_path = directory.path().join("env.db");
        let with_env = options(&directory, &[(variable.as_str(), env_path.to_str().unwrap())]);
        let found = locate(&with_env).unwrap();
        assert_eq!((found.source, found.path.clone()), ("environment", env_path));
        assert!(found.overridden_by.contains(&variable));
        let flag_path = directory.path().join("flag.db");
        let found = locate(&Options { database_path: Some(flag_path.clone()), ..with_env }).unwrap();
        assert_eq!((found.source, found.path), ("flag", flag_path));
    }

    #[test]
    fn opening_creates_the_database_once_and_read_only_needs_an_existing_one() {
        let directory = TempDir::new("service-open");
        let base = options(&directory, &[]);
        let read_only = Options { read_only: true, ..base.clone() };
        let error = App::open(read_only.clone()).err().unwrap().to_string();
        assert!(error.contains("start the application once to create it"), "{error}");
        let first = App::open(base.clone()).unwrap();
        assert!(first.created_database());
        first.records().create(&Input { name: "kept".to_string(), ..Input::default() }).unwrap();
        drop(first);
        let second = App::open(base).unwrap();
        assert!(!second.created_database());
        assert_eq!(second.records().count().unwrap(), 1);
        let reader = App::open(read_only).unwrap();
        assert_eq!(reader.records().count().unwrap(), 1);
        assert!(reader.records().create(&Input { name: "refused".to_string(), ..Input::default() }).is_err());
    }

    #[test]
    fn the_front_end_choice_is_remembered_in_the_configuration_file() {
        let directory = TempDir::new("service-frontend");
        let mut app = App::open(options(&directory, &[])).unwrap();
        assert_eq!(app.front_end(), "");
        app.remember_front_end("tui").unwrap();
        assert_eq!(app.front_end(), "tui");
        assert!(app.remember_front_end("web").is_err());
        drop(app);
        let mut reopened = App::open(options(&directory, &[])).unwrap();
        assert_eq!(reopened.front_end(), "tui");
        reopened.remember_front_end("").unwrap();
        assert_eq!(reopened.diagnostics(false).front_end, "ask every time");
    }

    #[test]
    fn moving_the_database_copies_or_switches_and_never_overwrites() {
        let directory = TempDir::new("service-move");
        let mut app = App::open(options(&directory, &[])).unwrap();
        app.records().create(&Input { name: "travels".to_string(), ..Input::default() }).unwrap();
        let original = app.location().path.clone();
        let copy_target = directory.path().join("moved").join("copy.db");
        let plan = app.plan_move(copy_target.to_str().unwrap(), MoveMode::Copy).unwrap();
        assert!(plan.statement.starts_with("Copy the current database to"), "{}", plan.statement);
        assert!(!copy_target.exists());
        app.move_database(&plan).unwrap();
        assert_eq!(app.location().path, copy_target);
        assert_eq!(app.location().source, "saved");
        assert_eq!(app.records().count().unwrap(), 1);
        assert!(original.exists());
        assert!(app.move_database(&plan).unwrap_err().to_string().contains("plan the change again"));
        let refused = app.plan_move(original.to_str().unwrap(), MoveMode::Copy).unwrap_err().to_string();
        assert!(refused.contains("would be overwritten"), "{refused}");
        let back = app.plan_move(original.to_str().unwrap(), MoveMode::Switch).unwrap();
        assert!(back.statement.starts_with("Use the existing database"), "{}", back.statement);
        let fresh_target = directory.path().join("fresh.db");
        let fresh = app.plan_move(fresh_target.to_str().unwrap(), MoveMode::Switch).unwrap();
        assert!(fresh.statement.starts_with("Create a new empty database"), "{}", fresh.statement);
        app.move_database(&fresh).unwrap();
        assert_eq!(app.records().count().unwrap(), 0);
        assert!(app.plan_move("", MoveMode::Copy).is_err());
        assert!(
            app.plan_move(fresh_target.to_str().unwrap(), MoveMode::Switch)
                .unwrap_err()
                .to_string()
                .contains("already at this location")
        );
        assert!(
            app.plan_move(directory.path().to_str().unwrap(), MoveMode::Switch)
                .unwrap_err()
                .to_string()
                .contains("is a directory")
        );
        drop(app);
        let reopened = App::open(options(&directory, &[])).unwrap();
        assert_eq!(reopened.location().path, fresh_target);
        let variable = meta::env_name("db");
        let pinned = App::open(options(&directory, &[(variable.as_str(), original.to_str().unwrap())])).unwrap();
        let error = pinned.plan_move(copy_target.to_str().unwrap(), MoveMode::Switch).unwrap_err().to_string();
        assert!(error.contains("remove the override first"), "{error}");
    }

    #[test]
    fn changes_made_by_another_connection_are_noticed_once() {
        let directory = TempDir::new("service-poll");
        let mut app = App::open(options(&directory, &[])).unwrap();
        let other = App::open(options(&directory, &[])).unwrap();
        app.poll().unwrap();
        assert!(!app.poll().unwrap());
        other.records().create(&Input { name: "from elsewhere".to_string(), ..Input::default() }).unwrap();
        assert!(app.poll().unwrap());
        assert!(!app.poll().unwrap());
        assert_eq!(app.records().count().unwrap(), 1);
    }

    #[test]
    fn appearance_and_time_format_follow_the_settings() {
        let directory = TempDir::new("service-appearance");
        let app = App::open(options(&directory, &[])).unwrap();
        assert_eq!(app.appearance(true).unwrap().name, "slate");
        assert!(app.appearance(true).unwrap().dark);
        app.settings().set(settings::KEY_THEME, "green").unwrap();
        app.settings().set(settings::KEY_MODE, "light").unwrap();
        let tokens = app.appearance(true).unwrap();
        assert_eq!((tokens.name, tokens.dark), ("green", false));
        assert_eq!(app.format_time(fixed_time()), "2026-03-02 11:30");
        app.settings().set(settings::KEY_TIME_ZONE, "utc").unwrap();
        assert_eq!(app.format_time(fixed_time()), "2026-03-02 09:30 UTC");
    }

    #[test]
    fn files_are_exported_privately_and_imported_back() {
        let directory = TempDir::new("service-transfer");
        let app = App::open(options(&directory, &[])).unwrap();
        app.records().create(&Input { name: "exported".to_string(), ..Input::default() }).unwrap();
        let target = directory.path().join("out").join("records.csv");
        assert_eq!(app.export_file(&target, "", false).unwrap(), 1);
        assert!(app.export_file(&target, "", false).unwrap_err().to_string().contains("already exists"));
        assert_eq!(app.export_file(&target, "", true).unwrap(), 1);
        assert!(app.export_file(Path::new(""), "", false).is_err());
        assert!(
            app.export_file(&directory.path().join("records.txt"), "", false)
                .unwrap_err()
                .to_string()
                .contains("--format")
        );
        let json = directory.path().join("records.data");
        assert_eq!(app.export_file(&json, "json", false).unwrap(), 1);
        let result = app.import_file(&json, "json", false).unwrap();
        assert_eq!((result.created, result.skipped), (0, 1));
        assert!(
            app.import_file(&directory.path().join("missing.csv"), "", false)
                .unwrap_err()
                .to_string()
                .contains("cannot open the import file")
        );
    }

    #[test]
    fn diagnostics_report_the_build_the_database_and_changed_settings() {
        let directory = TempDir::new("service-diagnostics");
        let variable = settings::env_name(settings::KEY_DENSITY);
        let app = App::open(options(&directory, &[(variable.as_str(), "compact")])).unwrap();
        app.settings().set(settings::KEY_THEME, "amber").unwrap();
        let report = app.diagnostics(true);
        assert_eq!((report.schema_version, report.integrity.as_str(), report.records), (2, "ok", 0));
        assert!(report.foreign_keys && report.full_text_search && report.build.dashboard);
        assert_eq!(report.changed, vec![settings::KEY_THEME.to_string()]);
        assert_eq!(report.overridden, vec![settings::KEY_DENSITY.to_string()]);
        assert!(report.problems.is_empty(), "{:?}", report.problems);
        let lines = report.lines();
        assert_eq!(lines[0], ("Version", meta::VERSION.to_string()));
        assert!(lines.iter().any(|(label, value)| *label == "Problems" && value == "none"));
        let issue = report.issue_report();
        assert!(meta::is_repository_url(&issue.url));
        assert!(issue.body.contains("Settings changed from default: appearance.theme"));
        assert!(!issue.body.contains(&report.database_path));
    }

    #[test]
    fn navigation_shortcuts_links_and_capabilities_are_complete() {
        assert_eq!(NAVIGATION.len(), 5);
        assert!(SHORTCUTS.iter().all(|shortcut| !shortcut.keys.is_empty() && !shortcut.terminal.is_empty()));
        assert!(source_link().allowed && advisory_link().allowed);
        let outside = external_link("Elsewhere", "https://example.test/");
        assert!(!outside.allowed && outside.reason.contains(meta::REPOSITORY_URL));
        let mut ids: Vec<&str> = CAPABILITIES.iter().map(|capability| capability.id).collect();
        let total = ids.len();
        ids.sort_unstable();
        ids.dedup();
        assert_eq!(ids.len(), total);
        for capability in CAPABILITIES {
            assert!(!capability.cli.is_empty() && capability.cli != NOT_APPLICABLE, "{}", capability.id);
            assert!(!capability.dashboard.is_empty() && !capability.terminal.is_empty(), "{}", capability.id);
        }
        assert_eq!(capability_table().lines().count(), total + 2);
        assert!(!usage_guide().sections.is_empty());
    }
}
