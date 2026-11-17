use app_core::allow::{self, EntrySpec, Kind};
use app_core::allowlists::StoredEntry;
use app_core::entropy;
use app_core::error::{AppError, Result};
use app_core::rulebook::{self, StoredRule};
use app_core::rules::{self, RuleSpec};
use app_core::theme::Severity;
use app_core::words;

use crate::model::{Action, Model, Notice, Preview, Submit, borrowed, move_cursor, severity_options};
use crate::text::{Line, Look, pad, wrapped};
use crate::widgets::{Field, Form, Key, table};

const NUMBER_LIMIT: usize = 4;
const KEYWORDS_LIMIT: usize = 700;
const ID_HELP: &str =
    "1 to 60 lower-case letters, digits and hyphens, for example internal-api-token. It cannot change later.";
const PATTERN_HELP: &str = "Regular expression that matches the secret. Look-around and back-references are not supported, so a pattern can never run for a long time.";
const GROUP_HELP: &str = "Number of the capture group that holds the secret value. 0 means the whole match.";
const KEYWORDS_HELP: &str =
    "Comma-separated words. A file is only checked when it contains one of them. Leave empty to check every file.";
const ENTROPY_HELP: &str = "Lowest entropy of the value in bits per character, for example 3.5. 0 turns the check off.";
const CONFIDENCE_HELP: &str = "Starting confidence from 1 to 100 before the context of the match raises or lowers it.";
const SAMPLE_HELP: &str = "Text to try the rule on. It is not stored. The result shows redacted previews only.";
const KIND_HELP: &str = "Path pattern: a glob such as tests/** or *.example. Value pattern: a regular expression for the value. Fingerprint: one exact finding.";
const ENTRY_RULE_HELP: &str = "Identifier of the only rule this entry applies to. Leave empty for every rule.";
const NOTE_HELP: &str = "Why this is safe to leave out. Do not paste a secret value here.";

#[derive(Default)]
pub(crate) struct RulesPage {
    pub list: Vec<StoredRule>,
    pub cursor: usize,
    pub detail: bool,
}

impl RulesPage {
    fn selected(&self) -> Option<&StoredRule> {
        self.list.get(self.cursor)
    }
}

#[derive(Default)]
pub(crate) struct AllowPage {
    pub list: Vec<StoredEntry>,
    pub cursor: usize,
}

fn origin(rule: &StoredRule) -> &'static str {
    match (rule.spec.builtin, rule.changed) {
        (true, false) => "Built-in",
        (true, true) => "Built-in, changed",
        (false, _) => "Custom",
    }
}

fn entropy_label(tenths: u32) -> String {
    if tenths == 0 { "none".to_string() } else { entropy::show(tenths) }
}

fn on_off(enabled: bool) -> &'static str {
    if enabled { "✔ on" } else { "○ off" }
}

fn number<T: std::str::FromStr>(form: &Form, key: &str, field: &str, fix: &str) -> Result<T> {
    let raw = form.value(key);
    raw.trim().parse().map_err(|_| AppError::invalid(field, format!("\"{}\" is not a whole number", raw.trim()), fix))
}

