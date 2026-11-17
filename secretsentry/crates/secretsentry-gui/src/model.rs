use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use app_core::error::{AppError, Result};
use app_core::guide::Guide;
use app_core::meta;
use app_core::notifier::Autostart;
use app_core::notify::{self, Filter, Notification, Rule};
use app_core::platform::Env;
use app_core::records::{self, Input, Overview, Query, Record, Status};
use app_core::service::{
    self, App, Diagnostics, Link, MoveMode, NAV_ABOUT, NAV_NOTIFICATIONS, NAV_RECORDS, NAV_SETTINGS, NavItem,
};
use app_core::settings::{self, Value, split_columns, split_sort};
use app_core::theme::{self, Severity, Tokens};
use app_core::userconfig::{FRONT_END_GUI, FRONT_END_TUI};

use crate::forms::{Action, Confirm, Field, Form, Submit};
use crate::icons::Icons;

pub const LIST_LIMIT: i64 = 500;
pub const SEARCH_LIMIT: usize = 200;
pub const PATH_LIMIT: usize = 1024;
pub const DEMO_PREFIX: &str = "Demo:";
const BUSY: &str = "The database is busy in another window. Nothing was lost; the view refreshes when it is free.";
const FORMATS: &[(&str, &str)] = &[("", "From the file name"), ("csv", "CSV"), ("json", "JSON")];
const CONFLICT: &str = "another window changed this record while you were editing. Your text is still here: copy what you need, choose Cancel, open Edit again to load the new version and repeat your change";
pub const NOTIFIER_OFF: &str = "Off. Turn it on to see system notifications while the application is closed. It adds a per-user entry, needs no administrator rights and makes no network requests.";
pub const NOTIFIER_MISSING: &str = "This build cannot manage the sign-in entry.";

pub type UrlOpener = dyn Fn(&str) -> Result<()>;
pub type TerminalOpener = dyn Fn(&[String]) -> Result<()>;

