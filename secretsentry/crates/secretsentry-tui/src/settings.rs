use app_core::error::Result;
use app_core::meta;
use app_core::platform::Os;
use app_core::service::MoveMode;
use app_core::settings::{self, Kind, Value};
use app_core::theme;
use app_core::userconfig::{FRONT_END_GUI, FRONT_END_TUI};

use crate::model::{Action, Confirmation, Model, Notice, Submit, move_cursor};
use crate::text::{Line, Look, pad, width, wrapped};
use crate::widgets::{Field, Form, Key, selectable, tail};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum RowAction {
    Database,
    Demo,
    Notifier,
    Launcher,
}

#[derive(Debug, Clone, Default)]
pub(crate) struct SettingRow {
    pub group: String,
    pub label: String,
    pub value: String,
    pub source: String,
    pub help: String,
    pub key: Option<String>,
    pub action: Option<RowAction>,
}

#[derive(Default)]
pub(crate) struct SettingsPage {
    pub rows: Vec<SettingRow>,
    pub cursor: usize,
    pub offset: usize,
}

const NOTIFIER_HELP: &str = "Enter turns the per-user sign-in entry on or off. The notifier shows system notifications while the application is closed. It needs no administrator rights and makes no network requests.";
const LAUNCHER_HELP: &str = "Which front end opens when the application starts without a command. Enter changes it, u asks again at every start.";
const DEMO_HELP: &str = "Enter removes the invented scans and findings of the demo data after confirmation. Your own scans and findings stay.";
const NOTIFIER_TITLE: &str = "Notifier at sign-in";

fn shown_value(value: &Value) -> String {
    let shown = match value.raw.as_str() {
        "true" => "Yes",
        "false" => "No",
        raw if value.key == settings::KEY_THEME => theme::label(raw),
        raw => raw,
    };
    if shown.is_empty() { "(empty)".to_string() } else { shown.to_string() }
}

pub(crate) fn front_end_label(mode: &str) -> &'static str {
    match mode {
        FRONT_END_GUI => "Dashboard window",
        FRONT_END_TUI => "Terminal UI",
        _ => "Ask every time",
    }
}

fn saved_value(value: &Value) -> &str {
    value.stored.as_deref().unwrap_or(value.definition.default)
}

fn example_database_path() -> String {
    if Os::current() == Os::Windows {
        format!("D:\\Data\\{}.db", meta::SLUG)
    } else {
        format!("/home/you/data/{}.db", meta::SLUG)
    }
}