impl Model<'_> {
    pub(crate) fn load_rules(&mut self) {
        let previous = self.rules.selected().map(|rule| rule.spec.id.clone());
        let list = self.app.rulebook().list();
        let Some(list) = self.fail(list) else {
            return;
        };
        self.rules.list = list;
        self.rules.cursor = self.rules.cursor.min(self.rules.list.len().saturating_sub(1));
        if let Some(index) = previous.and_then(|id| self.rules.list.iter().position(|rule| rule.spec.id == id)) {
            self.rules.cursor = index;
        }
    }

    fn rule_form(&mut self, existing: Option<&StoredRule>) {
        let spec = existing.map(|rule| rule.spec.clone()).unwrap_or_default();
        let severities = severity_options();
        let mut fields = Vec::new();
        if existing.is_none() {
            fields.push(Field::text("id", "Identifier", "", ID_HELP, rules::MAX_ID_LENGTH));
        }
        fields.extend([
            Field::text("name", "Name", &spec.name, "Shown in lists and reports.", rules::MAX_NAME_LENGTH),
            Field::text("pattern", "Pattern", &spec.pattern, PATTERN_HELP, rules::MAX_PATTERN_LENGTH),
            Field::text("group", "Secret group", &spec.secret_group.to_string(), GROUP_HELP, NUMBER_LIMIT),
            Field::text("keywords", "Keywords", &spec.keywords.join(", "), KEYWORDS_HELP, KEYWORDS_LIMIT),
            Field::text("entropy", "Lowest entropy", &entropy::show(spec.entropy_tenths), ENTROPY_HELP, NUMBER_LIMIT),
            Field::choice("severity", "Severity", spec.severity.as_str(), "", &borrowed(&severities)),
            Field::text("confidence", "Confidence", &spec.confidence.to_string(), CONFIDENCE_HELP, NUMBER_LIMIT),
            Field::text(
                "description",
                "Description",
                &spec.description,
                "What the rule finds and what to do about it.",
                rules::MAX_DESCRIPTION_LENGTH,
            ),
        ]);
        let (title, submit) = match existing {
            Some(rule) => (
                format!("Edit rule {}", rule.spec.id),
                Submit::EditRule { id: rule.spec.id.clone(), revision: rule.revision },
            ),
            None => ("Add a rule".to_string(), Submit::AddRule),
        };
        self.form = Some((Form::new(title, fields), submit));
    }

    pub(crate) fn submit_rule_spec(&mut self, form: &Form, existing: Option<(&str, i64)>) -> Result<()> {
        let mut spec = match existing {
            Some((id, _)) => self.app.rulebook().get(id)?.spec,
            None => RuleSpec { id: form.value("id").trim().to_string(), ..RuleSpec::default() },
        };
        spec.name = form.value("name");
        spec.pattern = form.value("pattern");
        spec.description = form.value("description");
        spec.secret_group =
            number(form, "group", "secret group", "Enter 0 for the whole match, or the number of a capture group")?;
        spec.keywords = rules::parse_keywords(&form.value("keywords"));
        spec.entropy_tenths = entropy::parse(&form.value("entropy")).ok_or_else(|| {
            AppError::invalid(
                "entropy",
                format!("\"{}\" is not an entropy threshold", form.value("entropy").trim()),
                "Enter a number from 0 to 8 with one decimal, for example 3.5",
            )
        })?;
        spec.severity = Severity::parse(&form.value("severity")).unwrap_or(spec.severity);
        spec.confidence = number(form, "confidence", "confidence", "Enter a number from 1 to 100")?;
        let saved = match existing {
            Some((id, revision)) => self.app.rulebook().update(id, revision, &spec),
            None => self.app.rulebook().create(&spec),
        };
        match saved {
            Ok(rule) => {
                let verb = if existing.is_some() { "Saved" } else { "Added" };
                self.changed(format!("{verb} rule {}. It applies from the next scan.", rule.spec.id));
                Ok(())
            }
            Err(AppError::Conflict(_)) if existing.is_some() => {
                self.reload();
                Err(AppError::Conflict(
                    "another window changed this rule while you were editing. Your text is still here: press Esc, press e again to load the new version, and repeat your change".to_string(),
                ))
            }
            Err(error) => Err(error),
        }
    }

    pub(crate) fn submit_try(&mut self, form: &Form, id: &str) -> Result<()> {
        let rule = self.app.rulebook().get(id)?;
        let hits = rulebook::try_rule(&rule.spec, &form.value("sample"))?;
        let mut lines = vec![format!(
            "Rule {} reports {} in the sample.",
            rule.spec.id,
            words::count(hits.len() as i64, "match", "matches")
        )];
        for hit in &hits {
            lines.push(String::new());
            lines.push(format!(
                "Line {}, column {} · {} ({} characters) · Entropy {} · Confidence {}",
                hit.line,
                hit.column,
                hit.preview,
                hit.length,
                entropy::show(hit.entropy_tenths),
                hit.score
            ));
            lines.push(format!("    {}", hit.context));
        }
        self.preview = Some(Preview {
            title: format!("Try rule {}", rule.spec.id),
            lines,
            offset: 0,
            accept: String::new(),
            action: None,
        });
        Ok(())
    }

    pub(crate) fn rules_key(&mut self, key: Key) {
        let count = self.rules.list.len();
        if move_cursor(&mut self.rules.cursor, key, count) {
            return;
        }
        let selected = self.rules.selected().cloned();
        match (key, selected) {
            (Key::Home | Key::Char('g'), _) => self.rules.cursor = 0,
            (Key::End | Key::Char('G'), _) => self.rules.cursor = count.saturating_sub(1),
            (Key::Esc, _) => self.rules.detail = false,
            (Key::Char('n'), _) => self.rule_form(None),
            (Key::Enter, Some(_)) => self.rules.detail = !self.rules.detail,
            (Key::Char('e'), Some(rule)) => self.rule_form(Some(&rule)),
            (Key::Char('x'), Some(rule)) => {
                let saved = self.app.rulebook().set_enabled(&rule.spec.id, !rule.spec.enabled);
                if let Some(saved) = self.fail(saved) {
                    let state = if saved.spec.enabled { "on" } else { "off" };
                    self.changed(format!("Rule {} is {state}. It applies from the next scan.", saved.spec.id));
                }
            }
            (Key::Char('t'), Some(rule)) => {
                let fields = vec![Field::text("sample", "Sample text", "", SAMPLE_HELP, rulebook::MAX_SAMPLE_LENGTH)];
                let title = format!("Try rule {} on a sample", rule.spec.id);
                self.form = Some((Form::new(title, fields), Submit::TryRule(rule.spec.id)));
            }
            (Key::Char('u'), Some(rule)) => {
                if !rule.spec.builtin {
                    self.say(Notice::Info, "Only a built-in rule can be restored. Press d to remove a custom rule.");
                    return;
                }
                let saved = self.app.rulebook().restore(&rule.spec.id);
                if let Some(saved) = self.fail(saved) {
                    self.changed(format!("Rule {} is back to its built-in definition", saved.spec.id));
                }
            }
            (Key::Char('d'), Some(rule)) => {
                if rule.spec.builtin {
                    self.say(Notice::Info, "A built-in rule cannot be removed. Press x to turn it off.");
                    return;
                }
                self.ask(
                    "Remove the rule",
                    format!("This removes the rule {} ({}). Its findings stay.", rule.spec.id, rule.spec.name),
                    "remove it",
                    Action::RemoveRule(rule.spec.id),
                );
            }
            _ => {}
        }
    }

    pub(crate) fn rules_view(&mut self, size: usize, height: usize) -> Vec<Line> {
        let mut lines = Vec::new();
        let columns: Vec<(&str, usize, bool)> = [
            ("STATE", 5, false),
            ("IDENTIFIER", 22, true),
            ("NAME", 18, true),
            ("SEVERITY", 10, false),
            ("ENTROPY", 7, false),
            ("ORIGIN", 17, false),
        ]
        .into_iter()
        .filter(|(title, _, _)| size >= 96 || *title != "NAME")
        .filter(|(title, _, _)| size >= 70 || *title != "ENTROPY")
        .collect();
        let rows: Vec<Vec<String>> = self
            .rules
            .list
            .iter()
            .map(|rule| {
                columns
                    .iter()
                    .map(|(title, _, _)| match *title {
                        "STATE" => on_off(rule.spec.enabled).to_string(),
                        "IDENTIFIER" => rule.spec.id.clone(),
                        "NAME" => rule.spec.name.clone(),
                        "SEVERITY" => format!("{} {}", rule.spec.severity.glyph(), rule.spec.severity.label()),
                        "ENTROPY" => entropy_label(rule.spec.entropy_tenths),
                        _ => origin(rule).to_string(),
                    })
                    .collect()
            })
            .collect();
        let selected = self.rules.selected().filter(|_| self.rules.detail);
        let mut detail = Vec::new();
        if let Some(rule) = selected {
            let spec = &rule.spec;
            let field = |label: &str, value: String| Line::styled(pad(label, 16), Look::Muted).with(value, Look::Plain);
            detail.push(Line::styled("─".repeat(size), Look::Border));
            detail.push(Line::styled(pad(&format!("{} ({})", spec.name, spec.id), size), Look::Heading));
            detail.push(field("Pattern", spec.pattern.clone()));
            detail.push(field("Secret group", spec.secret_group.to_string()));
            let keywords = if spec.keywords.is_empty() { "none".to_string() } else { spec.keywords.join(", ") };
            detail.push(field("Keywords", keywords));
            detail.push(field(
                "Checks",
                format!(
                    "Lowest entropy {} · Starting confidence {} · {}",
                    entropy_label(spec.entropy_tenths),
                    spec.confidence,
                    origin(rule)
                ),
            ));
            detail.extend(wrapped(&spec.description, size, Look::Muted).into_iter().take(3));
        }
        let room = height.saturating_sub(detail.len() + 2);
        lines.extend(table(&columns, &rows, self.rules.cursor, room, size));
        lines.extend(detail);
        lines
    }

    pub(crate) fn load_allowlists(&mut self) {
        let previous = self.allow.list.get(self.allow.cursor).map(|entry| entry.spec.id);
        let list = self.app.allowlists().list();
        let Some(list) = self.fail(list) else {
            return;
        };
        self.allow.list = list;
        self.allow.cursor = self.allow.cursor.min(self.allow.list.len().saturating_sub(1));
        if let Some(index) = previous.and_then(|id| self.allow.list.iter().position(|entry| entry.spec.id == id)) {
            self.allow.cursor = index;
        }
    }

    fn entry_form(&mut self, existing: Option<&StoredEntry>) {
        let spec = existing.map(|entry| entry.spec.clone()).unwrap_or_default();
        let kinds: Vec<(&str, &str)> = Kind::ALL.iter().map(|kind| (kind.as_str(), kind.label())).collect();
        let fields = vec![
            Field::choice("kind", "Kind", spec.kind.as_str(), KIND_HELP, &kinds),
            Field::text(
                "pattern",
                "Pattern",
                &spec.pattern,
                "The glob, regular expression or fingerprint.",
                rules::MAX_PATTERN_LENGTH,
            ),
            Field::text("rule", "Only for rule", &spec.rule_id, ENTRY_RULE_HELP, rules::MAX_ID_LENGTH),
            Field::text("note", "Note", &spec.note, NOTE_HELP, allow::MAX_NOTE_LENGTH),
        ];
        let (title, submit) = match existing {
            Some(entry) => (
                format!("Edit allowlist entry {}", entry.spec.id),
                Submit::EditEntry { id: entry.spec.id, revision: entry.revision },
            ),
            None => ("Add an allowlist entry".to_string(), Submit::AddEntry),
        };
        self.form = Some((Form::new(title, fields), submit));
    }

    pub(crate) fn submit_entry(&mut self, form: &Form, existing: Option<(i64, i64)>) -> Result<()> {
        let mut spec = match existing {
            Some((id, _)) => self.app.allowlists().get(id)?.spec,
            None => EntrySpec::default(),
        };
        spec.kind = Kind::parse(&form.value("kind")).unwrap_or(spec.kind);
        spec.pattern = form.value("pattern");
        spec.rule_id = form.value("rule").trim().to_string();
        spec.note = form.value("note");
        let saved = match existing {
            Some((id, revision)) => self.app.allowlists().update(id, revision, &spec),
            None => self.app.allowlists().create(&spec),
        };
        match saved {
            Ok(entry) => {
                let verb = if existing.is_some() { "Saved" } else { "Added" };
                self.changed(format!("{verb} allowlist entry {}. It applies from the next scan.", entry.spec.id));
                Ok(())
            }
            Err(AppError::Conflict(_)) if existing.is_some() => {
                self.reload();
                Err(AppError::Conflict(
                    "another window changed this entry while you were editing. Your text is still here: press Esc, press e again to load the new version, and repeat your change".to_string(),
                ))
            }
            Err(error) => Err(error),
        }
    }

    pub(crate) fn allowlists_key(&mut self, key: Key) {
        let count = self.allow.list.len();
        if move_cursor(&mut self.allow.cursor, key, count) {
            return;
        }
        let selected = self.allow.list.get(self.allow.cursor).cloned();
        match (key, selected) {
            (Key::Home | Key::Char('g'), _) => self.allow.cursor = 0,
            (Key::End | Key::Char('G'), _) => self.allow.cursor = count.saturating_sub(1),
            (Key::Char('n'), _) => self.entry_form(None),
            (Key::Char('e'), Some(entry)) => self.entry_form(Some(&entry)),
            (Key::Char('x'), Some(entry)) => {
                let saved = self.app.allowlists().set_enabled(entry.spec.id, !entry.spec.enabled);
                if let Some(saved) = self.fail(saved) {
                    let state = if saved.spec.enabled { "on" } else { "off" };
                    self.changed(format!(
                        "Allowlist entry {} is {state}. It applies from the next scan.",
                        saved.spec.id
                    ));
                }
            }
            (Key::Char('d'), Some(entry)) => self.ask(
                "Remove the allowlist entry",
                format!(
                    "This removes the {} \"{}\". Findings it left out need attention again after the next scan.",
                    entry.spec.kind.label().to_lowercase(),
                    entry.spec.pattern
                ),
                "remove it",
                Action::RemoveEntry(entry.spec.id),
            ),
            _ => {}
        }
    }

    pub(crate) fn allowlists_view(&mut self, size: usize, height: usize) -> Vec<Line> {
        let mut lines = wrapped(
            &format!(
                "An allowlist entry leaves matching findings out of what needs attention. A line that contains {} is left out too.",
                allow::LINE_MARKER
            ),
            size,
            Look::Muted,
        );
        lines.push(Line::new());
        if self.allow.list.is_empty() {
            lines.extend(wrapped("No allowlist entries yet. Press n to add one.", size, Look::Plain));
            return lines;
        }
        let columns: Vec<(&str, usize, bool)> = [
            ("ID", 4, false),
            ("STATE", 5, false),
            ("KIND", 13, false),
            ("PATTERN", 16, true),
            ("ONLY FOR RULE", 16, false),
            ("NOTE", 14, true),
        ]
        .into_iter()
        .filter(|(title, _, _)| size >= 100 || *title != "NOTE")
        .filter(|(title, _, _)| size >= 72 || *title != "ONLY FOR RULE")
        .collect();
        let rows: Vec<Vec<String>> = self
            .allow
            .list
            .iter()
            .map(|entry| {
                columns
                    .iter()
                    .map(|(title, _, _)| match *title {
                        "ID" => entry.spec.id.to_string(),
                        "STATE" => on_off(entry.spec.enabled).to_string(),
                        "KIND" => entry.spec.kind.label().to_string(),
                        "PATTERN" => entry.spec.pattern.clone(),
                        "ONLY FOR RULE" => {
                            if entry.spec.rule_id.is_empty() {
                                "every rule".to_string()
                            } else {
                                entry.spec.rule_id.clone()
                            }
                        }
                        _ => {
                            let demo = if entry.demo { "(demo data) " } else { "" };
                            format!("{demo}{}", entry.spec.note)
                        }
                    })
                    .collect()
            })
            .collect();
        let room = height.saturating_sub(lines.len() + 2);
        lines.extend(table(&columns, &rows, self.allow.cursor, room, size));
        lines
    }
}