pub struct Options {
    pub env: Env,
    pub system_dark: bool,
    pub autostart: Option<Autostart>,
    pub in_terminal: bool,
    pub open_terminal: Option<Box<TerminalOpener>>,
    pub open_url: Option<Box<UrlOpener>>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tone {
    Info,
    Success,
    Warning,
    Failure,
}

impl Tone {
    pub fn severity(self) -> Severity {
        match self {
            Tone::Info | Tone::Success => Severity::Info,
            Tone::Warning => Severity::High,
            Tone::Failure => Severity::Critical,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Shortcut {
    Page(usize),
    Search,
    Settings,
    Add,
    Refresh,
    Sidebar,
    Quit,
    Guide,
    Remove,
    Escape,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AboutTab {
    Overview,
    Guide,
    Shortcuts,
    Diagnostics,
    Licenses,
}

pub const ABOUT_TABS: &[(AboutTab, &str)] = &[
    (AboutTab::Overview, "Overview"),
    (AboutTab::Guide, "Usage guide"),
    (AboutTab::Shortcuts, "Shortcuts"),
    (AboutTab::Diagnostics, "Diagnostics"),
    (AboutTab::Licenses, "Licenses and notices"),
];

pub struct RecordsPage {
    pub query: Query,
    pub list: Vec<Record>,
    pub selected: Option<i64>,
    pub columns: Vec<String>,
}

#[derive(Default)]
pub struct InboxPage {
    pub rules: bool,
    pub unread_only: bool,
    pub list: Vec<Notification>,
    pub selected: Option<i64>,
    pub rule_list: Vec<Rule>,
    pub rule: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NotifierState {
    Missing,
    Unknown(String),
    On(String),
    Off,
}

pub struct SettingsPage {
    pub list: Vec<Value>,
    pub groups: Vec<String>,
    pub drafts: BTreeMap<String, String>,
    pub dirty: BTreeSet<String>,
    pub problems: BTreeMap<String, String>,
    pub notifier: NotifierState,
}

pub struct AboutPage {
    pub tab: AboutTab,
    pub usage: Guide,
    pub search: String,
    pub hits: Vec<usize>,
    pub section: Option<usize>,
    pub diagnostics: Option<Diagnostics>,
}

pub struct Dashboard<'a> {
    pub(crate) app: &'a mut App,
    pub(crate) options: Options,
    pub tokens: Tokens,
    pub(crate) values: BTreeMap<String, Value>,
    pub(crate) nav: &'static [NavItem],
    pub(crate) page: usize,
    pub(crate) collapsed: bool,
    pub(crate) notice: Option<(Tone, String)>,
    pub(crate) form: Option<Form>,
    pub(crate) confirm: Option<Confirm>,
    pub(crate) search: String,
    pub(crate) focus_search: bool,
    pub(crate) overview: Overview,
    pub(crate) rec: RecordsPage,
    pub(crate) inbox: InboxPage,
    pub(crate) set: SettingsPage,
    pub(crate) about: AboutPage,
    pub(crate) clipboard: Option<String>,
    pub(crate) icons: Icons,
    pub(crate) restyled: bool,
    pub(crate) flash: u32,
    pub(crate) flash_seen: u32,
    pub(crate) flash_time: f64,
    next: String,
    closing: bool,
}

pub fn severity_options() -> Vec<(String, String)> {
    Severity::ALL.iter().map(|level| (level.as_str().to_string(), level.label().to_string())).collect()
}

pub fn status_options() -> Vec<(String, String)> {
    Status::ALL.iter().map(|status| (status.as_str().to_string(), status.label().to_string())).collect()
}

fn borrowed(options: &[(String, String)]) -> Vec<(&str, &str)> {
    options.iter().map(|(value, label)| (value.as_str(), label.as_str())).collect()
}

pub fn event_label(id: &str) -> &str {
    notify::event_type(id).map(|event| event.label).unwrap_or(id)
}

pub fn enabled_label(enabled: bool) -> &'static str {
    if enabled { "Enabled" } else { "Disabled" }
}

pub fn front_end_label(mode: &str) -> &'static str {
    match mode {
        FRONT_END_GUI => "Dashboard window",
        FRONT_END_TUI => "Terminal UI",
        _ => "Ask every time",
    }
}

pub fn window_size(values: &BTreeMap<String, Value>) -> (f32, f32) {
    let side = |key: &str, fallback: f32| {
        values.get(key).map(Value::int).filter(|size| *size > 0).map(|size| size as f32).unwrap_or(fallback)
    };
    (side(settings::KEY_WINDOW_WIDTH, 1180.0), side(settings::KEY_WINDOW_HEIGHT, 760.0))
}

pub fn saved_value(value: &Value) -> &str {
    value.stored.as_deref().unwrap_or(value.definition.default)
}

fn record_form(title: String, accept: &str, input: &Input, submit: Submit) -> Form {
    let severities = severity_options();
    let statuses = status_options();
    Form::new(
        title,
        accept,
        vec![
            Field::text(
                "name",
                "Name",
                &input.name,
                &format!("Required, unique, at most {} characters.", records::MAX_NAME_LENGTH),
                records::MAX_NAME_LENGTH,
            ),
            Field::choice("status", "Status", input.status.as_str(), "", &borrowed(&statuses)),
            Field::choice(
                "severity",
                "Severity",
                input.severity.as_str(),
                "High and critical open records raise a notification.",
                &borrowed(&severities),
            ),
            Field::area("note", "Note", &input.note, "Optional free text, searchable.", records::MAX_NOTE_LENGTH),
            Field::yes_no("enabled", "Enabled", input.enabled, "Disabled records are kept but not counted."),
        ],
        submit,
    )
}

fn form_input(form: &Form) -> Input {
    Input {
        name: form.value("name"),
        status: Status::parse(&form.value("status")).unwrap_or(Status::Open),
        severity: Severity::parse(&form.value("severity")).unwrap_or(Severity::Info),
        note: form.value("note"),
        enabled: form.value("enabled") == "true",
    }
}

impl<'a> Dashboard<'a> {
    pub fn new(app: &'a mut App, options: Options) -> Dashboard<'a> {
        let welcome = app.created_database();
        let tokens = theme::resolve(&theme::Options::default());
        let usage = service::usage_guide();
        let hits = (0..usage.sections.len()).collect();
        let mut board = Dashboard {
            app,
            options,
            tokens,
            values: BTreeMap::new(),
            nav: service::NAVIGATION,
            page: 0,
            collapsed: false,
            notice: None,
            form: None,
            confirm: None,
            search: String::new(),
            focus_search: false,
            overview: Overview::default(),
            rec: RecordsPage {
                query: Query { sort: "updated".to_string(), descending: true, ..Query::default() },
                list: Vec::new(),
                selected: None,
                columns: Vec::new(),
            },
            inbox: InboxPage::default(),
            set: SettingsPage {
                list: Vec::new(),
                groups: Vec::new(),
                drafts: BTreeMap::new(),
                dirty: BTreeSet::new(),
                problems: BTreeMap::new(),
                notifier: NotifierState::Missing,
            },
            about: AboutPage {
                tab: AboutTab::Overview,
                usage,
                search: String::new(),
                hits,
                section: None,
                diagnostics: None,
            },
            clipboard: None,
            icons: Icons::default(),
            restyled: true,
            flash: 0,
            flash_seen: 0,
            flash_time: -10.0,
            next: String::new(),
            closing: false,
        };
        board.reload();
        board.collapsed = board.values.get(settings::KEY_SIDEBAR_COLLAPSED).is_some_and(Value::bool);
        if welcome {
            board.app.dismiss_welcome();
            let path = board.app.location().path.display().to_string();
            board.confirm = Some(Confirm::new(
                &format!("Welcome to {}", meta::APP_NAME),
                format!(
                    "A new database was created at {path}.\n\nEverything stays in this one file on your computer. You can move it later in Settings, Data, Database location.\n\nLoad demo data to look around with labeled sample records, or cancel to start with an empty database."
                ),
                "Load demo data",
                Some(Action::LoadDemo),
            ));
        }
        board
    }

    pub fn next(&self) -> &str {
        &self.next
    }

    pub fn finished(&self) -> bool {
        self.closing
    }

    pub fn notice(&self) -> Option<(Tone, &str)> {
        self.notice.as_ref().map(|(tone, text)| (*tone, text.as_str()))
    }

    pub fn window_size(&self) -> (f32, f32) {
        window_size(&self.values)
    }

    pub fn follow_system(&mut self, dark: bool) {
        if dark != self.options.system_dark {
            self.options.system_dark = dark;
            self.reload();
        }
    }

    pub fn interval(&self) -> f64 {
        let seconds = self.values.get(settings::KEY_REFRESH_SECONDS).map(Value::int).unwrap_or(0);
        if seconds < 1 { 2.0 } else { seconds as f64 }
    }

    pub fn close(&mut self, next: &str) {
        self.next = next.to_string();
        self.closing = true;
    }

    pub fn keep_window_size(&mut self, width: f32, height: f32) {
        for (key, size) in [(settings::KEY_WINDOW_WIDTH, width), (settings::KEY_WINDOW_HEIGHT, height)] {
            let wanted = (size.round() as i64).to_string();
            if self.values.get(key).is_some_and(|value| saved_value(value) != wanted) {
                let _ = self.app.settings().set(key, &wanted);
            }
        }
    }

    pub(crate) fn say(&mut self, tone: Tone, text: impl Into<String>) {
        self.notice = Some((tone, text.into()));
        self.flash += 1;
    }

    pub(crate) fn report(&mut self, error: &AppError) {
        if error.is_busy() {
            self.say(Tone::Warning, BUSY);
        } else {
            self.say(Tone::Failure, error.to_string());
        }
    }

    pub(crate) fn fail<T>(&mut self, result: Result<T>) -> Option<T> {
        match result {
            Ok(value) => Some(value),
            Err(error) => {
                self.report(&error);
                None
            }
        }
    }

    pub(crate) fn changed(&mut self, text: impl Into<String>) {
        self.reload();
        self.say(Tone::Success, text);
    }

    pub(crate) fn reload(&mut self) {
        let values = self.app.settings().effective();
        let Some(values) = self.fail(values) else {
            return;
        };
        let tokens = self.app.appearance(self.options.system_dark);
        let Some(tokens) = self.fail(tokens) else {
            return;
        };
        self.restyled = self.restyled || tokens != self.tokens;
        self.tokens = tokens;
        self.values = values;
        let overview = self.app.records().overview();
        let Some(overview) = self.fail(overview) else {
            return;
        };
        self.overview = overview;
        self.load_records();
        self.load_inbox();
        self.load_settings();
        self.about.diagnostics = Some(self.app.diagnostics(true));
        let _ = self.app.poll();
    }

    pub fn tick(&mut self) {
        match self.app.poll() {
            Ok(true) => {
                self.reload();
                if self.form.is_none() && self.confirm.is_none() {
                    self.say(Tone::Info, "Updated by another window");
                }
            }
            Ok(false) => {}
            Err(error) => {
                if !error.is_busy() {
                    self.report(&error);
                }
            }
        }
    }

    pub fn page_id(&self) -> &'static str {
        self.nav[self.page].id
    }

    pub fn show(&mut self, id: &str) {
        if let Some(index) = self.nav.iter().position(|item| item.id == id) {
            self.page = index;
        }
    }

    pub fn show_about(&mut self, tab: AboutTab) {
        self.show(NAV_ABOUT);
        self.about.tab = tab;
    }

    pub fn nav_label(&self, item: &NavItem) -> String {
        if item.id == NAV_NOTIFICATIONS && self.overview.unread_notifications > 0 {
            format!("{} ({})", item.label, self.overview.unread_notifications)
        } else {
            item.label.to_string()
        }
    }

    pub fn bell_label(&self) -> String {
        match self.overview.unread_notifications {
            0 => "Notifications: nothing unread".to_string(),
            1 => "Notifications: 1 unread".to_string(),
            count => format!("Notifications: {count} unread"),
        }
    }

    pub fn toggle_sidebar(&mut self) {
        self.collapsed = !self.collapsed;
        let saved =
            self.app.settings().set(settings::KEY_SIDEBAR_COLLAPSED, if self.collapsed { "true" } else { "false" });
        if self.fail(saved).is_some() {
            let _ = self.app.poll();
        }
    }

    pub fn refresh(&mut self) {
        self.changed("Refreshed");
    }

    fn terminal_arguments(&self) -> Vec<String> {
        let mut arguments = vec!["--ui".to_string(), "tui".to_string()];
        let options = self.app.options();
        if let Some(database) = &options.database_path {
            arguments.push("--db".to_string());
            arguments.push(database.display().to_string());
        }
        for (key, value) in &options.overrides {
            arguments.push("--set".to_string());
            arguments.push(format!("{key}={value}"));
        }
        arguments
    }

    pub fn open_terminal(&mut self) {
        if self.options.in_terminal {
            self.close(FRONT_END_TUI);
            return;
        }
        let arguments = self.terminal_arguments();
        let Some(open) = &self.options.open_terminal else {
            self.say(
                Tone::Warning,
                format!("No terminal window can be opened from here. Start {} --ui tui in a terminal.", meta::SLUG),
            );
            return;
        };
        let opened = open(&arguments);
        if self.fail(opened).is_some() {
            self.close("");
        }
    }

    pub fn shortcut(&mut self, shortcut: Shortcut) {
        if shortcut == Shortcut::Quit {
            self.close("");
            return;
        }
        if self.form.is_some() || self.confirm.is_some() {
            return;
        }
        match shortcut {
            Shortcut::Page(index) if index < self.nav.len() => self.page = index,
            Shortcut::Page(_) | Shortcut::Quit => {}
            Shortcut::Search => self.focus_search = true,
            Shortcut::Settings => self.show(NAV_SETTINGS),
            Shortcut::Add => {
                self.show(NAV_RECORDS);
                self.add_record();
            }
            Shortcut::Refresh => self.refresh(),
            Shortcut::Sidebar => self.toggle_sidebar(),
            Shortcut::Guide => self.show_about(AboutTab::Guide),
            Shortcut::Remove => {
                if self.page_id() == NAV_RECORDS {
                    self.remove_record();
                }
            }
            Shortcut::Escape => {
                if self.page_id() == NAV_RECORDS {
                    self.rec.selected = None;
                }
            }
        }
    }

    pub fn search_everywhere(&mut self) {
        let query = self.search.trim().to_string();
        if query.is_empty() {
            self.say(Tone::Info, "Type one or more words, then press Enter.");
            return;
        }
        let found = self.app.records().list(&Query {
            search: query.clone(),
            include_disabled: true,
            limit: LIST_LIMIT,
            ..Query::default()
        });
        let Some(found) = self.fail(found) else {
            return;
        };
        let sections = self.about.usage.search(&query).len();
        if !found.is_empty() {
            self.show(NAV_RECORDS);
            self.rec.query.include_disabled = true;
            self.rec.query.search = query.clone();
            self.load_records();
            self.say(
                Tone::Info,
                format!(
                    "{} records and {sections} usage guide sections match \"{query}\". The guide is under About, Usage guide.",
                    found.len()
                ),
            );
        } else if sections > 0 {
            self.show_about(AboutTab::Guide);
            self.about.search = query.clone();
            self.filter_guide();
            self.say(Tone::Info, format!("No record matches \"{query}\". {sections} usage guide sections do."));
        } else {
            self.say(
                Tone::Info,
                format!("Nothing matches \"{query}\". Search covers record names, record notes and the usage guide."),
            );
        }
    }

    pub(crate) fn ask(&mut self, confirm: Confirm) {
        let wanted = self.values.get(settings::KEY_CONFIRM).is_none_or(Value::bool);
        match (&confirm.action, wanted) {
            (Some(action), false) => self.run(action.clone()),
            _ => self.confirm = Some(confirm),
        }
    }

    pub fn answer(&mut self, accepted: bool) {
        let Some(confirm) = self.confirm.take() else {
            return;
        };
        if let (true, Some(action)) = (accepted, confirm.action) {
            self.run(action);
        }
    }

    pub(crate) fn run(&mut self, action: Action) {
        if action == Action::LoadDemo {
            return self.load_demo();
        }
        let done = self.perform(action);
        if let Some(done) = self.fail(done) {
            self.changed(done);
        }
    }

    fn perform(&mut self, action: Action) -> Result<String> {
        match action {
            Action::RemoveRecord(id) => {
                self.app.records().delete(id)?;
                if self.rec.selected == Some(id) {
                    self.rec.selected = None;
                }
                Ok(format!("Removed record {id}"))
            }
            Action::RemoveDemo => {
                let removed = self.app.records().remove_demo(false)?;
                Ok(format!("Removed {removed} demo records"))
            }
            Action::LoadDemo => Ok(String::new()),
            Action::InstallNotifier => {
                self.autostart()?.install(false)?;
                Ok("The notifier starts the next time you sign in".to_string())
            }
            Action::UninstallNotifier => {
                self.autostart()?.uninstall(false)?;
                Ok("The notifier no longer starts at sign-in".to_string())
            }
            Action::MoveDatabase(plan) => {
                self.app.move_database(&plan)?;
                Ok(format!("Now using {}", self.app.location().path.display()))
            }
            Action::Import { path, format } => {
                let done = self.app.import_file(Path::new(&path), &format, false)?;
                self.rec.query.include_disabled = true;
                Ok(format!("Import finished: {}", done.summary()))
            }
            Action::OpenLink(link) => self.open_link(&link),
        }
    }

    pub(crate) fn autostart(&self) -> Result<&Autostart> {
        self.options.autostart.as_ref().ok_or_else(|| AppError::Usage(NOTIFIER_MISSING.to_string()))
    }

    pub(crate) fn open_link(&self, link: &Link) -> Result<String> {
        if !link.allowed {
            return Err(AppError::Usage(link.reason.clone()));
        }
        let Some(open) = &self.options.open_url else {
            return Err(AppError::Usage(format!("No browser can be started from here. Copy this link: {}", link.url)));
        };
        open(&link.url).map_err(|error| AppError::Io(format!("{error}: {}", link.url)))?;
        Ok(format!("Opened {} in your browser", link.label))
    }

    pub fn follow(&mut self, link: &Link) {
        let opened = self.open_link(link);
        if let Some(done) = self.fail(opened) {
            self.say(Tone::Success, done);
        }
    }

    pub fn load_demo(&mut self) {
        let added = self.app.records().load_demo();
        let Some(added) = self.fail(added) else {
            return;
        };
        if added == 0 {
            self.changed("The demo data is already loaded");
        } else {
            self.changed(format!("Added {added} demo records. Remove them in Settings, Data, Remove demo data."));
        }
    }

    pub fn submit(&mut self) {
        let Some(mut form) = self.form.take() else {
            return;
        };
        let outcome = match form.submit.clone() {
            Submit::AddRecord => self.submit_add(&form),
            Submit::EditRecord { id, revision } => self.submit_edit(&form, id, revision),
            Submit::Columns => self.submit_columns(&form),
            Submit::Import => self.submit_import(&form),
            Submit::Export => self.submit_export(&form),
            Submit::Rule(event) => self.submit_rule(&form, &event),
            Submit::MoveDatabase => self.submit_move(&form),
        };
        if let Err(error) = outcome {
            form.problem = if error.is_busy() { BUSY.to_string() } else { error.to_string() };
            self.form = Some(form);
        }
    }

    pub fn cancel(&mut self) {
        self.form = None;
    }

    pub(crate) fn load_records(&mut self) {
        if let Some(value) = self.values.get(settings::KEY_RECORD_SORT) {
            let (key, descending) = split_sort(&value.raw);
            if !key.is_empty() {
                self.rec.query.sort = key;
                self.rec.query.descending = descending;
            }
        }
        if let Some(value) = self.values.get(settings::KEY_RECORD_COLUMNS) {
            self.rec.columns = split_columns(&value.raw);
        }
        let query = Query { limit: LIST_LIMIT, ..self.rec.query.clone() };
        let list = self.app.records().list(&query);
        let Some(list) = self.fail(list) else {
            return;
        };
        self.rec.list = list;
    }

    pub fn selected_record(&self) -> Option<&Record> {
        let id = self.rec.selected?;
        self.rec.list.iter().find(|record| record.id == id)
    }

    pub fn filtered(&self) -> bool {
        let query = &self.rec.query;
        !query.search.is_empty() || query.status.is_some() || query.severity.is_some()
    }

    pub fn empty_records_text(&self) -> &'static str {
        if self.filtered() {
            "No records match these filters. Clear the search box and the filters to see every record."
        } else if !self.rec.query.include_disabled && self.overview.disabled > 0 {
            "Every record is disabled. Select Show disabled to see them."
        } else {
            "No records yet. Choose Add record to create the first one, Import to read a CSV or JSON file, or load demo data on the Overview."
        }
    }

    pub fn clear_filters(&mut self) {
        self.rec.query.search.clear();
        self.rec.query.status = None;
        self.rec.query.severity = None;
        self.load_records();
    }

    pub fn add_record(&mut self) {
        self.form = Some(record_form("Add record".to_string(), "Add record", &Input::default(), Submit::AddRecord));
    }

    pub fn edit_record(&mut self) {
        let Some(record) = self.selected_record().cloned() else {
            return;
        };
        let submit = Submit::EditRecord { id: record.id, revision: record.revision };
        self.form = Some(record_form(format!("Edit record {}", record.id), "Save", &record.input(), submit));
    }

    pub fn toggle_record(&mut self) {
        let Some(record) = self.selected_record().cloned() else {
            return;
        };
        let updated = self.app.records().set_enabled(record.id, !record.enabled);
        let Some(updated) = self.fail(updated) else {
            return;
        };
        if !updated.enabled {
            self.rec.query.include_disabled = true;
        }
        let state = if updated.enabled { "enabled" } else { "disabled" };
        self.changed(format!("Record {} is {state}", updated.id));
    }

    pub fn remove_record(&mut self) {
        let Some(record) = self.selected_record().cloned() else {
            return;
        };
        let mut confirm = Confirm::new(
            "Remove record",
            format!(
                "This removes record {} ({}) permanently. Its notifications stay in the list without a link.",
                record.id, record.name
            ),
            "Remove",
            Some(Action::RemoveRecord(record.id)),
        );
        confirm.danger = true;
        self.ask(confirm);
    }

    pub fn copy_record(&mut self) {
        let Some(record) = self.selected_record().cloned() else {
            return;
        };
        self.clipboard = Some(format!(
            "Record {}: {}\nStatus: {}\nSeverity: {}\nState: {}\nUpdated: {}",
            record.id,
            record.name,
            record.status.label(),
            record.severity.label(),
            enabled_label(record.enabled),
            self.app.format_time(record.updated_at)
        ));
        self.say(Tone::Success, format!("Copied record {} to the clipboard", record.id));
    }

    pub fn record_details(&self, record: &Record) -> String {
        format!(
            "Record {}\nStatus: {}\nSeverity: {}\nState: {}\nCreated: {}\nUpdated: {}\nRevision: {}",
            record.id,
            record.status.label(),
            record.severity.label(),
            enabled_label(record.enabled),
            self.app.format_time(record.created_at),
            self.app.format_time(record.updated_at),
            record.revision
        )
    }

    fn submit_add(&mut self, form: &Form) -> Result<()> {
        let created = self.app.records().create(&form_input(form))?;
        if !created.enabled {
            self.rec.query.include_disabled = true;
        }
        self.changed(format!("Added record {}: {}", created.id, created.name));
        self.rec.selected = Some(created.id);
        Ok(())
    }

    fn submit_edit(&mut self, form: &Form, id: i64, revision: i64) -> Result<()> {
        match self.app.records().update(id, revision, &form_input(form)) {
            Ok(updated) => {
                self.changed(format!("Updated record {}", updated.id));
                Ok(())
            }
            Err(AppError::Conflict(_)) => {
                self.reload();
                Err(AppError::Conflict(CONFLICT.to_string()))
            }
            Err(error) => Err(error),
        }
    }

    pub(crate) fn keep(&mut self, key: &str, raw: &str, what: &str) -> Result<()> {
        let saved = self.app.settings().set(key, raw)?;
        self.reload();
        if !saved.overridden_by.is_empty() {
            self.say(Tone::Warning, format!("{what} saved, but the {} still takes precedence", saved.overridden_by));
        }
        Ok(())
    }

    pub fn sort_by(&mut self, key: &str, descending: bool) {
        let raw = format!("{key}:{}", if descending { "desc" } else { "asc" });
        let kept = self.keep(settings::KEY_RECORD_SORT, &raw, "Sort order");
        self.fail(kept);
    }

    pub fn choose_columns(&mut self) {
        let Some(value) = self.values.get(settings::KEY_RECORD_COLUMNS) else {
            return;
        };
        let stored = split_columns(saved_value(value));
        let fields = value
            .definition
            .options
            .iter()
            .filter(|column| !value.definition.pinned.contains(column))
            .map(|column| {
                let current = if stored.iter().any(|shown| shown == column) { "show" } else { "hide" };
                Field::choice(
                    column,
                    records::column_label(column),
                    current,
                    "",
                    &[("show", "Shown"), ("hide", "Hidden")],
                )
            })
            .collect();
        self.form = Some(Form::new("Columns of the record table", "Save", fields, Submit::Columns));
    }

    fn submit_columns(&mut self, form: &Form) -> Result<()> {
        let pinned = settings::lookup(settings::KEY_RECORD_COLUMNS).map(|definition| definition.pinned).unwrap_or(&[]);
        let shown: Vec<&str> = settings::RECORD_COLUMNS
            .iter()
            .copied()
            .filter(|column| pinned.contains(column) || form.value(column) == "show")
            .collect();
        let saved = self.app.settings().set(settings::KEY_RECORD_COLUMNS, &shown.join(","))?;
        self.reload();
        if saved.overridden_by.is_empty() {
            self.say(Tone::Success, "Saved the columns of the record table");
        } else {
            self.say(Tone::Warning, format!("Columns saved, but the {} still takes precedence", saved.overridden_by));
        }
        Ok(())
    }

    pub fn import_records(&mut self) {
        let help = "A CSV or JSON file. Rows are matched by name: new names are added, known names are updated. Nothing is written until you confirm the preview.";
        let fields =
            vec![Field::text("path", "File", "", help, PATH_LIMIT), Field::choice("format", "Format", "", "", FORMATS)];
        self.form = Some(Form::new("Import records", "Preview", fields, Submit::Import));
    }

    fn submit_import(&mut self, form: &Form) -> Result<()> {
        let path = form.value("path").trim().to_string();
        let format = form.value("format");
        if path.is_empty() {
            return Err(AppError::Usage("choose the file to import".to_string()));
        }
        let result = self.app.import_file(Path::new(&path), &format, true)?;
        let mut statement = format!("Dry run of {path}:\n\n{}\n", result.summary());
        for problem in &result.problems {
            statement.push_str(&format!("\nRejected {problem}"));
        }
        let action = if result.created + result.updated == 0 {
            statement.push_str("\n\nThere is nothing to import.");
            None
        } else {
            Some(Action::Import { path, format })
        };
        let mut confirm = Confirm::new("Import preview", statement, "Import these rows", action);
        if confirm.action.is_none() {
            confirm.cancel = "Close".to_string();
        }
        self.confirm = Some(confirm);
        Ok(())
    }

    pub fn export_records(&mut self) {
        let help = "Every record is exported, including disabled ones. The file is readable only by your user account.";
        let fields = vec![
            Field::text("path", "File", "records.csv", help, PATH_LIMIT),
            Field::choice("format", "Format", "", "", FORMATS),
            Field::choice(
                "overwrite",
                "If the file exists",
                "false",
                "",
                &[("false", "Stop and keep it"), ("true", "Overwrite it")],
            ),
        ];
        self.form = Some(Form::new("Export records", "Export", fields, Submit::Export));
    }

    fn submit_export(&mut self, form: &Form) -> Result<()> {
        let path = form.value("path").trim().to_string();
        if path.is_empty() {
            return Err(AppError::Usage("choose the file to write".to_string()));
        }
        match self.app.export_file(Path::new(&path), &form.value("format"), form.value("overwrite") == "true") {
            Ok(count) => {
                self.say(Tone::Success, format!("Exported {count} records to {path}"));
                Ok(())
            }
            Err(AppError::Conflict(_)) => Err(AppError::Conflict(format!(
                "{path} already exists. Choose another name, or set \"If the file exists\" to \"Overwrite it\""
            ))),
            Err(error) => Err(error),
        }
    }

    pub(crate) fn load_inbox(&mut self) {
        let filter = Filter { unread_only: self.inbox.unread_only, limit: LIST_LIMIT, ..Filter::default() };
        let list = self.app.notifications().list(&filter);
        let Some(list) = self.fail(list) else {
            return;
        };
        let rules = self.app.notifications().rules();
        let Some(rules) = self.fail(rules) else {
            return;
        };
        let page = &mut self.inbox;
        page.list = list;
        page.rule_list = rules;
        if page.selected.is_some_and(|id| !page.list.iter().any(|item| item.id == id)) {
            page.selected = None;
        }
    }

    pub fn inbox_summary(&self) -> String {
        let unread = self.inbox.list.iter().filter(|item| item.unread()).count();
        format!("{} shown, {unread} unread", self.inbox.list.len())
    }

    pub fn empty_inbox_text(&self) -> &'static str {
        if self.inbox.unread_only {
            "No unread notifications. Clear Unread only to see all of them."
        } else {
            "No notifications. They appear when a record needs attention or is completed."
        }
    }