impl Model<'_> {
    pub(crate) fn load_settings(&mut self) {
        let list = self.app.settings().list();
        let Some(list) = self.fail(list) else {
            return;
        };
        let mut rows: Vec<SettingRow> = Vec::new();
        for value in &list {
            let mut source = value.source.label().to_string();
            let mut help = value.definition.help.to_string();
            if !value.overridden_by.is_empty() {
                source = "overridden".to_string();
                help.push_str(&format!(
                    " Overridden by the {}. The saved value is \"{}\" and applies when the override is removed.",
                    value.overridden_by,
                    saved_value(value)
                ));
            }
            if !value.override_error.is_empty() {
                help.push(' ');
                help.push_str(&value.override_error);
            }
            rows.push(SettingRow {
                group: value.definition.group.to_string(),
                label: value.definition.label.to_string(),
                value: shown_value(value),
                source,
                help,
                key: Some(value.key.clone()),
                action: None,
            });
        }
        let location = self.app.location();
        let path = location.path.display().to_string();
        let (source, help) = if location.overridden_by.is_empty() {
            (
                location.source.to_string(),
                format!(
                    "In use: {path}. Enter copies the database to a new place and uses it, or switches to another database. The old file is never deleted."
                ),
            )
        } else {
            (
                "overridden".to_string(),
                format!(
                    "In use: {path}. Set by the {}. Remove the override to change the location here.",
                    location.overridden_by
                ),
            )
        };
        let action =
            |group: &str, label: &str, value: String, source: &str, help: String, action: RowAction| SettingRow {
                group: group.to_string(),
                label: label.to_string(),
                value,
                source: source.to_string(),
                help,
                key: None,
                action: Some(action),
            };
        rows.push(action("Data", "Database location", path, &source, help, RowAction::Database));
        rows.push(action(
            "Data",
            "Remove demo data",
            format!("{} demo findings", self.overview.demo),
            "action",
            DEMO_HELP.to_string(),
            RowAction::Demo,
        ));
        let (notifier, notifier_help) = match self.options.autostart.as_ref().map(|entry| entry.status()) {
            None => ("Not available", String::new()),
            Some(Err(error)) => ("Unknown", format!("{error} ")),
            Some(Ok((true, entry))) => ("On", format!("Entry: {}. ", entry.display())),
            Some(Ok((false, _))) => ("Off", String::new()),
        };
        rows.push(action(
            "Notifications",
            NOTIFIER_TITLE,
            notifier.to_string(),
            "action",
            format!("{notifier_help}{NOTIFIER_HELP}"),
            RowAction::Notifier,
        ));
        let variable = meta::env_name("ui");
        let front_end = location.config.front_end.as_str();
        let (launcher_source, launcher_help) = match self.options.env.get(&variable).filter(|value| !value.is_empty()) {
            Some(value) => ("overridden", format!("Overridden by the environment variable {variable}={value}. ")),
            None => (if front_end.is_empty() { "default" } else { "saved" }, String::new()),
        };
        rows.push(action(
            "Launcher",
            "Open with",
            front_end_label(front_end).to_string(),
            launcher_source,
            format!("{launcher_help}{LAUNCHER_HELP}"),
            RowAction::Launcher,
        ));
        let mut groups: Vec<String> = settings::groups(&list).into_iter().map(str::to_string).collect();
        for extra in ["Data", "Notifications", "Launcher"] {
            if !groups.iter().any(|group| group == extra) {
                groups.push(extra.to_string());
            }
        }
        let mut ordered = Vec::with_capacity(rows.len());
        for group in &groups {
            ordered.extend(rows.iter().filter(|row| &row.group == group).cloned());
        }
        self.set.cursor = self.set.cursor.min(ordered.len().saturating_sub(1));
        self.set.rows = ordered;
    }

    pub(crate) fn settings_key(&mut self, key: Key) {
        let count = self.set.rows.len();
        if move_cursor(&mut self.set.cursor, key, count) {
            return;
        }
        let Some(row) = self.set.rows.get(self.set.cursor).cloned() else {
            return;
        };
        match (key, row.key.as_deref(), row.action) {
            (Key::Enter, Some(setting), _) => self.edit_setting(setting),
            (Key::Char('u'), Some(setting), _) => {
                let reset = self.app.settings().reset(setting);
                let Some(value) = self.fail(reset) else {
                    return;
                };
                let label = value.definition.label;
                self.changed(format!("{label} is back to its default"));
                if !value.overridden_by.is_empty() {
                    self.say(
                        Notice::Warning,
                        format!(
                            "{label} is back to its default, but the {} still takes precedence",
                            value.overridden_by
                        ),
                    );
                }
            }
            (Key::Enter, _, Some(RowAction::Database)) => {
                let help = format!(
                    "Full path of the database file, for example {}. The current file stays where it is.",
                    example_database_path()
                );
                let fields = vec![
                    Field::text("path", "New location", "", &help, 1024),
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
                self.form = Some((Form::new("Database location", fields), Submit::MoveDatabase));
            }
            (Key::Enter, _, Some(RowAction::Demo)) => {
                let count = self.app.scans().remove_demo(true);
                let Some(count) = self.fail(count) else {
                    return;
                };
                if count == 0 {
                    self.say(Notice::Info, "There is no demo data to remove");
                    return;
                }
                self.ask(
                    "Remove demo data",
                    format!("This removes {count} demo findings with their scans. Your own scans and findings stay."),
                    "remove them",
                    Action::RemoveDemo,
                );
            }
            (Key::Enter, _, Some(RowAction::Notifier)) => self.toggle_notifier(),
            (Key::Enter | Key::Char('u'), _, Some(RowAction::Launcher)) => {
                let target = if key == Key::Enter {
                    match self.app.location().config.front_end.as_str() {
                        "" => FRONT_END_GUI,
                        FRONT_END_GUI => FRONT_END_TUI,
                        _ => "",
                    }
                } else {
                    ""
                };
                let remembered = self.app.remember_front_end(target);
                if self.fail(remembered).is_some() {
                    self.changed(format!("Open with: {}", front_end_label(target)));
                }
            }
            _ => {}
        }
    }

    fn edit_setting(&mut self, key: &str) {
        let value = self.app.settings().get(key);
        let Some(value) = self.fail(value) else {
            return;
        };
        let definition = &value.definition;
        let mut help = definition.help.to_string();
        if !value.overridden_by.is_empty() {
            help.push_str(&format!(
                " This value is overridden by the {}; your change is saved and applies when the override is removed.",
                value.overridden_by
            ));
        }
        let current = saved_value(&value);
        let field = match definition.kind {
            Kind::Bool => Field::yes_no("value", definition.label, current == "true", &help),
            Kind::Enum => {
                let options: Vec<(&str, &str)> = definition
                    .options
                    .iter()
                    .map(|option| (*option, if key == settings::KEY_THEME { theme::label(option) } else { *option }))
                    .collect();
                Field::choice("value", definition.label, current, &help, &options)
            }
            Kind::Int => Field::text(
                "value",
                definition.label,
                current,
                &format!("{help} Whole number from {} to {}.", definition.min, definition.max),
                12,
            ),
            _ => Field::text("value", definition.label, current, &help, 200),
        };
        let title = format!("{}, {}", definition.group, definition.label);
        self.form = Some((Form::new(title, vec![field]), Submit::Setting(key.to_string())));
    }

    pub(crate) fn submit_setting(&mut self, form: &Form, key: &str) -> Result<()> {
        let saved = self.app.settings().set(key, &form.value("value"))?;
        let label = saved.definition.label;
        self.changed(format!("Saved {label}"));
        if !saved.overridden_by.is_empty() {
            self.say(Notice::Warning, format!("Saved {label}, but the {} still takes precedence", saved.overridden_by));
        }
        if key == settings::KEY_ACCENT
            && let Some(warning) = self.warnings.last().cloned()
        {
            self.say(Notice::Warning, warning);
        }
        Ok(())
    }

    pub(crate) fn submit_move(&mut self, form: &Form) -> Result<()> {
        let mode = if form.value("mode") == "switch" { MoveMode::Switch } else { MoveMode::Copy };
        let plan = self.app.plan_move(form.value("path").trim(), mode)?;
        self.confirm = Some(Confirmation {
            title: "Database location".to_string(),
            statement: plan.statement.clone(),
            accept: "continue".to_string(),
            action: Action::MoveDatabase(plan),
        });
        Ok(())
    }

    fn toggle_notifier(&mut self) {
        let status = self.autostart().and_then(|entry| entry.status());
        let Some((installed, path)) = self.fail(status) else {
            return;
        };
        if installed {
            self.ask(
                NOTIFIER_TITLE,
                format!(
                    "This removes {}. The notifier no longer starts when you sign in. Nothing else changes.",
                    path.display()
                ),
                "remove the entry",
                Action::UninstallNotifier,
            );
            return;
        }
        let target = self.autostart().and_then(|entry| entry.install(true));
        let Some(target) = self.fail(target) else {
            return;
        };
        self.confirm = Some(Confirmation {
            title: NOTIFIER_TITLE.to_string(),
            statement: format!(
                "This writes {} so that \"{} notifier run\" starts when you sign in. It needs no administrator rights and can be removed here at any time.",
                target.display(),
                meta::SLUG
            ),
            accept: "write the entry".to_string(),
            action: Action::InstallNotifier,
        });
    }

    pub(crate) fn settings_view(&mut self, size: usize, height: usize) -> Vec<Line> {
        let page = &self.set;
        let label_width =
            page.rows.iter().map(|row| width(&row.label)).max().unwrap_or(0).min(size.saturating_sub(36).max(18));
        let value_width = size.saturating_sub(label_width + 18).max(12);
        let mut all: Vec<Line> = Vec::new();
        let mut cursor_line = 0;
        let mut group = "";
        for (index, row) in page.rows.iter().enumerate() {
            if row.group != group {
                if !group.is_empty() {
                    all.push(Line::new());
                }
                group = &row.group;
                all.push(Line::styled(group.to_string(), Look::Heading));
            }
            let text = format!(
                "{}  {}  {}",
                pad(&row.label, label_width),
                pad(&tail(&row.value, value_width), value_width),
                row.source
            );
            if index == page.cursor {
                cursor_line = all.len();
            }
            all.push(selectable(&text, index == page.cursor, size));
        }
        let help = page.rows.get(page.cursor).map(|row| wrapped(&row.help, size, Look::Muted)).unwrap_or_default();
        let rows = height.saturating_sub(help.len() + 2).max(3);
        let mut offset = page.offset;
        if cursor_line < offset + 1 {
            offset = cursor_line.saturating_sub(1);
        }
        if cursor_line >= offset + rows {
            offset = cursor_line + 1 - rows;
        }
        let mut lines: Vec<Line> = all.into_iter().skip(offset).take(rows).collect();
        lines.push(Line::styled("─".repeat(size), Look::Border));
        lines.extend(help);
        self.set.offset = offset;
        lines
    }
}
