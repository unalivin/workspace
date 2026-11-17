use std::collections::BTreeMap;
use std::time::Duration;

use app_core::error::{AppError, Result};
use app_core::meta;
use app_core::notifier::Autostart;
use app_core::platform::Env;
use app_core::findings::{Overview, Query, Scope};
use app_core::jobs::{Job, Report};
use app_core::scans::Scan;
use app_core::service::{
    self, App, Link, MovePlan, NAV_ABOUT, NAV_ALLOWLISTS, NAV_EXPORTS, NAV_FINDINGS, NAV_HISTORY, NAV_NOTIFICATIONS,
    NAV_RULES, NAV_SCANS, NAV_SETTINGS, NavItem,
};
use app_core::settings::{self, Value};
use app_core::theme::{self, Profile, Severity, Style};
use app_core::userconfig::FRONT_END_GUI;
use app_core::words;

use crate::about::{AboutPage, AboutTab};
use crate::catalog::{AllowPage, RulesPage};
use crate::findings::FindingsPage;
use crate::inbox::InboxPage;
use crate::reports::{ExportsPage, HistoryPage};
use crate::scans::ScansPage;
use crate::settings::SettingsPage;
use crate::text::{Line, Look, pad, width, wrap, wrapped};
use crate::widgets::{Form, FormEvent, Key, row, step, tail};

pub const MIN_WIDTH: usize = 60;
pub const MIN_HEIGHT: usize = 16;
const MAX_NOTICE_LINES: usize = 3;
pub(crate) const LIST_LIMIT: i64 = 500;
pub(crate) const CANCELLED: &str = "Cancelled, nothing was changed";
const SCANNING_INTERVAL: Duration = Duration::from_millis(200);
const BUSY: &str = "The database is busy in another window. Nothing was lost; press r to try again.";

pub type UrlOpener = dyn Fn(&str) -> Result<()>;