    fn selected_notification(&mut self) -> Option<Notification> {
        let found = self.inbox.selected.and_then(|id| self.inbox.list.iter().find(|item| item.id == id).cloned());
        if found.is_none() {
            self.say(Tone::Info, "Select a notification first");
        }
        found
    }

    pub fn mark_read(&mut self) {
        let Some(item) = self.selected_notification() else {
            return;
        };
        let marked = self.app.notifications().mark_read(item.id);
        if self.fail(marked).is_some() {
            self.changed("Marked the notification as read");
        }
    }

    pub fn mark_all_read(&mut self) {
        let changed = self.app.notifications().mark_all_read();
        if let Some(changed) = self.fail(changed) {
            self.changed(format!("Marked {changed} notifications as read"));
        }
    }

    pub fn open_notification_record(&mut self) {
        let Some(item) = self.selected_notification() else {
            return;
        };
        if item.unread() {
            let marked = self.app.notifications().mark_read(item.id);
            if self.fail(marked).is_none() {
                return;
            }
            self.reload();
        }
        if item.record_id == 0 {
            self.say(Tone::Info, "The record of this notification was removed");
            return;
        }
        self.rec.query = Query {
            sort: self.rec.query.sort.clone(),
            descending: self.rec.query.descending,
            include_disabled: true,
            ..Query::default()
        };
        self.load_records();
        self.rec.selected = Some(item.record_id);
        self.show(NAV_RECORDS);
    }

