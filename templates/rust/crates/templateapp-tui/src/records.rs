use std::path::Path;

use app_core::error::{AppError, Result};
use app_core::records::{self, Input, Query, Record, Status};
use app_core::settings::{self, split_columns, split_sort};
use app_core::theme::Severity;

use crate::model::{Action, LIST_LIMIT, Model, Notice, Preview, Submit, move_cursor};
use crate::text::{Line, Look, pad, width, wrapped};
use crate::widgets::{Field, Form, Input as TextInput, Key, layout, row, selectable};

const SEARCH_LIMIT: usize = 200;
const PATH_LIMIT: usize = 1024;
const FORMATS: &[(&str, &str)] = &[("", "From the file name"), ("csv", "CSV"), ("json", "JSON")];

pub(crate) struct RecordsPage {
    pub query: Query,
    pub list: Vec<Record>,
    pub cursor: usize,
    pub offset: usize,
    pub detail: bool,
    pub searching: bool,
    pub search: TextInput,
}

impl RecordsPage {
    pub fn new() -> RecordsPage {
        RecordsPage {
            query: Query { sort: "updated".to_string(), descending: true, ..Query::default() },
            list: Vec::new(),
            cursor: 0,
            offset: 0,
            detail: false,
            searching: false,
            search: TextInput::new("", SEARCH_LIMIT),
        }
    }

    pub fn selected(&self) -> Option<&Record> {
        self.list.get(self.cursor)
    }

    pub fn select(&mut self, id: i64) {
        if let Some(index) = self.list.iter().position(|record| record.id == id) {
            self.cursor = index;
        }
    }

    fn filtered(&self) -> bool {
        !self.query.search.is_empty() || self.query.status.is_some() || self.query.severity.is_some()
    }
}

pub(crate) fn severity_options() -> Vec<(String, String)> {
    Severity::ALL
        .iter()
        .map(|level| (level.as_str().to_string(), format!("{} {}", level.glyph(), level.label())))
        .collect()
}

fn status_options() -> Vec<(String, String)> {
    Status::ALL
        .iter()
        .map(|status| (status.as_str().to_string(), format!("{} {}", status.glyph(), status.label())))
        .collect()
}

pub(crate) fn borrowed(options: &[(String, String)]) -> Vec<(&str, &str)> {
    options.iter().map(|(value, label)| (value.as_str(), label.as_str())).collect()
}

fn next<T: Copy + PartialEq>(options: &[T], current: T) -> T {
    let index = options.iter().position(|option| *option == current).map(|index| index + 1).unwrap_or(0);
    options[index % options.len()]
}

fn enabled_label(enabled: bool) -> &'static str {
    if enabled { "Enabled" } else { "Disabled" }
}

