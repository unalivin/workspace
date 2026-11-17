use app_core::allow;
use app_core::confidence::Level;
use app_core::error::{AppError, Result};
use app_core::findings::{self, Finding, Occurrence, Query, Scope, Status};
use app_core::scanner::Allowed;

use crate::model::{Action, LIST_LIMIT, Model, Submit, move_cursor, next};
use crate::text::{Line, Look, pad, wrapped};
use crate::widgets::{Field, Form, Input as TextInput, Key, table};

const SEARCH_LIMIT: usize = 200;
const DETAIL_STEP: usize = 5;
const DECISION_HELP: &str = "Open and confirmed findings need attention. False positive, accepted risk and resolved close the finding.";
const ACCEPT_STATEMENT: &str = "This marks every current finding as known. Findings that appear later count as new. Decisions and notes stay.";
const RESET_STATEMENT: &str = "This empties the baseline: every finding counts as new again. Decisions and notes stay.";

pub(crate) struct FindingsPage {
    pub query: Query,
    pub list: Vec<Finding>,
    pub total: i64,
    pub cursor: usize,
    pub detail: bool,
    pub places: Vec<Occurrence>,
    pub scroll: usize,
    pub searching: bool,
    pub search: TextInput,
}

impl FindingsPage {
    pub fn new() -> FindingsPage {
        FindingsPage {
            query: Query::default(),
            list: Vec::new(),
            total: 0,
            cursor: 0,
            detail: false,
            places: Vec::new(),
            scroll: 0,
            searching: false,
            search: TextInput::new("", SEARCH_LIMIT),
        }
    }

    pub fn selected(&self) -> Option<&Finding> {
        self.list.get(self.cursor)
    }

    pub fn select(&mut self, id: i64) {
        if let Some(index) = self.list.iter().position(|finding| finding.id == id) {
            self.cursor = index;
        }
    }

    pub fn show(&mut self, query: Query) {
        self.search = TextInput::new(&query.search, SEARCH_LIMIT);
        self.query = query;
        self.cursor = 0;
        self.scroll = 0;
        self.detail = false;
        self.searching = false;
    }

    fn filtered(&self) -> bool {
        let query = &self.query;
        !query.search.is_empty()
            || query.level.is_some()
            || query.status.is_some()
            || query.scope != Scope::All
            || query.scan_id != 0
            || !query.commit_id.is_empty()
            || !query.root.is_empty()
    }
}

fn confidence_text(finding: &Finding) -> String {
    format!("{} {}", finding.level().label(), finding.score)
}

fn found_in(finding: &Finding) -> &'static str {
    match (finding.in_tree, finding.in_history) {
        (true, true) => "files and Git history",
        (true, false) => "files",
        (false, true) => "Git history",
        (false, false) => "no longer found",
    }
}

fn allowance(finding: &Finding) -> String {
    match finding.allowed {
        Allowed::No => "no".to_string(),
        Allowed::Marker => format!("yes, by the marker {} on the line", allow::LINE_MARKER),
        Allowed::Entry(id) => format!("yes, by allowlist entry {id}"),
    }
}