    fn selected_rule(&mut self) -> Option<Rule> {
        let found = self
            .inbox
            .rule
            .as_ref()
            .and_then(|id| self.inbox.rule_list.iter().find(|rule| &rule.event_type == id).cloned());
        if found.is_none() {
            self.say(Tone::Info, "Select an event type first");
        }
        found
    }

    pub fn edit_rule(&mut self) {
        let Some(rule) = self.selected_rule() else {
            return;
        };
        let severities = severity_options();
        let fields = vec![
            Field::yes_no("enabled", "Enabled", rule.enabled, "Off stops notifications of this event type."),
            Field::choice(
                "severity",
                "Minimum severity",
                rule.min_severity.as_str(),
                "Events below this severity raise no notification.",
                &borrowed(&severities),
            ),
            Field::text(
                "start",
                "Quiet hours start",
                &rule.quiet_start,
                "Time of day as HH:MM. Leave both times empty for no quiet hours.",
                5,
            ),
            Field::text(
                "end",
                "Quiet hours end",
                &rule.quiet_end,
                "Time of day as HH:MM. May be earlier than the start to span midnight.",
                5,
            ),
            Field::text(
                "dedup",
                "De-duplication",
                &rule.dedup_minutes.to_string(),
                "Minutes in which the same event for the same record is not raised again. 0 turns it off.",
                5,
            ),
        ];
        let title = format!("Rule for {}", event_label(&rule.event_type));
        self.form = Some(Form::new(title, "Save", fields, Submit::Rule(rule.event_type)));
    }