pub struct Options {
    pub env: Env,
    pub system_dark: bool,
    pub autostart: Option<Autostart>,
    pub has_dashboard: bool,
    pub open_url: Option<Box<UrlOpener>>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Notice {
    Info,
    Success,
    Warning,
    Failure,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Action {
    RemoveDemo,
    InstallNotifier,
    UninstallNotifier,
    MoveDatabase(MovePlan),
    OpenLink(Link),
    AcceptBaseline,
    ResetBaseline,
    RemoveRule(String),
    RemoveEntry(i64),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Submit {
    Scan,
    Decide { id: i64, revision: i64 },
    AddRule,
    EditRule { id: String, revision: i64 },
    TryRule(String),
    AddEntry,
    EditEntry { id: i64, revision: i64 },
    Export,
    Rule(String),
    Setting(String),
    MoveDatabase,
}

pub(crate) struct Confirmation {
    pub title: String,
    pub statement: String,
    pub accept: String,
    pub action: Action,
}

pub(crate) struct Preview {
    pub title: String,
    pub lines: Vec<String>,
    pub offset: usize,
    pub accept: String,
    pub action: Option<Action>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum MenuAction {
    Dashboard,
    Guide,
    Shortcuts,
    Settings,
    Refresh,
    Sidebar,
    Quit,
}

const MENU: &[(&str, &str, MenuAction)] = &[
    ("Open dashboard", "Close the terminal UI and open the dashboard window", MenuAction::Dashboard),
    ("Usage guide", "Read and search the guide offline", MenuAction::Guide),
    ("Keyboard shortcuts", "List every key", MenuAction::Shortcuts),
    ("Settings", "Appearance, data and notifier", MenuAction::Settings),
    ("Refresh now", "Read the database again", MenuAction::Refresh),
    ("Collapse or expand the sidebar", "", MenuAction::Sidebar),
    ("Quit", "", MenuAction::Quit),
];

const NO_DASHBOARD_HINT: &str = "Not available: this build has no dashboard";
const NO_DASHBOARD: &str = "This build has no dashboard. Download the standard build to use the dashboard window.";

pub struct Model<'a> {
    pub(crate) app: &'a mut App,
    pub(crate) options: Options,
    pub(crate) terminal: Option<theme::Terminal>,
    pub(crate) warnings: Vec<String>,
    pub(crate) values: BTreeMap<String, Value>,
    pub(crate) width: usize,
    pub(crate) height: usize,
    pub(crate) nav: &'static [NavItem],
    pub(crate) page: usize,
    pub(crate) sidebar: bool,
    pub(crate) notice: String,
    pub(crate) kind: Notice,
    pub(crate) form: Option<(Form, Submit)>,
    pub(crate) confirm: Option<Confirmation>,
    pub(crate) preview: Option<Preview>,
    menu: Option<usize>,
    welcome: bool,
    switch_to: String,
    quitting: bool,
    pub(crate) overview: Overview,
    pub(crate) last_scan: Option<Scan>,
    pub(crate) job: Option<Job>,
    pub(crate) scan: ScansPage,
    pub(crate) find: FindingsPage,
    pub(crate) hist: HistoryPage,
    pub(crate) rules: RulesPage,
    pub(crate) allow: AllowPage,
    pub(crate) exp: ExportsPage,
    pub(crate) inbox: InboxPage,
    pub(crate) set: SettingsPage,
    pub(crate) about: AboutPage,
}

impl<'a> Model<'a> {
    pub fn new(app: &'a mut App, options: Options) -> Model<'a> {
        let welcome = app.created_database();
        let mut model = Model {
            app,
            options,
            terminal: None,
            warnings: Vec::new(),
            values: BTreeMap::new(),
            width: 100,
            height: 30,
            nav: service::NAVIGATION,
            page: 0,
            sidebar: false,
            notice: String::new(),
            kind: Notice::Info,
            form: None,
            confirm: None,
            preview: None,
            menu: None,
            welcome,
            switch_to: String::new(),
            quitting: false,
            overview: Overview::default(),
            last_scan: None,
            job: None,
            scan: ScansPage::default(),
            find: FindingsPage::new(),
            hist: HistoryPage::default(),
            rules: RulesPage::default(),
            allow: AllowPage::default(),
            exp: ExportsPage::default(),
            inbox: InboxPage::default(),
            set: SettingsPage::default(),
            about: AboutPage::new(),
        };
        model.reload();
        model.sidebar = model.values.get(settings::KEY_SIDEBAR_COLLAPSED).is_some_and(Value::bool);
        model
    }

    pub fn switch_to(&self) -> &str {
        &self.switch_to
    }

    pub fn finished(&self) -> bool {
        self.quitting || !self.switch_to.is_empty()
    }

    pub fn notice(&self) -> (Notice, &str) {
        (self.kind, &self.notice)
    }

    pub fn resize(&mut self, columns: usize, rows: usize) {
        self.width = columns;
        self.height = rows;
    }

    pub fn interval(&self) -> Duration {
        if self.job.is_some() {
            return SCANNING_INTERVAL;
        }
        let seconds = self.values.get(settings::KEY_REFRESH_SECONDS).map(Value::int).unwrap_or(0);
        Duration::from_secs(if seconds < 1 { 2 } else { seconds.unsigned_abs() })
    }

    pub fn style(&self, look: Look) -> Style {
        let Some(terminal) = &self.terminal else {
            return Style::default();
        };
        match look {
            Look::Plain => terminal.text,
            Look::Muted => terminal.muted,
            Look::Accent => terminal.accent,
            Look::Heading => terminal.heading,
            Look::Selected => terminal.selected,
            Look::Border => terminal.border,
            Look::Status => terminal.status_bar,
            Look::Success => terminal.success,
            Look::Warning => terminal.warning,
            Look::Failure => terminal.error,
            Look::Level(level) => terminal.severity(level),
        }
    }

    pub fn profile(&self) -> Profile {
        self.terminal.map(|terminal| terminal.profile).unwrap_or(Profile::Mono)
    }

    pub(crate) fn say(&mut self, kind: Notice, text: impl Into<String>) {
        self.kind = kind;
        self.notice = text.into();
    }

    pub(crate) fn report(&mut self, error: &AppError) {
        if error.is_busy() {
            self.say(Notice::Warning, BUSY);
        } else {
            self.say(Notice::Failure, error.to_string());
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

    fn restyle(&mut self) {
        let values = self.app.settings().effective();
        let Some(values) = self.fail(values) else {
            return;
        };
        let tokens = self.app.appearance(self.options.system_dark);
        let Some(tokens) = self.fail(tokens) else {
            return;
        };
        let monochrome = values.get(settings::KEY_MONOCHROME).is_some_and(Value::bool);
        self.terminal = Some(theme::for_terminal(&tokens, Profile::detect(&self.options.env, monochrome)));
        self.warnings = tokens.warnings;
        self.values = values;
    }

    pub(crate) fn reload(&mut self) {
        self.restyle();
        let overview = self.app.findings().overview();
        let Some(overview) = self.fail(overview) else {
            return;
        };
        self.overview = overview;
        let last = self.app.scans().latest();
        if let Some(last) = self.fail(last) {
            self.last_scan = last;
        }
        self.load_scans();
        self.load_findings();
        self.load_history();
        self.load_rules();
        self.load_allowlists();
        self.load_exports();
        self.load_inbox();
        self.load_settings();
        self.load_about();
        let _ = self.app.poll();
    }

    pub(crate) fn changed(&mut self, text: impl Into<String>) {
        self.reload();
        self.say(Notice::Success, text);
    }

    pub(crate) fn finish_scan(&mut self, outcome: Result<Report>) {
        self.job = None;
        self.reload();
        match outcome {
            Ok(report) => {
                let kind = if report.cancelled() { Notice::Warning } else { Notice::Success };
                self.say(kind, report.summary());
            }
            Err(error) => self.report(&error),
        }
    }

    pub fn wait_for_scan(&mut self, limit: Duration) -> bool {
        let Some(job) = &self.job else {
            return true;
        };
        match job.wait(limit) {
            Some(outcome) => {
                self.finish_scan(outcome);
                true
            }
            None => false,
        }
    }

    pub fn stop_scan(&mut self, limit: Duration) {
        if let Some(job) = self.job.take() {
            job.cancel();
            drop(job.wait(limit));
        }
    }

    pub fn tick(&mut self) {
        if let Some(outcome) = self.job.as_ref().and_then(Job::poll) {
            self.finish_scan(outcome);
            return;
        }
        match self.app.poll() {
            Ok(true) => {
                self.reload();
                if self.form.is_none() && self.job.is_none() {
                    self.say(Notice::Info, "Updated by another window");
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

    pub(crate) fn page_id(&self) -> &'static str {
        self.nav[self.page].id
    }

    pub(crate) fn open(&mut self, id: &str) {
        if let Some(index) = self.nav.iter().position(|item| item.id == id) {
            self.page = index;
        }
    }

    pub(crate) fn ask(&mut self, title: &str, statement: String, accept: &str, action: Action) {
        if !self.values.get(settings::KEY_CONFIRM).is_some_and(Value::bool) {
            self.run(action);
            return;
        }
        self.confirm = Some(Confirmation { title: title.to_string(), statement, accept: accept.to_string(), action });
    }

    pub(crate) fn run(&mut self, action: Action) {
        let done = self.perform(action);
        if let Some(done) = self.fail(done) {
            self.changed(done);
        }
    }

    fn perform(&mut self, action: Action) -> Result<String> {
        match action {
            Action::RemoveDemo => {
                let removed = self.app.scans().remove_demo(false)?;
                Ok(format!("Removed {removed} demo findings"))
            }
            Action::AcceptBaseline => {
                let changed = self.app.findings().accept_baseline("")?;
                Ok(format!("Added {} to the baseline", words::count(changed as i64, "finding", "findings")))
            }
            Action::ResetBaseline => {
                let changed = self.app.findings().reset_baseline("")?;
                Ok(format!("Removed {} from the baseline", words::count(changed as i64, "finding", "findings")))
            }
            Action::RemoveRule(id) => {
                self.app.rulebook().delete(&id)?;
                Ok(format!("Removed rule {id}"))
            }
            Action::RemoveEntry(id) => {
                self.app.allowlists().delete(id)?;
                Ok(format!("Removed allowlist entry {id}"))
            }
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
            Action::OpenLink(link) => self.open_link(&link),
        }
    }

    pub(crate) fn autostart(&self) -> Result<&Autostart> {
        self.options
            .autostart
            .as_ref()
            .ok_or_else(|| AppError::Usage("This build cannot manage the sign-in entry".to_string()))
    }

    pub(crate) fn open_link(&self, link: &Link) -> Result<String> {
        if !link.allowed {
            return Err(AppError::Usage(link.reason.clone()));
        }
        let Some(open) = &self.options.open_url else {
            return Err(AppError::Usage(format!("no browser can be started from here. Copy this link: {}", link.url)));
        };
        open(&link.url).map_err(|error| AppError::Io(format!("{error}: {}", link.url)))?;
        Ok(format!("Opened {} in the browser", link.label))
    }

    pub(crate) fn open_guide(&mut self) {
        self.open(NAV_ABOUT);
        self.about.tab = AboutTab::Guide;
    }

    fn toggle_sidebar(&mut self) {
        self.sidebar = !self.sidebar;
        let saved =
            self.app.settings().set(settings::KEY_SIDEBAR_COLLAPSED, if self.sidebar { "true" } else { "false" });
        if self.fail(saved).is_some() {
            let _ = self.app.poll();
        }
    }

    pub(crate) fn load_demo(&mut self) {
        let added = self.app.scans().load_demo();
        let Some(added) = self.fail(added) else {
            return;
        };
        if added == 0 {
            self.changed("The demo data is already loaded");
        } else {
            self.changed(format!("Added {added} demo findings. Remove them in Settings, Remove demo data."));
        }
    }

    fn choose(&mut self, action: MenuAction) {
        match action {
            MenuAction::Dashboard if self.options.has_dashboard => self.switch_to = FRONT_END_GUI.to_string(),
            MenuAction::Dashboard => self.say(Notice::Warning, NO_DASHBOARD),
            MenuAction::Guide => self.open_guide(),
            MenuAction::Shortcuts => {
                self.open(NAV_ABOUT);
                self.about.tab = AboutTab::Shortcuts;
            }
            MenuAction::Settings => self.open(NAV_SETTINGS),
            MenuAction::Refresh => self.changed("Refreshed"),
            MenuAction::Sidebar => self.toggle_sidebar(),
            MenuAction::Quit => self.quit(),
        }
    }

    fn quit(&mut self) {
        if let Some(job) = &self.job {
            job.cancel();
        }
        self.quitting = true;
    }

    pub(crate) fn show_findings(&mut self, query: Query) {
        self.find.show(query);
        self.load_findings();
        self.open(NAV_FINDINGS);
    }

    pub fn handle(&mut self, key: Key) {
        if key == Key::Ctrl('c') {
            self.quit();
            return;
        }
        self.notice.clear();
        if self.welcome {
            self.welcome = false;
            self.app.dismiss_welcome();
            if key == Key::Char('l') {
                self.load_demo();
            }
            return;
        }
        if let Some((mut form, submit)) = self.form.take() {
            match form.handle(key) {
                FormEvent::Open => self.form = Some((form, submit)),
                FormEvent::Cancel => self.say(Notice::Info, CANCELLED),
                FormEvent::Submit => {
                    if let Err(error) = self.submit(&form, &submit) {
                        form.problem = error.to_string();
                        self.form = Some((form, submit));
                    }
                }
            }
            return;
        }
        if let Some(confirm) = self.confirm.take() {
            match key {
                Key::Char('y' | 'Y') => self.run(confirm.action),
                Key::Char('n' | 'N' | 'q') | Key::Esc => self.say(Notice::Info, CANCELLED),
                _ => self.confirm = Some(confirm),
            }
            return;
        }
        if let Some(mut preview) = self.preview.take() {
            match key {
                Key::Up | Key::Char('k') => preview.offset = preview.offset.saturating_sub(1),
                Key::Down | Key::Char('j') => preview.offset += 1,
                Key::PageUp => preview.offset = preview.offset.saturating_sub(10),
                Key::PageDown | Key::Char(' ') => preview.offset += 10,
                Key::Esc | Key::Char('q' | 'n') => {
                    if preview.action.is_some() {
                        self.say(Notice::Info, CANCELLED);
                    }
                    return;
                }
                Key::Enter | Key::Char('y') => {
                    if let Some(action) = preview.action {
                        self.run(action);
                    }
                    return;
                }
                _ => {}
            }
            self.preview = Some(preview);
            return;
        }
        if let Some(at) = self.menu {
            match key {
                Key::Up | Key::Char('k') => self.menu = Some((at + MENU.len() - 1) % MENU.len()),
                Key::Down | Key::Char('j') => self.menu = Some((at + 1) % MENU.len()),
                Key::Esc | Key::Char('q' | ':') | Key::F(10) => self.menu = None,
                Key::Enter => {
                    self.menu = None;
                    self.choose(MENU[at].2);
                }
                _ => {}
            }
            return;
        }
        if self.captures_text() {
            self.page_key(key);
            return;
        }
        match key {
            Key::Char('q') => self.quit(),
            Key::Tab => self.page = (self.page + 1) % self.nav.len(),
            Key::BackTab => self.page = (self.page + self.nav.len() - 1) % self.nav.len(),
            Key::Char(',') => self.open(NAV_SETTINGS),
            Key::Char('?') | Key::F(1) => self.open_guide(),
            Key::Char('r') => self.changed("Refreshed"),
            Key::Char('b') => self.toggle_sidebar(),
            Key::Char(':') | Key::F(10) => self.menu = Some(0),
            Key::Char(digit @ '0'..='9') if page_of(digit) < self.nav.len() => self.page = page_of(digit),
            _ => self.page_key(key),
        }
    }

    fn captures_text(&self) -> bool {
        match self.page_id() {
            NAV_FINDINGS => self.find.searching,
            NAV_ABOUT => self.about.searching,
            _ => false,
        }
    }

    fn page_key(&mut self, key: Key) {
        match self.page_id() {
            NAV_SCANS => self.scans_key(key),
            NAV_FINDINGS => self.findings_key(key),
            NAV_HISTORY => self.history_key(key),
            NAV_RULES => self.rules_key(key),
            NAV_ALLOWLISTS => self.allowlists_key(key),
            NAV_EXPORTS => self.exports_key(key),
            NAV_NOTIFICATIONS => self.inbox_key(key),
            NAV_SETTINGS => self.settings_key(key),
            NAV_ABOUT => self.about_key(key),
            _ => match key {
                Key::Char('l') => self.load_demo(),
                Key::Char('n') => self.scan_form(),
                Key::Enter => self.show_findings(Query { scope: Scope::Attention, ..Query::default() }),
                _ => {}
            },
        }
    }

    fn submit(&mut self, form: &Form, submit: &Submit) -> Result<()> {
        match submit {
            Submit::Scan => self.submit_scan(form),
            Submit::Decide { id, revision } => self.submit_decide(form, *id, *revision),
            Submit::AddRule => self.submit_rule_spec(form, None),
            Submit::EditRule { id, revision } => self.submit_rule_spec(form, Some((id.as_str(), *revision))),
            Submit::TryRule(id) => self.submit_try(form, id),
            Submit::AddEntry => self.submit_entry(form, None),
            Submit::EditEntry { id, revision } => self.submit_entry(form, Some((*id, *revision))),
            Submit::Export => self.submit_export(form),
            Submit::Rule(event) => self.submit_rule(form, event),
            Submit::Setting(key) => self.submit_setting(form, key),
            Submit::MoveDatabase => self.submit_move(form),
        }
    }

    pub fn hints(&self) -> String {
        let common = ": menu · ? guide · q quit";
        match self.page_id() {
            NAV_SCANS => format!("n new scan · x cancel the running scan · Enter details · f findings of the scan · {common}"),
            NAV_FINDINGS if self.find.searching => "Type to search · Enter keep · Esc clear".to_string(),
            NAV_FINDINGS if self.find.detail => {
                format!("e decide · Up/Down next finding · PageDown scroll · Esc back to the list · {common}")
            }
            NAV_FINDINGS => format!(
                "Enter details · e decide · / search · f scope · v confidence · t decision · s sort · B accept baseline · R reset baseline · {common}"
            ),
            NAV_HISTORY => format!("Enter findings of the commit · {common}"),
            NAV_RULES => format!(
                "n add · e edit · x enable/disable · t try on a sample · u restore built-in · d remove · Enter details · {common}"
            ),
            NAV_ALLOWLISTS => format!("n add · e edit · x enable/disable · d remove · {common}"),
            NAV_EXPORTS => format!("o export to a file · f scope · {common}"),
            NAV_NOTIFICATIONS if self.inbox.rules => format!("e edit rule · u reset rule · Left inbox · {common}"),
            NAV_NOTIFICATIONS => {
                format!("m mark read · M mark all · u unread only · Enter open the scan · Right rules · {common}")
            }
            NAV_SETTINGS => format!("Enter change · u reset · {common}"),
            NAV_ABOUT if self.about.searching => "Type to search the guide · Enter keep · Esc clear".to_string(),
            NAV_ABOUT => format!("Left/Right section · / search guide · Enter open · {common}"),
            _ => format!("n scan a directory · l load demo data · Enter findings · 1-9 and 0 pages · {common}"),
        }
    }

    pub fn view(&mut self) -> Vec<Line> {
        if self.finished() {
            return Vec::new();
        }
        if self.width < MIN_WIDTH || self.height < MIN_HEIGHT {
            return vec![
                Line::plain(format!(
                    "{} needs a terminal of at least {MIN_WIDTH} columns and {MIN_HEIGHT} rows.",
                    meta::APP_NAME
                )),
                Line::plain(format!(
                    "This one is {} by {}. Enlarge the window, or use the commands: {} --help",
                    self.width,
                    self.height,
                    meta::SLUG
                )),
            ];
        }
        let status = self.status_lines();
        let body_height = self.height - 2 - status.len();
        let sidebar_width = if self.sidebar { 4 } else { 20 };
        let content_width = self.width - sidebar_width - 3;
        let content = if self.welcome {
            self.welcome_view(content_width)
        } else if let Some((form, _)) = &self.form {
            form.view(content_width)
        } else if self.confirm.is_some() {
            self.confirm_view(content_width)
        } else if self.preview.is_some() {
            self.preview_view(content_width, body_height)
        } else if let Some(at) = self.menu {
            self.menu_view(at, content_width)
        } else {
            self.page_view(content_width, body_height)
        };
        let sidebar = self.sidebar_view(sidebar_width, body_height);
        let title = format!("{} · {}", meta::APP_NAME, self.nav[self.page].label);
        let room = self.width.saturating_sub(width(&title) + 4);
        let full = self.app.location().path.display().to_string();
        let location = if room < 8 { String::new() } else { tail(&full, room) };
        let gap = self.width.saturating_sub(width(&title) + width(&location)).max(1);
        let mut lines = vec![
            Line::styled(title, Look::Heading).with(" ".repeat(gap), Look::Plain).with(location, Look::Muted),
            Line::styled("─".repeat(self.width), Look::Border),
        ];
        let mut content = content.into_iter();
        let mut sidebar = sidebar.into_iter();
        for _ in 0..body_height - 1 {
            let mut line = sidebar.next().unwrap_or_default().fit(sidebar_width).with(" │ ", Look::Border);
            line.0.extend(content.next().unwrap_or_default().fit(content_width).0);
            lines.push(line);
        }
        lines.extend(status);
        lines
    }

    pub fn screen(&mut self) -> String {
        let mut out = String::new();
        for line in self.view() {
            out.push_str(line.text().trim_end());
            out.push('\n');
        }
        out
    }

    fn status_lines(&self) -> Vec<Line> {
        if self.notice.is_empty() {
            let hints = short_hints(&self.hints(), self.width.saturating_sub(2));
            return vec![Line::styled(pad(&format!(" {hints}"), self.width), Look::Status)];
        }
        let glyph = match self.kind {
            Notice::Info => "ℹ",
            Notice::Success => "✔",
            Notice::Warning => "▲",
            Notice::Failure => "✖",
        };
        let mut wrapped = wrap(&self.notice, self.width.saturating_sub(4));
        if wrapped.len() > MAX_NOTICE_LINES {
            wrapped.truncate(MAX_NOTICE_LINES);
            let last = &mut wrapped[MAX_NOTICE_LINES - 1];
            last.push('…');
        }
        wrapped
            .iter()
            .enumerate()
            .map(|(index, line)| {
                let lead = if index == 0 { format!(" {glyph} ") } else { "   ".to_string() };
                Line::styled(pad(&format!("{lead}{line}"), self.width), Look::Status)
            })
            .collect()
    }

    fn sidebar_view(&self, size: usize, height: usize) -> Vec<Line> {
        let entry = |index: usize| {
            let item = &self.nav[index];
            let digit = (index + 1) % 10;
            let mut label = format!("{digit} {}", item.label);
            if self.sidebar {
                label = format!("{digit}");
            } else if item.id == NAV_NOTIFICATIONS && self.overview.unread_notifications > 0 {
                label.push_str(&format!(" ({})", self.overview.unread_notifications));
            }
            if index == self.page {
                Line::styled(pad(&format!("▌{label}"), size), Look::Selected)
            } else {
                Line::plain(format!(" {label}"))
            }
        };
        let bottom_start = self.nav.iter().position(|item| item.bottom).unwrap_or(self.nav.len());
        let mut lines: Vec<Line> = (0..bottom_start).map(entry).collect();
        let bottom = self.nav.len() - bottom_start;
        while lines.len() + bottom + 1 < height {
            lines.push(Line::new());
        }
        lines.extend((bottom_start..self.nav.len()).map(entry));
        lines
    }

    fn welcome_view(&self, size: usize) -> Vec<Line> {
        let mut lines = vec![Line::styled(format!("Welcome to {}", meta::APP_NAME), Look::Heading), Line::new()];
        lines.extend(wrapped(
            &format!(
                "A new database was created at {}. Everything stays in this one file on your computer. You can move it later in Settings, Database location.",
                self.app.location().path.display()
            ),
            size,
            Look::Plain,
        ));
        lines.push(Line::new());
        lines.extend(wrapped("Press l to load labeled demo data and look around.", size, Look::Plain));
        lines.extend(wrapped("Press any other key to start with an empty database.", size, Look::Plain));
        lines
    }

    fn confirm_view(&self, size: usize) -> Vec<Line> {
        let Some(confirm) = &self.confirm else {
            return Vec::new();
        };
        let mut lines = vec![Line::styled(confirm.title.clone(), Look::Heading), Line::new()];
        lines.extend(wrapped(&confirm.statement, size, Look::Plain));
        lines.push(Line::new());
        lines.extend(wrapped(&format!("▲ Press y to {}, or n or Esc to cancel.", confirm.accept), size, Look::Warning));
        lines
    }

    fn preview_view(&mut self, size: usize, height: usize) -> Vec<Line> {
        let Some(preview) = &mut self.preview else {
            return Vec::new();
        };
        let body: Vec<String> = preview.lines.iter().flat_map(|line| wrap(line, size)).collect();
        let prompt = if preview.action.is_some() {
            wrapped(
                &format!("▲ Press Enter to {}, or Esc to cancel. Up and Down scroll.", preview.accept),
                size,
                Look::Warning,
            )
        } else {
            wrapped("Up and Down scroll · Esc close", size, Look::Muted)
        };
        let room = height.saturating_sub(5 + prompt.len()).max(1);
        preview.offset = preview.offset.min(body.len().saturating_sub(room));
        let mut lines = vec![Line::styled(preview.title.clone(), Look::Heading), Line::new()];
        lines.extend(body.into_iter().skip(preview.offset).take(room).map(Line::plain));
        lines.push(Line::new());
        lines.extend(prompt);
        lines
    }

    fn menu_view(&self, at: usize, size: usize) -> Vec<Line> {
        let mut lines = vec![Line::styled("Menu", Look::Heading), Line::new()];
        for (index, (label, hint, action)) in MENU.iter().enumerate() {
            let hint =
                if *action == MenuAction::Dashboard && !self.options.has_dashboard { NO_DASHBOARD_HINT } else { *hint };
            let line = if index == at {
                Line::styled(pad(&format!("▌ {label}"), 34), Look::Selected)
            } else {
                Line::plain(pad(&format!("  {label}"), 34))
            };
            lines.push(line.with(hint.to_string(), Look::Muted).fit(size));
        }
        lines.push(Line::new());
        lines.extend(wrapped("Up and Down move · Enter choose · Esc close", size, Look::Muted));
        lines
    }

    fn page_view(&mut self, size: usize, height: usize) -> Vec<Line> {
        let room = height.saturating_sub(1);
        match self.page_id() {
            NAV_SCANS => self.scans_view(size, room),
            NAV_FINDINGS => self.findings_view(size, room),
            NAV_HISTORY => self.history_view(size, room),
            NAV_RULES => self.rules_view(size, room),
            NAV_ALLOWLISTS => self.allowlists_view(size, room),
            NAV_EXPORTS => self.exports_view(size),
            NAV_NOTIFICATIONS => self.inbox_view(size, room),
            NAV_SETTINGS => self.settings_view(size, room),
            NAV_ABOUT => self.about_view(size, room),
            _ => self.overview_view(size),
        }
    }

    fn overview_view(&self, size: usize) -> Vec<Line> {
        let overview = &self.overview;
        let card = |label: &str, value: i64, hint: &str| {
            Line::plain(pad(label, 30))
                .with(pad(&value.to_string(), 7), Look::Accent)
                .with(hint.to_string(), Look::Muted)
        };
        let mut lines = vec![
            Line::styled("What needs attention", Look::Heading),
            Line::new(),
            card("Findings that need attention", overview.attention, "open or confirmed, not allowlisted"),
            card("New findings", overview.new, "not in the baseline"),
            card("In the baseline", overview.baseline, "known and still open"),
            card("Allowlisted", overview.allowlisted, "left out by an allowlist entry or a marker"),
            card("Unread notifications", overview.unread_notifications, "press 8 to read them"),
            Line::new(),
        ];
        if let Some(job) = &self.job {
            lines.extend(wrapped(
                &format!("… Scanning {} ({}). Press 2 and then x to cancel.", job.root(), job.source().label()),
                size,
                Look::Accent,
            ));
        }
        let Some(last) = &self.last_scan else {
            lines.extend(wrapped(
                "No scan yet. Press n to scan a directory, or press l to load labeled demo data.",
                size,
                Look::Plain,
            ));
            return lines;
        };
        lines.extend(wrapped(
            &format!(
                "Last scan: {} of {} · {} · {} in {} · {}",
                last.kind.label(),
                last.root,
                last.status.label(),
                last.scope_label(),
                last.duration_label(),
                self.app.format_time(last.started_at)
            ),
            size,
            Look::Plain,
        ));
        lines.push(Line::new());
        if overview.by_rule.is_empty() {
            lines.extend(wrapped("✔ No finding needs attention.", size, Look::Success));
        } else {
            lines.push(Line::styled("Findings that need attention by rule and confidence", Look::Heading));
            let widths = [size.saturating_sub(34).clamp(12, 44), 5, 7, 4, 6];
            lines.push(Line::styled(row(&["RULE", "HIGH", "MEDIUM", "LOW", "TOTAL"], &widths), Look::Muted));
            for item in &overview.by_rule {
                lines.push(Line::plain(row(
                    &[
                        item.rule_name.clone(),
                        item.high.to_string(),
                        item.medium.to_string(),
                        item.low.to_string(),
                        item.total().to_string(),
                    ],
                    &widths,
                )));
            }
            lines.push(Line::new());
            lines.push(Line::styled("Files with the most findings", Look::Heading));
            let widths = [size.saturating_sub(26).clamp(12, 60), 8, 9];
            lines.push(Line::styled(row(&["PATH", "FINDINGS", "TOP SCORE"], &widths), Look::Muted));
            for item in &overview.top_files {
                lines.push(Line::plain(row(
                    &[item.path.clone(), item.total.to_string(), item.top_score.to_string()],
                    &widths,
                )));
            }
        }
        if overview.demo > 0 {
            lines.push(Line::new());
            lines.extend(wrapped(
                &format!(
                    "ℹ Demo data is loaded: {} invented findings in \"{}\". Remove them in Settings, Remove demo data.",
                    overview.demo,
                    app_core::demo::ROOT
                ),
                size,
                Look::Plain,
            ));
        }
        lines
    }
}

fn page_of(digit: char) -> usize {
    match digit {
        '0' => 9,
        other => (other as usize).saturating_sub('1' as usize),
    }
}

pub(crate) fn severity_options() -> Vec<(String, String)> {
    Severity::ALL
        .iter()
        .map(|level| (level.as_str().to_string(), format!("{} {}", level.glyph(), level.label())))
        .collect()
}

pub(crate) fn borrowed(options: &[(String, String)]) -> Vec<(&str, &str)> {
    options.iter().map(|(value, label)| (value.as_str(), label.as_str())).collect()
}

pub(crate) fn next<T: Copy + PartialEq>(options: &[T], current: T) -> T {
    let index = options.iter().position(|option| *option == current).map(|index| index + 1).unwrap_or(0);
    options[index % options.len()]
}

pub(crate) fn move_cursor(cursor: &mut usize, key: Key, count: usize) -> bool {
    let delta = match key {
        Key::Up | Key::Char('k') => -1,
        Key::Down | Key::Char('j') => 1,
        Key::PageUp => -10,
        Key::PageDown => 10,
        _ => return false,
    };
    *cursor = step(*cursor, delta, count);
    true
}

pub fn short_hints(hints: &str, size: usize) -> String {
    if width(hints) <= size {
        return hints.to_string();
    }
    let ending = "… · ? guide · q quit";
    let mut kept = String::new();
    for part in hints.split(" · ") {
        let candidate = if kept.is_empty() { part.to_string() } else { format!("{kept} · {part}") };
        if width(&candidate) + width(ending) + 3 > size {
            break;
        }
        kept = candidate;
    }
    if kept.is_empty() { ending.to_string() } else { format!("{kept} · {ending}") }
}