impl Model<'_> {
    pub(crate) fn load_findings(&mut self) {
        let previous = self.find.selected().map(|finding| finding.id);
        let query = Query { limit: LIST_LIMIT, ..self.find.query.clone() };
        let page = self.app.findings().page(&query);
        let Some(page) = self.fail(page) else {
            return;
        };
        self.find.list = page.findings;
        self.find.total = page.total;
        self.find.cursor = self.find.cursor.min(self.find.list.len().saturating_sub(1));
        if let Some(id) = previous {
            self.find.select(id);
        }
        if self.find.list.is_empty() {
            self.find.detail = false;
        }
        self.load_places();
    }

    fn load_places(&mut self) {
        self.find.places.clear();
        let Some(id) = self.find.selected().filter(|_| self.find.detail).map(|finding| finding.id) else {
            return;
        };
        let places = self.app.findings().occurrences(id);
        if let Some(places) = self.fail(places) {
            self.find.places = places;
        }
    }

    fn filter_by(&mut self, change: impl FnOnce(&mut Query)) {
        change(&mut self.find.query);
        self.find.cursor = 0;
        self.load_findings();
    }

    pub(crate) fn findings_key(&mut self, key: Key) {
        if self.find.searching {
            match key {
                Key::Enter => self.find.searching = false,
                Key::Esc => {
                    self.find.searching = false;
                    self.find.search = TextInput::new("", SEARCH_LIMIT);
                    self.filter_by(|query| query.search.clear());
                }
                _ => {
                    if self.find.search.handle(key) {
                        let text = self.find.search.value();
                        self.filter_by(|query| query.search = text);
                    }
                }
            }
            return;
        }
        if self.find.detail {
            match key {
                Key::PageDown | Key::Char(' ') => {
                    self.find.scroll += DETAIL_STEP;
                    return;
                }
                Key::PageUp => {
                    self.find.scroll = self.find.scroll.saturating_sub(DETAIL_STEP);
                    return;
                }
                _ => {}
            }
        }
        let count = self.find.list.len();
        if move_cursor(&mut self.find.cursor, key, count) {
            self.find.scroll = 0;
            self.load_places();
            return;
        }
        match key {
            Key::Home | Key::Char('g') => self.find.cursor = 0,
            Key::End | Key::Char('G') => self.find.cursor = count.saturating_sub(1),
            Key::Enter => {
                self.find.detail = self.find.selected().is_some() && !self.find.detail;
                self.find.scroll = 0;
                self.load_places();
            }
            Key::Esc => {
                if self.find.detail {
                    self.find.detail = false;
                } else if self.find.filtered() {
                    self.find.show(Query {
                        sort: self.find.query.sort.clone(),
                        descending: self.find.query.descending,
                        ..Query::default()
                    });
                    self.load_findings();
                }
            }
            Key::Char('/') => {
                self.find.detail = false;
                self.find.searching = true;
                self.find.search = TextInput::new(&self.find.query.search, SEARCH_LIMIT);
            }
            Key::Char('f') => self.filter_by(|query| query.scope = next(&Scope::ALL, query.scope)),
            Key::Char('v') => {
                let options: Vec<Option<Level>> = std::iter::once(None).chain(Level::ALL.map(Some)).collect();
                self.filter_by(|query| query.level = next(&options, query.level));
            }
            Key::Char('t') => {
                let options: Vec<Option<Status>> = std::iter::once(None).chain(Status::ALL.map(Some)).collect();
                self.filter_by(|query| query.status = next(&options, query.status));
            }
            Key::Char('s') => {
                let keys = findings::SORT_KEYS;
                let at = keys.iter().position(|key| *key == self.find.query.sort).map_or(0, |index| index + 1);
                let chosen = keys[at % keys.len()];
                self.filter_by(|query| {
                    query.sort = chosen.to_string();
                    query.descending = matches!(chosen, "score" | "seen");
                });
            }
            Key::Char('S') => self.filter_by(|query| query.descending = !query.descending),
            Key::Char('e') => self.decide_form(),
            Key::Char('B') => {
                self.ask("Accept the baseline", ACCEPT_STATEMENT.to_string(), "accept them", Action::AcceptBaseline);
            }
            Key::Char('R') => {
                self.ask("Reset the baseline", RESET_STATEMENT.to_string(), "empty it", Action::ResetBaseline);
            }
            _ => {}
        }
    }

    fn decide_form(&mut self) {
        let Some(finding) = self.find.selected() else {
            return;
        };
        let options: Vec<(&str, &str)> = Status::ALL.iter().map(|status| (status.as_str(), status.label())).collect();
        let fields = vec![
            Field::choice("status", "Decision", finding.status.as_str(), DECISION_HELP, &options),
            Field::text(
                "note",
                "Note",
                &finding.note,
                "Why you decided this. Do not paste the secret value here.",
                findings::MAX_NOTE_LENGTH,
            ),
        ];
        let title = format!("Decide finding {}: {} in {}", finding.id, finding.rule_name, finding.path);
        let submit = Submit::Decide { id: finding.id, revision: finding.revision };
        self.form = Some((Form::new(title, fields), submit));
    }

    pub(crate) fn submit_decide(&mut self, form: &Form, id: i64, revision: i64) -> Result<()> {
        let status = Status::parse(&form.value("status")).unwrap_or(Status::Open);
        let decided = self.app.findings().decide(id, revision, status, &form.value("note"));
        match decided {
            Ok(decided) => {
                self.changed(format!("Finding {} is {}", decided.id, decided.status.label().to_lowercase()));
                Ok(())
            }
            Err(AppError::Conflict(_)) => {
                self.reload();
                Err(AppError::Conflict(
                    "another window changed this finding while you were deciding. Your text is still here: press Esc, press e again to load the new version, and repeat your change".to_string(),
                ))
            }
            Err(error) => Err(error),
        }
    }

    fn filter_line(&self) -> String {
        let page = &self.find;
        let search = if page.searching {
            page.search.view(24, true).trim_end().to_string()
        } else if page.query.search.is_empty() {
            "none".to_string()
        } else {
            page.query.search.clone()
        };
        let mut text = format!(
            "Search: {search} · Scope: {} · Confidence: {} · Decision: {} · Sort: {} {}",
            page.query.scope.label(),
            page.query.level.map_or("all", Level::label),
            page.query.status.map_or("all", Status::label),
            page.query.sort,
            if page.query.descending { "descending" } else { "ascending" },
        );
        if page.query.scan_id != 0 {
            text.push_str(&format!(" · Scan: {}", page.query.scan_id));
        }
        if !page.query.commit_id.is_empty() {
            let commit = &page.query.commit_id;
            text.push_str(&format!(" · Commit: {}", commit.get(..10).unwrap_or(commit)));
        }
        text
    }

    pub(crate) fn findings_view(&mut self, size: usize, height: usize) -> Vec<Line> {
        if self.find.detail && self.find.selected().is_some() {
            return self.finding_detail(size, height);
        }
        let mut lines = wrapped(&self.filter_line(), size, Look::Muted);
        lines.push(Line::new());
        if self.find.list.is_empty() {
            let message = if self.find.filtered() || self.find.searching {
                "No findings match these filters. Press Esc to clear them."
            } else {
                "No findings yet. Press 2 and then n to scan a directory, or press 1 and then l to load labeled demo data."
            };
            lines.extend(wrapped(message, size, Look::Plain));
            return lines;
        }
        let columns: Vec<(&str, usize, bool)> = [
            ("ID", 4, false),
            ("CONFIDENCE", 10, false),
            ("RULE", 12, true),
            ("PATH", 14, true),
            ("VALUE", 12, false),
            ("STATE", 15, false),
            ("DECISION", 14, false),
        ]
        .into_iter()
        .filter(|(title, _, _)| size >= 108 || *title != "DECISION")
        .filter(|(title, _, _)| size >= 84 || *title != "VALUE")
        .collect();
        let rows: Vec<Vec<String>> = self
            .find
            .list
            .iter()
            .map(|finding| {
                columns
                    .iter()
                    .map(|(title, _, _)| match *title {
                        "ID" => finding.id.to_string(),
                        "CONFIDENCE" => confidence_text(finding),
                        "RULE" => finding.rule_name.clone(),
                        "PATH" => finding.path.clone(),
                        "VALUE" => finding.preview.clone(),
                        "STATE" => finding.state_label().to_string(),
                        _ => finding.status.label().to_string(),
                    })
                    .collect()
            })
            .collect();
        let room = height.saturating_sub(lines.len() + 3);
        lines.extend(table(&columns, &rows, self.find.cursor, room, size));
        if self.find.total > self.find.list.len() as i64 {
            lines.push(Line::styled(
                format!("The first {} of {} findings are shown. Narrow the filters to see the others.", rows.len(), self.find.total),
                Look::Muted,
            ));
        }
        lines
    }

    fn finding_detail(&mut self, size: usize, height: usize) -> Vec<Line> {
        let Some(finding) = self.find.selected() else {
            return Vec::new();
        };
        let field = |label: &str, value: String| Line::styled(pad(label, 18), Look::Muted).with(value, Look::Plain);
        let demo = if finding.demo { " (demo data)" } else { "" };
        let mut body = vec![
            Line::plain("Severity ")
                .with(format!("{} {}", finding.severity.glyph(), finding.severity.label()), Look::Level(finding.severity))
                .with(
                    format!(
                        " · Confidence {} · State {} · Decision {}",
                        confidence_text(finding),
                        finding.state_label(),
                        finding.status.label()
                    ),
                    Look::Plain,
                ),
            Line::new(),
            field("Value", format!("{} ({} characters, never stored)", finding.preview, finding.length)),
            field("Path", finding.path.clone()),
            field("Scanned directory", format!("{}{demo}", finding.root)),
            field("Found in", found_in(finding).to_string()),
            field("In the baseline", if finding.baseline { "yes" } else { "no" }.to_string()),
            field("Allowlisted", allowance(finding)),
            field("First seen", self.app.format_time(finding.first_seen_at)),
            field("Last seen", self.app.format_time(finding.last_seen_at)),
        ];
        let note = if finding.note.is_empty() { "No note." } else { finding.note.as_str() };
        body.extend(wrapped(&format!("Note: {note}"), size, Look::Plain));
        body.push(Line::new());
        body.push(Line::styled("Why this confidence", Look::Heading));
        for reason in &finding.reasons {
            body.extend(wrapped(&format!("{:+4}  {}", reason.effect, reason.text), size, Look::Plain));
        }
        body.push(Line::new());
        body.push(Line::styled("Where it was seen", Look::Heading));
        if self.find.places.is_empty() {
            body.extend(wrapped("The last scan did not find it any more.", size, Look::Muted));
        }
        for place in &self.find.places {
            let source = if place.commit_id.is_empty() {
                "files".to_string()
            } else {
                let moment = place.authored_at.map(|moment| self.app.format_time(moment)).unwrap_or_default();
                format!(
                    "commit {} {moment} {} {}",
                    place.commit_id.get(..10).unwrap_or(&place.commit_id),
                    place.author,
                    place.subject
                )
            };
            body.extend(wrapped(
                &format!("Line {}, column {} · {}", place.line, place.column, source.trim_end()),
                size,
                Look::Plain,
            ));
            body.extend(wrapped(&format!("    {}", place.context), size, Look::Muted));
        }
        let title = format!("Finding {} of {}: {} ({})", self.find.cursor + 1, self.find.list.len(), finding.rule_name, finding.rule_id);
        let room = height.saturating_sub(3).max(1);
        let scroll = self.find.scroll.min(body.len().saturating_sub(room));
        let mut lines = vec![Line::styled(pad(&title, size), Look::Heading), Line::new()];
        lines.extend(body.into_iter().skip(scroll).take(room));
        self.find.scroll = scroll;
        lines
    }
}