    fn submit_rule(&mut self, form: &Form, event: &str) -> Result<()> {
        let minutes: i64 = form.value("dedup").trim().parse().map_err(|_| {
            AppError::Usage("de-duplication needs a whole number of minutes, for example 60".to_string())
        })?;
        let changed = Rule {
            event_type: event.to_string(),
            enabled: form.value("enabled") == "true",
            min_severity: Severity::parse(&form.value("severity")).unwrap_or(Severity::Info),
            quiet_start: form.value("start").trim().to_string(),
            quiet_end: form.value("end").trim().to_string(),
            dedup_minutes: minutes,
            customized: true,
        };
        self.app.notifications().set_rule(&changed)?;
        self.changed(format!("Saved the rule for {}", event_label(event)));
        Ok(())
    }

    pub fn reset_rule(&mut self) {
        let Some(rule) = self.selected_rule() else {
            return;
        };
        let reset = self.app.notifications().reset_rule(&rule.event_type);
        if self.fail(reset).is_some() {
            self.changed(format!("The rule for {} is back to its defaults", event_label(&rule.event_type)));
        }
    }

    pub(crate) fn load_settings(&mut self) {
        let list = self.app.settings().list();
        let Some(list) = self.fail(list) else {
            return;
        };
        let mut groups: Vec<String> = settings::groups(&list).into_iter().map(str::to_string).collect();
        for extra in ["Data", "Notifications", "Launcher"] {
            if !groups.iter().any(|group| group == extra) {
                groups.push(extra.to_string());
            }
        }
        for value in &list {
            if !self.set.dirty.contains(&value.key) {
                self.set.drafts.insert(value.key.clone(), saved_value(value).to_string());
            }
        }
        self.set.list = list;
        self.set.groups = groups;
        self.set.notifier = match self.options.autostart.as_ref().map(Autostart::status) {
            None => NotifierState::Missing,
            Some(Err(error)) => NotifierState::Unknown(error.to_string()),
            Some(Ok((true, entry))) => NotifierState::On(entry.display().to_string()),
            Some(Ok((false, _))) => NotifierState::Off,
        };
    }

