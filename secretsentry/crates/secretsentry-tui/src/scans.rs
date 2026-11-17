use app_core::error::Result;
use app_core::findings::Query;
use app_core::jobs::{Job, Request, Source};
use app_core::scans::Scan;
use app_core::service::NAV_SCANS;
use app_core::tree;

use crate::model::{LIST_LIMIT, Model, Notice, Submit, move_cursor};
use crate::text::{Line, Look, pad, width, wrapped};
use crate::widgets::{Field, Form, Key, table};

const PATH_LIMIT: usize = 1024;
const ROOT_HELP: &str = "Path of the directory to scan. Its content is read on this computer only; nothing is sent anywhere and no secret value is stored.";
const SOURCE_HELP: &str =
    "Git history reads every commit of the repository in the directory, including secrets that were removed later.";

#[derive(Default)]
pub(crate) struct ScansPage {
    pub list: Vec<Scan>,
    pub cursor: usize,
    pub detail: bool,
}

impl ScansPage {
    pub fn selected(&self) -> Option<&Scan> {
        self.list.get(self.cursor)
    }

    pub fn select(&mut self, id: i64) -> bool {
        match self.list.iter().position(|scan| scan.id == id) {
            Some(index) => {
                self.cursor = index;
                true
            }
            None => false,
        }
    }
}

impl Model<'_> {
    pub(crate) fn load_scans(&mut self) {
        let previous = self.scan.selected().map(|scan| scan.id);
        let list = self.app.scans().list(None, LIST_LIMIT);
        let Some(list) = self.fail(list) else {
            return;
        };
        self.scan.list = list;
        self.scan.cursor = self.scan.cursor.min(self.scan.list.len().saturating_sub(1));
        if let Some(id) = previous {
            self.scan.select(id);
        }
    }

    pub(crate) fn scan_form(&mut self) {
        if self.job.is_some() {
            self.say(Notice::Warning, "A scan is already running. Press 2 and then x to cancel it.");
            return;
        }
        let last = self.scan.list.iter().find(|scan| !scan.demo).map(|scan| scan.root.clone()).unwrap_or_default();
        let sources: Vec<(&str, &str)> = Source::ALL.iter().map(|source| (source.as_str(), source.label())).collect();
        let fields = vec![
            Field::text("root", "Directory", &last, ROOT_HELP, PATH_LIMIT),
            Field::choice("source", "What to scan", Source::Files.as_str(), SOURCE_HELP, &sources),
        ];
        self.form = Some((Form::new("Scan a directory", fields), Submit::Scan));
    }

    pub(crate) fn submit_scan(&mut self, form: &Form) -> Result<()> {
        let source = Source::parse(&form.value("source")).unwrap_or(Source::Files);
        let request = Request::from_settings(self.app, &form.value("root"), source)?;
        tree::check_root(&request.root)?;
        self.job = Some(Job::start(self.app.options().clone(), request));
        self.open(NAV_SCANS);
        self.say(Notice::Info, "The scan is running. Press x to cancel it.");
        Ok(())
    }

    pub(crate) fn scans_key(&mut self, key: Key) {
        let count = self.scan.list.len();
        if move_cursor(&mut self.scan.cursor, key, count) {
            return;
        }
        match key {
            Key::Home | Key::Char('g') => self.scan.cursor = 0,
            Key::End | Key::Char('G') => self.scan.cursor = count.saturating_sub(1),
            Key::Char('n') => self.scan_form(),
            Key::Char('x') => match &self.job {
                Some(job) => {
                    job.cancel();
                    self.say(Notice::Info, "Cancelling the scan. Findings recorded by earlier scans stay.");
                }
                None => self.say(Notice::Info, "No scan is running"),
            },
            Key::Enter => self.scan.detail = self.scan.selected().is_some() && !self.scan.detail,
            Key::Esc => self.scan.detail = false,
            Key::Char('f') => {
                if let Some(scan) = self.scan.selected() {
                    let query = Query { scan_id: scan.id, ..Query::default() };
                    self.show_findings(query);
                }
            }
            _ => {}
        }
    }

    pub(crate) fn scans_view(&mut self, size: usize, height: usize) -> Vec<Line> {
        let mut lines = Vec::new();
        if let Some(job) = &self.job {
            let state = if job.cancelling() { "Cancelling the scan of" } else { "Scanning" };
            lines.extend(wrapped(
                &format!("… {state} {} ({}). Press x to cancel.", job.root(), job.source().label()),
                size,
                Look::Accent,
            ));
            lines.push(Line::new());
        }
        if self.scan.list.is_empty() {
            lines.extend(wrapped(
                "No scans yet. Press n to scan a directory, or press 1 and then l to load labeled demo data.",
                size,
                Look::Plain,
            ));
            return lines;
        }
        let moment = self.scan.list.first().map(|scan| width(&self.app.format_time(scan.started_at))).unwrap_or(16);
        let columns: Vec<(&str, usize, bool)> = [
            ("ID", 4, false),
            ("STARTED", moment, false),
            ("KIND", 11, false),
            ("STATUS", 9, false),
            ("FINDINGS", 8, false),
            ("NEW", 4, false),
            ("SCANNED DIRECTORY", 20, true),
        ]
        .into_iter()
        .filter(|(title, _, _)| size >= 62 + moment || *title != "STARTED")
        .collect();
        let rows: Vec<Vec<String>> = self
            .scan
            .list
            .iter()
            .map(|scan| {
                columns
                    .iter()
                    .map(|(title, _, _)| match *title {
                        "ID" => scan.id.to_string(),
                        "STARTED" => self.app.format_time(scan.started_at),
                        "KIND" => scan.kind.label().to_string(),
                        "STATUS" => scan.status.label().to_string(),
                        "FINDINGS" => scan.findings_seen.to_string(),
                        "NEW" => scan.findings_new.to_string(),
                        _ => scan.root.clone(),
                    })
                    .collect()
            })
            .collect();
        let detail_height = if self.scan.detail { 7 } else { 0 };
        let room = height.saturating_sub(lines.len() + detail_height + 2);
        lines.extend(table(&columns, &rows, self.scan.cursor, room, size));
        if let Some(scan) = self.scan.selected().filter(|_| self.scan.detail) {
            let demo = if scan.demo { " (demo data)" } else { "" };
            lines.push(Line::styled("─".repeat(size), Look::Border));
            lines.push(Line::styled(
                pad(&format!("Scan {}: {} of {}{demo}", scan.id, scan.kind.label(), scan.root), size),
                Look::Heading,
            ));
            lines.push(Line::plain("Status ").with(scan.status.label(), Look::Level(scan.status.severity())).with(
                format!(
                    " · {} in {} · Started {}",
                    scan.scope_label(),
                    scan.duration_label(),
                    self.app.format_time(scan.started_at)
                ),
                Look::Plain,
            ));
            lines.push(Line::plain(format!(
                "Skipped files {} · Rules used {} · Findings seen {} · New findings {}",
                scan.files_skipped, scan.rules_used, scan.findings_seen, scan.findings_new
            )));
            let problem = if scan.problem.is_empty() { "No problem was reported." } else { scan.problem.as_str() };
            lines.extend(wrapped(problem, size, Look::Muted).into_iter().take(3));
        }
        lines
    }
}
