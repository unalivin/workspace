use std::path::Path;

use app_core::error::{AppError, Result};
use app_core::export::{self, Format};
use app_core::findings::{self, CommitGroup, Query, Scope};
use app_core::words;

use crate::model::{LIST_LIMIT, Model, Notice, Submit, move_cursor, next};
use crate::text::{Line, Look, width, wrapped};
use crate::widgets::{Field, Form, Key, table};

const PATH_LIMIT: usize = 1024;
const FORMATS: &[(&str, &str)] = &[("", "From the file name"), ("json", "JSON"), ("sarif", "SARIF 2.1.0")];
const PATH_HELP: &str = "Path of the file to write. It holds redacted previews only, never a full secret value, and is readable only by you.";

#[derive(Default)]
pub(crate) struct HistoryPage {
    pub list: Vec<CommitGroup>,
    pub cursor: usize,
}

pub(crate) struct ExportsPage {
    pub scope: Scope,
    pub total: i64,
    pub last: String,
}

impl Default for ExportsPage {
    fn default() -> Self {
        ExportsPage { scope: Scope::Attention, total: 0, last: String::new() }
    }
}

impl Model<'_> {
    pub(crate) fn load_history(&mut self) {
        let list = self.app.findings().commits(LIST_LIMIT);
        let Some(list) = self.fail(list) else {
            return;
        };
        self.hist.list = list;
        self.hist.cursor = self.hist.cursor.min(self.hist.list.len().saturating_sub(1));
    }

    pub(crate) fn history_key(&mut self, key: Key) {
        let count = self.hist.list.len();
        if move_cursor(&mut self.hist.cursor, key, count) {
            return;
        }
        match key {
            Key::Home | Key::Char('g') => self.hist.cursor = 0,
            Key::End | Key::Char('G') => self.hist.cursor = count.saturating_sub(1),
            Key::Enter => {
                if let Some(group) = self.hist.list.get(self.hist.cursor) {
                    let query =
                        Query { commit_id: group.commit_id.clone(), root: group.root.clone(), ..Query::default() };
                    self.show_findings(query);
                }
            }
            _ => {}
        }
    }

    pub(crate) fn history_view(&mut self, size: usize, height: usize) -> Vec<Line> {
        let mut lines = wrapped(
            "Findings in Git history, by the commit that added them. A secret that was removed later is still in the history.",
            size,
            Look::Muted,
        );
        lines.push(Line::new());
        if self.hist.list.is_empty() {
            lines.extend(wrapped(
                "No findings in Git history yet. Press 2 and then n, and choose Git history as what to scan.",
                size,
                Look::Plain,
            ));
            return lines;
        }
        let moment = self
            .hist
            .list
            .iter()
            .find_map(|group| group.authored_at)
            .map(|moment| width(&self.app.format_time(moment)))
            .unwrap_or(16);
        let columns: Vec<(&str, usize, bool)> = [
            ("COMMIT", 10, false),
            ("AUTHORED", moment, false),
            ("AUTHOR", 14, false),
            ("FINDINGS", 8, false),
            ("OPEN", 4, false),
            ("TOP", 3, false),
            ("SUBJECT", 16, true),
        ]
        .into_iter()
        .filter(|(title, _, _)| size >= 78 + moment || *title != "AUTHOR")
        .filter(|(title, _, _)| size >= 60 + moment || *title != "AUTHORED")
        .collect();
        let rows: Vec<Vec<String>> = self
            .hist
            .list
            .iter()
            .map(|group| {
                columns
                    .iter()
                    .map(|(title, _, _)| match *title {
                        "COMMIT" => group.short_id().to_string(),
                        "AUTHORED" => group
                            .authored_at
                            .map_or_else(|| "unknown".to_string(), |moment| self.app.format_time(moment)),
                        "AUTHOR" => group.author.clone(),
                        "FINDINGS" => group.findings.to_string(),
                        "OPEN" => group.attention.to_string(),
                        "TOP" => group.top_score.to_string(),
                        _ => group.subject.clone(),
                    })
                    .collect()
            })
            .collect();
        let room = height.saturating_sub(lines.len() + 2);
        lines.extend(table(&columns, &rows, self.hist.cursor, room, size));
        lines
    }

    pub(crate) fn load_exports(&mut self) {
        let total = self.app.findings().total(&Query { scope: self.exp.scope, ..Query::default() });
        if let Some(total) = self.fail(total) {
            self.exp.total = total;
        }
    }

    pub(crate) fn exports_key(&mut self, key: Key) {
        match key {
            Key::Char('f') => {
                self.exp.scope = next(&Scope::ALL, self.exp.scope);
                self.load_exports();
            }
            Key::Char('o') | Key::Enter => {
                let scopes: Vec<(&str, &str)> = Scope::ALL.iter().map(|scope| (scope.as_str(), scope.label())).collect();
                let name = export::default_file_name(Format::Json, self.app.now());
                let fields = vec![
                    Field::text("path", "File", &name, PATH_HELP, PATH_LIMIT),
                    Field::choice("format", "Format", "", "", FORMATS),
                    Field::choice("scope", "Findings", self.exp.scope.as_str(), "", &scopes),
                    Field::choice(
                        "overwrite",
                        "If the file exists",
                        "false",
                        "",
                        &[("false", "Stop and keep it"), ("true", "Overwrite it")],
                    ),
                ];
                self.form = Some((Form::new("Export findings", fields), Submit::Export));
            }
            _ => {}
        }
    }

    pub(crate) fn submit_export(&mut self, form: &Form) -> Result<()> {
        let path = form.value("path").trim().to_string();
        if path.is_empty() {
            return Err(AppError::Usage("give the path of the file to write".to_string()));
        }
        let format = Format::for_file(&path, &form.value("format"))?;
        let scope = Scope::parse(&form.value("scope")).unwrap_or(Scope::All);
        let query = Query {
            scope,
            limit: findings::MAX_LIMIT,
            sort: "path".to_string(),
            descending: false,
            ..Query::default()
        };
        let items = export::collect(&self.app.findings(), &query)?;
        let written = export::write(Path::new(&path), format, &items, self.app.now(), form.value("overwrite") == "true");
        match written {
            Ok(count) => {
                let done = format!(
                    "Exported {} as {} to {path}",
                    words::count(count as i64, "finding", "findings"),
                    format.label()
                );
                self.exp.last = done.clone();
                self.exp.scope = scope;
                self.load_exports();
                self.say(Notice::Success, done);
                Ok(())
            }
            Err(AppError::Conflict(_)) => Err(AppError::Conflict(format!(
                "{path} already exists. Choose another name, or set \"If the file exists\" to \"Overwrite it\""
            ))),
            Err(error) => Err(error),
        }
    }

    pub(crate) fn exports_view(&self, size: usize) -> Vec<Line> {
        let mut lines = vec![Line::styled("Export findings", Look::Heading), Line::new()];
        lines.extend(wrapped(
            "A report holds the rule, the place, the confidence, your decision and a redacted preview of every finding. It never holds a full secret value.",
            size,
            Look::Plain,
        ));
        lines.push(Line::new());
        lines.push(
            Line::plain("Findings to export  ")
                .with(self.exp.scope.label(), Look::Accent)
                .with(format!("  ({})", words::count(self.exp.total, "finding", "findings")), Look::Muted),
        );
        lines.push(Line::new());
        lines.push(Line::styled("Formats", Look::Heading));
        lines.extend(wrapped("JSON         for scripts and for comparing two scans", size, Look::Plain));
        lines.extend(wrapped("SARIF 2.1.0  for code scanning views and CI systems", size, Look::Plain));
        lines.push(Line::new());
        lines.extend(wrapped("Press o to choose the file and export. Press f to change which findings.", size, Look::Muted));
        if !self.exp.last.is_empty() {
            lines.push(Line::new());
            lines.extend(wrapped(&format!("✔ {}", self.exp.last), size, Look::Success));
        }
        lines
    }
}