    pub fn setting_note(&self, value: &Value) -> String {
        let mut note = String::new();
        if !value.overridden_by.is_empty() {
            note = format!("overridden by {}", value.overridden_by);
        }
        if !value.override_error.is_empty() {
            if !note.is_empty() {
                note.push_str(". ");
            }
            note.push_str(&value.override_error);
        }
        note
    }

    pub fn save_setting(&mut self, key: &str, raw: &str) {
        match self.app.settings().set(key, raw) {
            Ok(saved) => {
                self.set.problems.remove(key);
                self.set.dirty.remove(key);
                let label = saved.definition.label;
                self.changed(format!("Saved {label}"));
                if !saved.overridden_by.is_empty() {
                    self.say(
                        Tone::Warning,
                        format!("Saved {label}, but the {} still takes precedence", saved.overridden_by),
                    );
                } else if key == settings::KEY_ACCENT
                    && let Some(warning) = self.tokens.warnings.last().cloned()
                {
                    self.say(Tone::Warning, warning);
                }
            }
            Err(error) => {
                let problem = if error.is_busy() { BUSY.to_string() } else { error.to_string() };
                self.set.problems.insert(key.to_string(), problem);
            }
        }
    }

    pub fn reset_setting(&mut self, key: &str) {
        let reset = self.app.settings().reset(key);
        let Some(value) = self.fail(reset) else {
            return;
        };
        self.set.problems.remove(key);
        self.set.dirty.remove(key);
        self.changed(format!("{} is back to its default", value.definition.label));
    }