fn record_form(title: String, input: &Input) -> Form {
    let severities = severity_options();
    let statuses = status_options();
    Form::new(
        title,
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
            Field::text("note", "Note", &input.note, "Optional free text, searchable.", records::MAX_NOTE_LENGTH),
            Field::yes_no("enabled", "Enabled", input.enabled, "Disabled records are kept but not counted."),
        ],
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

impl Model<'_> {
    pub(crate) fn load_records(&mut self) {
        let previous = self.rec.selected().map(|record| record.id);
        if let Some(value) = self.values.get(settings::KEY_RECORD_SORT) {
            let (key, descending) = split_sort(&value.raw);
            if !key.is_empty() {
                self.rec.query.sort = key;
                self.rec.query.descending = descending;
            }
        }
        let query = Query { limit: LIST_LIMIT, ..self.rec.query.clone() };
        let list = self.app.records().list(&query);
        let Some(list) = self.fail(list) else {
            return;
        };
        self.rec.list = list;
        self.rec.cursor = self.rec.cursor.min(self.rec.list.len().saturating_sub(1));
        if let Some(id) = previous {
            self.rec.select(id);
        }
    }

    pub(crate) fn records_key(&mut self, key: Key) {
        if self.rec.searching {
            match key {
                Key::Enter => self.rec.searching = false,
                Key::Esc => {
                    self.rec.searching = false;
                    self.rec.search = TextInput::new("", SEARCH_LIMIT);
                    self.rec.query.search.clear();
                    self.load_records();
                }
                _ => {
                    if self.rec.search.handle(key) {
                        self.rec.query.search = self.rec.search.value();
                        self.rec.cursor = 0;
                        self.load_records();
                    }
                }
            }
            return;
        }
        let count = self.rec.list.len();
        if move_cursor(&mut self.rec.cursor, key, count) {
            return;
        }
        let selected = self.rec.selected().cloned();
        match key {
            Key::Home | Key::Char('g') => self.rec.cursor = 0,
            Key::End | Key::Char('G') => self.rec.cursor = count.saturating_sub(1),
            Key::Enter => self.rec.detail = selected.is_some() && !self.rec.detail,
            Key::Esc => {
                if self.rec.detail {
                    self.rec.detail = false;
                } else if self.rec.filtered() {
                    self.rec.query.search.clear();
                    self.rec.query.status = None;
                    self.rec.query.severity = None;
                    self.rec.search = TextInput::new("", SEARCH_LIMIT);
                    self.load_records();
                }
            }
            Key::Char('/') => {
                self.rec.searching = true;
                self.rec.search = TextInput::new(&self.rec.query.search, SEARCH_LIMIT);
            }
            Key::Char('f') => {
                let options: Vec<Option<Status>> = std::iter::once(None).chain(Status::ALL.map(Some)).collect();
                self.rec.query.status = next(&options, self.rec.query.status);
                self.load_records();
            }
            Key::Char('v') => {
                let options: Vec<Option<Severity>> = std::iter::once(None).chain(Severity::ALL.map(Some)).collect();
                self.rec.query.severity = next(&options, self.rec.query.severity);
                self.load_records();
            }
            Key::Char('s') => {
                let keys = records::SORT_KEYS;
                let at = keys.iter().position(|key| *key == self.rec.query.sort).map(|index| index + 1).unwrap_or(0);
                self.sort_by(keys[at % keys.len()], self.rec.query.descending);
            }
            Key::Char('S') => {
                let sort = self.rec.query.sort.clone();
                self.sort_by(&sort, !self.rec.query.descending);
            }
            Key::Char('c') => self.column_form(),
            Key::Char('a') => {
                self.rec.query.include_disabled = !self.rec.query.include_disabled;
                self.load_records();
            }
            Key::Char('n') => {
                self.form = Some((record_form("Add record".to_string(), &Input::default()), Submit::AddRecord));
            }
            Key::Char('i') => {
                let help = "Path of a CSV or JSON file. Rows are matched by name: new names are added, known names are updated. Nothing is written until you confirm the preview.";
                let fields = vec![
                    Field::text("path", "File", "", help, PATH_LIMIT),
                    Field::choice("format", "Format", "", "", FORMATS),
                ];
                self.form = Some((Form::new("Import records", fields), Submit::Import));
            }
            Key::Char('o') => {
                let help = "Path of the file to write. Every record is exported, including disabled ones. The file is readable only by you.";
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
                self.form = Some((Form::new("Export records", fields), Submit::Export));
            }
            _ => {
                if let Some(record) = selected {
                    self.record_key(key, &record);
                }
            }
        }
    }

    fn record_key(&mut self, key: Key, record: &Record) {
        match key {
            Key::Char('e') => {
                let form = record_form(format!("Edit record {}", record.id), &record.input());
                self.form = Some((form, Submit::EditRecord { id: record.id, revision: record.revision }));
            }
            Key::Char('x') => {
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
            Key::Char('d') | Key::Delete => self.ask(
                "Remove record",
                format!(
                    "This removes record {} ({}) permanently. Its notifications stay in the list without a link.",
                    record.id, record.name
                ),
                "remove it",
                Action::RemoveRecord(record.id),
            ),
            _ => {}
        }
    }

    pub(crate) fn submit_add(&mut self, form: &Form) -> Result<()> {
        let created = self.app.records().create(&form_input(form))?;
        if !created.enabled {
            self.rec.query.include_disabled = true;
        }
        self.changed(format!("Added record {}: {}", created.id, created.name));
        self.rec.select(created.id);
        Ok(())
    }

    pub(crate) fn submit_edit(&mut self, form: &Form, id: i64, revision: i64) -> Result<()> {
        let updated = self.app.records().update(id, revision, &form_input(form));
        match updated {
            Ok(updated) => {
                self.changed(format!("Updated record {}", updated.id));
                Ok(())
            }
            Err(AppError::Conflict(_)) => {
                self.reload();
                Err(AppError::Conflict(
                    "another window changed this record while you were editing. Your text is still here: press Esc, press e again to load the new version, and repeat your change".to_string(),
                ))
            }
            Err(error) => Err(error),
        }
    }

    pub(crate) fn keep(&mut self, key: &str, raw: &str, what: &str) -> Result<()> {
        let saved = self.app.settings().set(key, raw)?;
        self.reload();
        if !saved.overridden_by.is_empty() {
            self.say(Notice::Warning, format!("{what} saved, but the {} still takes precedence", saved.overridden_by));
        }
        Ok(())
    }

    fn sort_by(&mut self, key: &str, descending: bool) {
        let raw = format!("{key}:{}", if descending { "desc" } else { "asc" });
        let kept = self.keep(settings::KEY_RECORD_SORT, &raw, "Sort order");
        self.fail(kept);
    }

    fn column_form(&mut self) {
        let Some(value) = self.values.get(settings::KEY_RECORD_COLUMNS) else {
            return;
        };
        let stored = split_columns(value.stored.as_deref().unwrap_or(value.definition.default));
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
        self.form = Some((Form::new("Columns of the record table", fields), Submit::Columns));
    }

    pub(crate) fn submit_columns(&mut self, form: &Form) -> Result<()> {
        let shown: Vec<&str> =
            settings::RECORD_COLUMNS.iter().copied().filter(|column| form.value(column) == "show").collect();
        self.keep(settings::KEY_RECORD_COLUMNS, &shown.join(","), "Columns")
    }

    pub(crate) fn submit_import(&mut self, form: &Form) -> Result<()> {
        let path = form.value("path").trim().to_string();
        let format = form.value("format");
        if path.is_empty() {
            return Err(AppError::Usage("give the path of the file to import".to_string()));
        }
        let result = self.app.import_file(Path::new(&path), &format, true)?;
        let mut lines = vec![format!("Dry run of {path}:"), String::new(), result.summary(), String::new()];
        lines.extend(result.problems.iter().map(|problem| format!("✖ rejected {problem}")));
        let action = if result.created + result.updated == 0 {
            lines.push("There is nothing to import.".to_string());
            None
        } else {
            Some(Action::Import { path, format })
        };
        self.preview = Some(Preview {
            title: "Import preview".to_string(),
            lines,
            offset: 0,
            accept: "import these rows".to_string(),
            action,
        });
        Ok(())
    }

    pub(crate) fn submit_export(&mut self, form: &Form) -> Result<()> {
        let path = form.value("path").trim().to_string();
        let exported = self.app.export_file(Path::new(&path), &form.value("format"), form.value("overwrite") == "true");
        match exported {
            Ok(count) => {
                self.say(Notice::Success, format!("Exported {count} records to {path}"));
                Ok(())
            }
            Err(AppError::Conflict(_)) => Err(AppError::Conflict(format!(
                "{path} already exists. Choose another name, or set \"If the file exists\" to \"Overwrite it\""
            ))),
            Err(error) => Err(error),
        }
    }

    fn filter_line(&self) -> String {
        let page = &self.rec;
        let search = if page.searching {
            page.search.view(24, true).trim_end().to_string()
        } else if page.query.search.is_empty() {
            "none".to_string()
        } else {
            page.query.search.clone()
        };
        format!(
            "Search: {search} · Status: {} · Severity: {} · Disabled: {} · Sort: {} {}",
            page.query.status.map(Status::label).unwrap_or("all"),
            page.query.severity.map(Severity::label).unwrap_or("all"),
            if page.query.include_disabled { "shown" } else { "hidden" },
            page.query.sort,
            if page.query.descending { "descending" } else { "ascending" },
        )
    }

    pub(crate) fn records_view(&mut self, size: usize, height: usize) -> Vec<Line> {
        let mut lines = wrapped(&self.filter_line(), size, Look::Muted);
        lines.push(Line::new());
        if self.rec.list.is_empty() {
            let message = if self.rec.filtered() {
                "No records match these filters. Press Esc to clear them."
            } else if !self.rec.query.include_disabled && self.overview.disabled > 0 {
                "Every record is disabled. Press a to show disabled records."
            } else {
                "No records yet. Press n to add the first record, i to import a file, or 1 and then l to load demo data."
            };
            lines.extend(wrapped(message, size, Look::Plain));
            return lines;
        }
        let chosen = self.values.get(settings::KEY_RECORD_COLUMNS).map(|value| split_columns(&value.raw));
        let moment = self.rec.list.first().map(|record| width(&self.app.format_time(record.updated_at))).unwrap_or(16);
        let columns: Vec<(&str, usize, bool)> = [
            ("id", 5, false),
            ("name", 16, true),
            ("status", 13, false),
            ("severity", 11, false),
            ("updated", moment, false),
            ("state", 8, false),
        ]
        .into_iter()
        .filter(|(key, _, _)| chosen.as_ref().is_none_or(|shown| shown.iter().any(|column| column == key)))
        .filter(|(key, _, _)| size >= 67 + moment || *key != "updated")
        .collect();
        let shape: Vec<(usize, bool)> = columns.iter().map(|(_, column, flex)| (*column, *flex)).collect();
        let widths = layout(&shape, size.saturating_sub(2));
        let titles: Vec<String> = columns.iter().map(|(key, _, _)| records::column_label(key).to_uppercase()).collect();
        lines.push(Line::styled(format!("  {}", row(&titles, &widths)), Look::Muted));
        let detail_height = if self.rec.detail { 8 } else { 0 };
        let rows = height.saturating_sub(lines.len() + detail_height + 1).max(3);
        let page = &mut self.rec;
        if page.cursor < page.offset {
            page.offset = page.cursor;
        }
        if page.cursor >= page.offset + rows {
            page.offset = page.cursor + 1 - rows;
        }
        page.offset = page.offset.min(page.list.len().saturating_sub(rows));
        let page = &self.rec;
        for (index, record) in page.list.iter().enumerate().skip(page.offset).take(rows) {
            let cells: Vec<String> = columns
                .iter()
                .map(|(key, _, _)| match *key {
                    "id" => record.id.to_string(),
                    "name" => record.name.clone(),
                    "status" => format!("{} {}", record.status.glyph(), record.status.label()),
                    "severity" => format!("{} {}", record.severity.glyph(), record.severity.label()),
                    "updated" => self.app.format_time(record.updated_at),
                    _ => enabled_label(record.enabled).to_string(),
                })
                .collect();
            lines.push(selectable(&row(&cells, &widths), index == page.cursor, size));
        }
        lines.push(Line::styled(format!("Row {} of {}", page.cursor + 1, page.list.len()), Look::Muted));
        if let Some(record) = page.selected().filter(|_| page.detail) {
            lines.push(Line::styled("─".repeat(size), Look::Border));
            lines.push(Line::styled(pad(&format!("Record {}: {}", record.id, record.name), size), Look::Heading));
            lines.push(
                Line::plain(format!("Status {} {} · Severity ", record.status.glyph(), record.status.label()))
                    .with(
                        format!("{} {}", record.severity.glyph(), record.severity.label()),
                        Look::Level(record.severity),
                    )
                    .with(format!(" · {} · Revision {}", enabled_label(record.enabled), record.revision), Look::Plain),
            );
            lines.push(Line::styled(
                format!(
                    "Created {} · Updated {}",
                    self.app.format_time(record.created_at),
                    self.app.format_time(record.updated_at)
                ),
                Look::Muted,
            ));
            let note = if record.note.is_empty() { "No note." } else { record.note.as_str() };
            let mut note_lines = wrapped(note, size, Look::Plain);
            if note_lines.len() > 3 {
                note_lines.truncate(2);
                note_lines.push(Line::plain("… press e to read the whole note"));
            }
            lines.extend(note_lines);
        }
        lines
    }
}