    pub fn database_note(&self) -> String {
        let location = self.app.location();
        if location.overridden_by.is_empty() {
            "Change copies the database to a new place and uses it, or switches to another database. The old file is never deleted.".to_string()
        } else {
            format!("Set by the {}. Remove the override to change the location here.", location.overridden_by)
        }
    }

    pub fn database_locked(&self) -> bool {
        !self.app.location().overridden_by.is_empty()
    }

    pub fn database_path(&self) -> String {
        self.app.location().path.display().to_string()
    }

    pub fn change_database(&mut self) {
        let fields = vec![
            Field::text(
                "path",
                "New location",
                "",
                "Full path of the database file. The current file stays where it is.",
                PATH_LIMIT,
            ),
            Field::choice(
                "mode",
                "What to do",
                "copy",
                "",
                &[
                    ("copy", "Copy my data there and use it"),
                    ("switch", "Use the database that is there, or start an empty one"),
                ],
            ),
        ];
        self.form = Some(Form::new("Database location", "Continue", fields, Submit::MoveDatabase));
    }

    fn submit_move(&mut self, form: &Form) -> Result<()> {
        let mode = if form.value("mode") == "switch" { MoveMode::Switch } else { MoveMode::Copy };
        let plan = self.app.plan_move(form.value("path").trim(), mode)?;
        self.confirm = Some(Confirm::new(
            "Database location",
            plan.statement.clone(),
            "Continue",
            Some(Action::MoveDatabase(plan)),
        ));
        Ok(())
    }

    pub fn remove_demo(&mut self) {
        let count = self.app.records().remove_demo(true);
        let Some(count) = self.fail(count) else {
            return;
        };
        if count == 0 {
            self.say(Tone::Info, "There is no demo data to remove");
            return;
        }
        let mut confirm = Confirm::new(
            "Remove demo data",
            format!("This removes {count} demo records. Your own records are not touched."),
            "Remove",
            Some(Action::RemoveDemo),
        );
        confirm.danger = true;
        self.ask(confirm);
    }

    pub fn toggle_notifier(&mut self) {
        let status = self.autostart().and_then(|entry| entry.status());
        let Some((installed, path)) = self.fail(status) else {
            return;
        };
        if installed {
            self.run(Action::UninstallNotifier);
            return;
        }
        self.confirm = Some(Confirm::new(
            "Start at sign-in",
            format!(
                "This writes {} so that \"{} notifier run\" starts when you sign in. It needs no administrator rights and can be turned off here at any time.",
                path.display(),
                meta::SLUG
            ),
            "Turn on",
            Some(Action::InstallNotifier),
        ));
    }

    pub fn launcher_override(&self) -> Option<String> {
        let variable = meta::env_name("ui");
        self.options
            .env
            .get(&variable)
            .filter(|value| !value.is_empty())
            .map(|value| format!("Overridden by the environment variable {variable}={value}"))
    }

    pub fn front_end(&self) -> &str {
        self.app.location().config.front_end.as_str()
    }

    pub fn remember(&mut self, mode: &str) {
        let remembered = self.app.remember_front_end(mode);
        if self.fail(remembered).is_some() {
            self.changed(format!("Open with: {}", front_end_label(mode)));
        }
    }

    pub fn filter_guide(&mut self) {
        let page = &mut self.about;
        let query = page.search.trim().to_string();
        page.hits = if query.is_empty() {
            (0..page.usage.sections.len()).collect()
        } else {
            let mut found: Vec<usize> = Vec::new();
            for hit in page.usage.search(&query) {
                if !found.contains(&hit.section) {
                    found.push(hit.section);
                }
            }
            found
        };
        if page.section.is_some_and(|section| !page.hits.contains(&section)) {
            page.section = None;
        }
    }

    pub fn report_bug(&mut self) {
        let Some(report) = self.about.diagnostics.as_ref().map(Diagnostics::issue_report) else {
            return;
        };
        let link = service::external_link("the report form", &report.url);
        self.confirm = Some(Confirm::new(
            "Report a bug",
            format!(
                "The report form opens in your browser, prefilled with exactly this text and nothing else. You can edit it before you submit it.\n\n{}",
                report.body
            ),
            "Open the report form",
            Some(Action::OpenLink(link)),
        ));
    }

    pub fn diagnostics_text(&self) -> String {
        let Some(report) = &self.about.diagnostics else {
            return String::new();
        };
        report.lines().iter().map(|(label, value)| format!("{label:<30} {value}\n")).collect()
    }

    pub fn copy_diagnostics(&mut self) {
        self.clipboard = Some(self.diagnostics_text());
        self.say(Tone::Success, "Diagnostics copied");
    }

    pub fn check_database(&mut self) {
        match self.app.store().integrity_check() {
            Ok(()) => {
                self.about.diagnostics = Some(self.app.diagnostics(true));
                self.say(Tone::Success, "The database is healthy");
            }
            Err(error) => self.say(Tone::Failure, format!("The integrity check failed: {error}")),
        }
    }
}
