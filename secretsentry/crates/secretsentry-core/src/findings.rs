use std::collections::BTreeMap;

use rusqlite::{Connection, OptionalExtension, Row, Transaction, TransactionBehavior};

use crate::clock::{Clock, Timestamp};
use crate::confidence::{self, Level, Reason};
use crate::error::{AppError, Result};
use crate::scanner::{Allowed, Candidate};
use crate::theme::Severity;

pub const KIND: &str = "finding";
pub const MAX_NOTE_LENGTH: usize = 2000;
pub const MAX_OCCURRENCES_PER_FINDING: usize = 50;
pub const DEFAULT_LIMIT: i64 = 200;
pub const MAX_LIMIT: i64 = 5000;
pub const TOP_FILES: i64 = 10;
pub const SORT_KEYS: &[&str] = &["score", "rule", "path", "status", "seen"];

const SQL_COLUMNS_GET: &str = "SELECT id, root, fingerprint, value_digest, rule_id, rule_name, severity, path, preview, value_length, entropy_tenths, score, reasons, status, note, allow_kind, allow_entry_id, baseline, in_tree, in_history, is_demo, revision, last_scan_id, first_seen_at, last_seen_at, updated_at, 1 FROM findings WHERE id = ?1";
const SQL_LIST: &str = "SELECT id, root, fingerprint, value_digest, rule_id, rule_name, severity, path, preview, value_length, entropy_tenths, score, reasons, status, note, allow_kind, allow_entry_id, baseline, in_tree, in_history, is_demo, revision, last_scan_id, first_seen_at, last_seen_at, updated_at, count(*) OVER ()
FROM findings
WHERE (?1 = '' OR instr(lower(path), ?1) > 0 OR instr(lower(rule_name), ?1) > 0 OR instr(lower(note), ?1) > 0 OR instr(rule_id, ?1) > 0)
  AND (?2 = '' OR rule_id = ?2)
  AND (?3 = '' OR confidence = ?3)
  AND (?4 = '' OR status = ?4)
  AND (?5 = '' OR root = ?5)
  AND (?6 = '' OR path = ?6)
  AND score >= ?12
  AND (?7 = 'all'
    OR (?7 = 'attention' AND id IN (SELECT id FROM attention_findings))
    OR (?7 = 'new' AND id IN (SELECT id FROM attention_findings WHERE baseline = 0))
    OR (?7 = 'baseline' AND id IN (SELECT id FROM attention_findings WHERE baseline = 1))
    OR (?7 = 'allowlisted' AND allow_kind <> '')
    OR (?7 = 'decided' AND status IN ('false_positive', 'accepted_risk', 'resolved'))
    OR (?7 = 'gone' AND in_tree = 0 AND in_history = 0)
    OR (?7 = 'history' AND in_history = 1))
ORDER BY
  CASE WHEN ?8 = 'score' AND ?9 = 0 THEN score END ASC,
  CASE WHEN ?8 = 'score' AND ?9 = 1 THEN score END DESC,
  CASE WHEN ?8 = 'rule' AND ?9 = 0 THEN rule_name END COLLATE NOCASE ASC,
  CASE WHEN ?8 = 'rule' AND ?9 = 1 THEN rule_name END COLLATE NOCASE DESC,
  CASE WHEN ?8 = 'path' AND ?9 = 0 THEN path END ASC,
  CASE WHEN ?8 = 'path' AND ?9 = 1 THEN path END DESC,
  CASE WHEN ?8 = 'status' AND ?9 = 0 THEN status END ASC,
  CASE WHEN ?8 = 'status' AND ?9 = 1 THEN status END DESC,
  CASE WHEN ?8 = 'seen' AND ?9 = 0 THEN last_seen_at END ASC,
  CASE WHEN ?8 = 'seen' AND ?9 = 1 THEN last_seen_at END DESC,
  path ASC,
  id ASC
LIMIT ?10 OFFSET ?11";
const SQL_FIND: &str = "SELECT id, baseline FROM findings WHERE root = ?1 AND fingerprint = ?2";
const SQL_INSERT: &str = "INSERT INTO findings (root, fingerprint, value_digest, rule_id, rule_name, severity, path, preview, value_length, entropy_tenths, score, confidence, reasons, allow_kind, allow_entry_id, in_tree, in_history, is_demo, last_scan_id, first_seen_at, last_seen_at, updated_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18, ?19, ?20, ?20, ?20)";
const SQL_REFRESH: &str = "UPDATE findings SET rule_name = ?1, severity = ?2, preview = ?3, value_length = ?4, entropy_tenths = ?5, score = ?6, confidence = ?7, reasons = ?8, allow_kind = ?9, allow_entry_id = ?10, in_tree = max(in_tree, ?11), in_history = max(in_history, ?12), last_scan_id = ?13, last_seen_at = ?14, updated_at = ?14 WHERE id = ?15";
const SQL_DROP_TREE_OCCURRENCES: &str = "DELETE FROM occurrences WHERE finding_id = ?1 AND commit_id = ''";
const SQL_DROP_HISTORY_OCCURRENCES: &str = "DELETE FROM occurrences WHERE finding_id = ?1 AND commit_id <> ''";
const SQL_ADD_OCCURRENCE: &str =
    "INSERT INTO occurrences (finding_id, scan_id, line, col, context, commit_id) VALUES (?1, ?2, ?3, ?4, ?5, ?6)";
const SQL_OCCURRENCES: &str = "SELECT id, finding_id, scan_id, line, col, context, commit_id FROM occurrences WHERE finding_id = ?1 ORDER BY commit_id, line, col, id";
const SQL_GONE_FROM_TREE: &str = "UPDATE findings SET in_tree = 0, updated_at = ?3 WHERE root = ?1 AND in_tree = 1 AND (last_scan_id IS NULL OR last_scan_id <> ?2)";
const SQL_GONE_FROM_HISTORY: &str = "UPDATE findings SET in_history = 0, updated_at = ?3 WHERE root = ?1 AND in_history = 1 AND (last_scan_id IS NULL OR last_scan_id <> ?2)";
const SQL_DECIDE: &str = "UPDATE findings SET status = ?1, note = ?2, revision = revision + 1, updated_at = ?3 WHERE id = ?4 AND revision = ?5";
const SQL_ACCEPT_BASELINE: &str = "UPDATE findings SET baseline = 1, revision = revision + 1, updated_at = ?1 WHERE baseline = 0 AND (?2 = '' OR root = ?2)";
const SQL_RESET_BASELINE: &str = "UPDATE findings SET baseline = 0, revision = revision + 1, updated_at = ?1 WHERE baseline = 1 AND (?2 = '' OR root = ?2)";
const SQL_OVERVIEW: &str = "SELECT attention_total, new_total, baseline_total, allowlisted_total, decided_total, gone_total, demo_total, scan_total, failed_scan_total, unread_notifications FROM overview_scanning";
const SQL_BY_RULE: &str =
    "SELECT rule_id, rule_name, confidence, total FROM overview_findings_by_rule ORDER BY rule_name, rule_id";
const SQL_TOP_FILES: &str =
    "SELECT root, path, total, top_score FROM overview_findings_by_file ORDER BY total DESC, top_score DESC, path, root LIMIT ?1";
const SQL_ROOTS: &str = "SELECT DISTINCT root FROM findings ORDER BY root";
const SQL_COUNT: &str = "SELECT count(*) FROM findings";

pub const HOT_QUERIES: &[(&str, &str, usize)] = &[
    ("finding by identifier", SQL_COLUMNS_GET, 1),
    ("finding by root and fingerprint", SQL_FIND, 2),
    ("occurrences of a finding", SQL_OCCURRENCES, 1),
];

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Status {
    Open,
    Confirmed,
    FalsePositive,
    AcceptedRisk,
    Resolved,
}

impl Status {
    pub const ALL: [Status; 5] =
        [Status::Open, Status::Confirmed, Status::FalsePositive, Status::AcceptedRisk, Status::Resolved];

    pub fn parse(text: &str) -> Option<Status> {
        let wanted = text.trim().to_lowercase().replace(['-', ' '], "_");
        Status::ALL.into_iter().find(|status| status.as_str() == wanted)
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Status::Open => "open",
            Status::Confirmed => "confirmed",
            Status::FalsePositive => "false_positive",
            Status::AcceptedRisk => "accepted_risk",
            Status::Resolved => "resolved",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Status::Open => "Open",
            Status::Confirmed => "Confirmed",
            Status::FalsePositive => "False positive",
            Status::AcceptedRisk => "Accepted risk",
            Status::Resolved => "Resolved",
        }
    }

    pub fn needs_attention(self) -> bool {
        matches!(self, Status::Open | Status::Confirmed)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum Scope {
    #[default]
    All,
    Attention,
    New,
    Baseline,
    Allowlisted,
    Decided,
    Gone,
    History,
}

impl Scope {
    pub const ALL: [Scope; 8] = [
        Scope::All,
        Scope::Attention,
        Scope::New,
        Scope::Baseline,
        Scope::Allowlisted,
        Scope::Decided,
        Scope::Gone,
        Scope::History,
    ];

    pub fn parse(text: &str) -> Option<Scope> {
        let wanted = text.trim().to_lowercase();
        Scope::ALL.into_iter().find(|scope| scope.as_str() == wanted)
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Scope::All => "all",
            Scope::Attention => "attention",
            Scope::New => "new",
            Scope::Baseline => "baseline",
            Scope::Allowlisted => "allowlisted",
            Scope::Decided => "decided",
            Scope::Gone => "gone",
            Scope::History => "history",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Scope::All => "All findings",
            Scope::Attention => "Needs attention",
            Scope::New => "New",
            Scope::Baseline => "In the baseline",
            Scope::Allowlisted => "Allowlisted",
            Scope::Decided => "Decided",
            Scope::Gone => "No longer found",
            Scope::History => "Found in Git history",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Source {
    Tree,
    History,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Finding {
    pub id: i64,
    pub root: String,
    pub fingerprint: String,
    pub value_digest: String,
    pub rule_id: String,
    pub rule_name: String,
    pub severity: Severity,
    pub path: String,
    pub preview: String,
    pub length: i64,
    pub entropy_tenths: u32,
    pub score: u8,
    pub reasons: Vec<Reason>,
    pub status: Status,
    pub note: String,
    pub allowed: Allowed,
    pub baseline: bool,
    pub in_tree: bool,
    pub in_history: bool,
    pub demo: bool,
    pub revision: i64,
    pub last_scan_id: Option<i64>,
    pub first_seen_at: Timestamp,
    pub last_seen_at: Timestamp,
    pub updated_at: Timestamp,
}

impl Finding {
    pub fn level(&self) -> Level {
        Level::of(self.score)
    }

    pub fn needs_attention(&self) -> bool {
        self.allowed == Allowed::No && self.status.needs_attention() && (self.in_tree || self.in_history)
    }

    pub fn state_label(&self) -> &'static str {
        if self.allowed != Allowed::No {
            "Allowlisted"
        } else if !self.in_tree && !self.in_history {
            "No longer found"
        } else if !self.status.needs_attention() {
            "Decided"
        } else if self.baseline {
            "Baseline"
        } else {
            "New"
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Occurrence {
    pub id: i64,
    pub finding_id: i64,
    pub scan_id: i64,
    pub line: i64,
    pub column: i64,
    pub context: String,
    pub commit_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Query {
    pub search: String,
    pub rule_id: String,
    pub level: Option<Level>,
    pub status: Option<Status>,
    pub scope: Scope,
    pub root: String,
    pub path: String,
    pub min_score: u8,
    pub sort: String,
    pub descending: bool,
    pub limit: i64,
    pub offset: i64,
}

impl Default for Query {
    fn default() -> Self {
        Query {
            search: String::new(),
            rule_id: String::new(),
            level: None,
            status: None,
            scope: Scope::All,
            root: String::new(),
            path: String::new(),
            min_score: 0,
            sort: "score".to_string(),
            descending: true,
            limit: DEFAULT_LIMIT,
            offset: 0,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Page {
    pub findings: Vec<Finding>,
    pub total: i64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuleCount {
    pub rule_id: String,
    pub rule_name: String,
    pub low: i64,
    pub medium: i64,
    pub high: i64,
}

impl RuleCount {
    pub fn total(&self) -> i64 {
        self.low + self.medium + self.high
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileCount {
    pub root: String,
    pub path: String,
    pub total: i64,
    pub top_score: u8,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Overview {
    pub attention: i64,
    pub new: i64,
    pub baseline: i64,
    pub allowlisted: i64,
    pub decided: i64,
    pub gone: i64,
    pub demo: i64,
    pub scans: i64,
    pub failed_scans: i64,
    pub unread_notifications: i64,
    pub by_rule: Vec<RuleCount>,
    pub top_files: Vec<FileCount>,
}

impl Overview {
    pub fn by_level(&self, level: Level) -> i64 {
        self.by_rule
            .iter()
            .map(|rule| match level {
                Level::Low => rule.low,
                Level::Medium => rule.medium,
                Level::High => rule.high,
            })
            .sum()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Recorded {
    pub seen: usize,
    pub new: usize,
    pub top_new_severity: Option<Severity>,
}

pub struct Sighting<'a> {
    pub candidate: &'a Candidate,
    pub commit_id: &'a str,
}

pub struct RuleLabel {
    pub name: String,
    pub severity: Severity,
}

struct RawFinding {
    id: i64,
    root: String,
    fingerprint: String,
    value_digest: String,
    rule_id: String,
    rule_name: String,
    severity: String,
    path: String,
    preview: String,
    length: i64,
    entropy_tenths: i64,
    score: i64,
    reasons: String,
    status: String,
    note: String,
    allow_kind: String,
    allow_entry_id: i64,
    baseline: i64,
    in_tree: i64,
    in_history: i64,
    demo: i64,
    revision: i64,
    last_scan_id: Option<i64>,
    first_seen_at: String,
    last_seen_at: String,
    updated_at: String,
    total: i64,
}

fn scan(row: &Row<'_>) -> rusqlite::Result<RawFinding> {
    Ok(RawFinding {
        id: row.get(0)?,
        root: row.get(1)?,
        fingerprint: row.get(2)?,
        value_digest: row.get(3)?,
        rule_id: row.get(4)?,
        rule_name: row.get(5)?,
        severity: row.get(6)?,
        path: row.get(7)?,
        preview: row.get(8)?,
        length: row.get(9)?,
        entropy_tenths: row.get(10)?,
        score: row.get(11)?,
        reasons: row.get(12)?,
        status: row.get(13)?,
        note: row.get(14)?,
        allow_kind: row.get(15)?,
        allow_entry_id: row.get(16)?,
        baseline: row.get(17)?,
        in_tree: row.get(18)?,
        in_history: row.get(19)?,
        demo: row.get(20)?,
        revision: row.get(21)?,
        last_scan_id: row.get(22)?,
        first_seen_at: row.get(23)?,
        last_seen_at: row.get(24)?,
        updated_at: row.get(25)?,
        total: row.get(26)?,
    })
}

fn allowed_from(kind: &str, entry: i64) -> Option<Allowed> {
    match kind {
        "" => Some(Allowed::No),
        "marker" => Some(Allowed::Marker),
        "entry" => Some(Allowed::Entry(entry)),
        _ => None,
    }
}

fn allowed_parts(allowed: Allowed) -> (&'static str, i64) {
    match allowed {
        Allowed::No => ("", 0),
        Allowed::Marker => ("marker", 0),
        Allowed::Entry(id) => ("entry", id),
    }
}

fn build(raw: RawFinding) -> Result<Finding> {
    let id = raw.id;
    let unreadable = |what: &str| AppError::Storage(format!("finding {id} has an unreadable {what}"));
    Ok(Finding {
        id,
        root: raw.root,
        fingerprint: raw.fingerprint,
        value_digest: raw.value_digest,
        rule_id: raw.rule_id,
        rule_name: raw.rule_name,
        severity: Severity::parse(&raw.severity).ok_or_else(|| unreadable("severity"))?,
        path: raw.path,
        preview: raw.preview,
        length: raw.length,
        entropy_tenths: u32::try_from(raw.entropy_tenths).map_err(|_| unreadable("entropy"))?,
        score: u8::try_from(raw.score).map_err(|_| unreadable("score"))?,
        reasons: confidence::reasons_from_json(&raw.reasons),
        status: Status::parse(&raw.status).ok_or_else(|| unreadable("status"))?,
        note: raw.note,
        allowed: allowed_from(&raw.allow_kind, raw.allow_entry_id).ok_or_else(|| unreadable("allowlist state"))?,
        baseline: raw.baseline == 1,
        in_tree: raw.in_tree == 1,
        in_history: raw.in_history == 1,
        demo: raw.demo == 1,
        revision: raw.revision,
        last_scan_id: raw.last_scan_id,
        first_seen_at: Timestamp::parse(&raw.first_seen_at).ok_or_else(|| unreadable("first time"))?,
        last_seen_at: Timestamp::parse(&raw.last_seen_at).ok_or_else(|| unreadable("last time"))?,
        updated_at: Timestamp::parse(&raw.updated_at).ok_or_else(|| unreadable("update time"))?,
    })
}

fn find(conn: &Connection, id: i64) -> Result<Option<Finding>> {
    conn.query_row(SQL_COLUMNS_GET, [id], scan).optional()?.map(build).transpose()
}

fn not_found(id: i64) -> AppError {
    AppError::NotFound(format!("finding {id} (list the findings to see valid identifiers)"))
}

fn combined_allowance(group: &[&Sighting<'_>]) -> Allowed {
    if let Some(entry) = group.iter().find_map(|sighting| match sighting.candidate.allowed {
        Allowed::Entry(id) => Some(Allowed::Entry(id)),
        _ => None,
    }) {
        return entry;
    }
    if group.iter().all(|sighting| sighting.candidate.allowed == Allowed::Marker) {
        Allowed::Marker
    } else {
        Allowed::No
    }
}

pub struct Recording<'a> {
    pub scan_id: i64,
    pub root: &'a str,
    pub source: Source,
    pub demo: bool,
    pub complete: bool,
    pub now: Timestamp,
}

pub fn record(
    conn: &Connection,
    recording: &Recording<'_>,
    rules: &BTreeMap<String, RuleLabel>,
    sightings: &[Sighting<'_>],
) -> Result<Recorded> {
    let mut groups: BTreeMap<&str, Vec<&Sighting<'_>>> = BTreeMap::new();
    for sighting in sightings {
        groups.entry(sighting.candidate.fingerprint.as_str()).or_default().push(sighting);
    }
    let now = recording.now.to_rfc3339();
    let (tree, history) = match recording.source {
        Source::Tree => (1i64, 0i64),
        Source::History => (0i64, 1i64),
    };
    let mut recorded = Recorded { seen: groups.len(), ..Recorded::default() };
    for (print, group) in &groups {
        let best = group.iter().max_by_key(|sighting| sighting.candidate.score).map_or(group[0], |best| *best).candidate;
        let label = rules.get(&best.rule_id);
        let name = label.map_or(best.rule_id.as_str(), |label| label.name.as_str());
        let severity = label.map_or(Severity::Medium, |label| label.severity);
        let allowed = combined_allowance(group);
        let (allow_kind, allow_entry) = allowed_parts(allowed);
        let reasons = confidence::reasons_to_json(&best.reasons);
        let level = Level::of(best.score).as_str();
        let existing: Option<(i64, i64)> =
            conn.query_row(SQL_FIND, (recording.root, print), |row| Ok((row.get(0)?, row.get(1)?))).optional()?;
        let id = match existing {
            Some((id, _)) => {
                conn.execute(
                    SQL_REFRESH,
                    (
                        name,
                        severity.as_str(),
                        &best.preview,
                        best.length as i64,
                        i64::from(best.entropy_tenths),
                        i64::from(best.score),
                        level,
                        &reasons,
                        allow_kind,
                        allow_entry,
                        tree,
                        history,
                        recording.scan_id,
                        &now,
                        id,
                    ),
                )?;
                id
            }
            None => {
                conn.execute(
                    SQL_INSERT,
                    rusqlite::params![
                        recording.root,
                        print,
                        best.value_digest,
                        best.rule_id,
                        name,
                        severity.as_str(),
                        best.path,
                        best.preview,
                        best.length as i64,
                        i64::from(best.entropy_tenths),
                        i64::from(best.score),
                        level,
                        reasons,
                        allow_kind,
                        allow_entry,
                        tree,
                        history,
                        i64::from(recording.demo),
                        recording.scan_id,
                        now,
                    ],
                )?;
                if allowed == Allowed::No {
                    recorded.new += 1;
                    recorded.top_new_severity = recorded.top_new_severity.max(Some(severity));
                }
                conn.last_insert_rowid()
            }
        };
        let drop_old = match recording.source {
            Source::Tree => SQL_DROP_TREE_OCCURRENCES,
            Source::History => SQL_DROP_HISTORY_OCCURRENCES,
        };
        conn.execute(drop_old, [id])?;
        for sighting in group.iter().take(MAX_OCCURRENCES_PER_FINDING) {
            conn.execute(
                SQL_ADD_OCCURRENCE,
                (
                    id,
                    recording.scan_id,
                    sighting.candidate.line as i64,
                    sighting.candidate.column as i64,
                    &sighting.candidate.context,
                    sighting.commit_id,
                ),
            )?;
        }
    }
    if recording.complete {
        let gone = match recording.source {
            Source::Tree => SQL_GONE_FROM_TREE,
            Source::History => SQL_GONE_FROM_HISTORY,
        };
        conn.execute(gone, (recording.root, recording.scan_id, &now))?;
    }
    Ok(recorded)
}

pub struct Findings<'a> {
    conn: &'a Connection,
    clock: Clock,
}

impl<'a> Findings<'a> {
    pub fn new(conn: &'a Connection, clock: Clock) -> Self {
        Findings { conn, clock }
    }

    fn begin(&self) -> Result<Transaction<'a>> {
        Ok(Transaction::new_unchecked(self.conn, TransactionBehavior::Immediate)?)
    }

    pub fn get(&self, id: i64) -> Result<Finding> {
        find(self.conn, id)?.ok_or_else(|| not_found(id))
    }

    pub fn count(&self) -> Result<i64> {
        Ok(self.conn.query_row(SQL_COUNT, [], |row| row.get(0))?)
    }

    pub fn page(&self, query: &Query) -> Result<Page> {
        if !SORT_KEYS.contains(&query.sort.as_str()) {
            return Err(AppError::invalid(
                "sort",
                format!("\"{}\" is not a sort key", query.sort),
                format!("Use one of: {}", SORT_KEYS.join(", ")),
            ));
        }
        let mut statement = self.conn.prepare(SQL_LIST)?;
        let rows = statement.query_map(
            rusqlite::params![
                query.search.trim().to_lowercase(),
                query.rule_id.trim(),
                query.level.map_or("", Level::as_str),
                query.status.map_or("", Status::as_str),
                query.root,
                query.path,
                query.scope.as_str(),
                query.sort,
                i64::from(query.descending),
                query.limit.clamp(1, MAX_LIMIT),
                query.offset.max(0),
                i64::from(query.min_score),
            ],
            scan,
        )?;
        let mut page = Page { findings: Vec::new(), total: 0 };
        for row in rows {
            let raw = row?;
            page.total = raw.total;
            page.findings.push(build(raw)?);
        }
        Ok(page)
    }

    pub fn list(&self, query: &Query) -> Result<Vec<Finding>> {
        Ok(self.page(query)?.findings)
    }

    pub fn total(&self, query: &Query) -> Result<i64> {
        Ok(self.page(&Query { limit: 1, offset: 0, ..query.clone() })?.total)
    }

    pub fn occurrences(&self, finding_id: i64) -> Result<Vec<Occurrence>> {
        let mut statement = self.conn.prepare(SQL_OCCURRENCES)?;
        let rows = statement.query_map([finding_id], |row| {
            Ok(Occurrence {
                id: row.get(0)?,
                finding_id: row.get(1)?,
                scan_id: row.get(2)?,
                line: row.get(3)?,
                column: row.get(4)?,
                context: row.get(5)?,
                commit_id: row.get(6)?,
            })
        })?;
        let mut list = Vec::new();
        for row in rows {
            list.push(row?);
        }
        Ok(list)
    }

    pub fn decide(&self, id: i64, revision: i64, status: Status, note: &str) -> Result<Finding> {
        let note = note.trim();
        if note.chars().count() > MAX_NOTE_LENGTH {
            return Err(AppError::invalid(
                "note",
                format!("the note is longer than {MAX_NOTE_LENGTH} characters"),
                "Shorten the note",
            ));
        }
        if note.chars().any(|letter| letter.is_control() && letter != '\n' && letter != '\t') {
            return Err(AppError::invalid("note", "the note contains a control character", "Remove the control character"));
        }
        let transaction = self.begin()?;
        let changed =
            transaction.execute(SQL_DECIDE, (status.as_str(), note, self.clock.now().to_rfc3339(), id, revision))?;
        if changed == 0 {
            return Err(if find(&transaction, id)?.is_some() {
                AppError::Conflict(format!(
                    "finding {id} was changed by another window: reload it and apply your decision again"
                ))
            } else {
                not_found(id)
            });
        }
        transaction.commit()?;
        self.get(id)
    }

    pub fn accept_baseline(&self, root: &str) -> Result<usize> {
        Ok(self.conn.execute(SQL_ACCEPT_BASELINE, (self.clock.now().to_rfc3339(), root))?)
    }

    pub fn reset_baseline(&self, root: &str) -> Result<usize> {
        Ok(self.conn.execute(SQL_RESET_BASELINE, (self.clock.now().to_rfc3339(), root))?)
    }

    pub fn roots(&self) -> Result<Vec<String>> {
        let mut statement = self.conn.prepare(SQL_ROOTS)?;
        let rows = statement.query_map([], |row| row.get::<_, String>(0))?;
        let mut roots = Vec::new();
        for row in rows {
            roots.push(row?);
        }
        Ok(roots)
    }

    pub fn overview(&self) -> Result<Overview> {
        let mut overview = self.conn.query_row(SQL_OVERVIEW, [], |row| {
            Ok(Overview {
                attention: row.get(0)?,
                new: row.get(1)?,
                baseline: row.get(2)?,
                allowlisted: row.get(3)?,
                decided: row.get(4)?,
                gone: row.get(5)?,
                demo: row.get(6)?,
                scans: row.get(7)?,
                failed_scans: row.get(8)?,
                unread_notifications: row.get(9)?,
                by_rule: Vec::new(),
                top_files: Vec::new(),
            })
        })?;
        let mut statement = self.conn.prepare(SQL_BY_RULE)?;
        let rows = statement.query_map([], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?, row.get::<_, String>(2)?, row.get::<_, i64>(3)?))
        })?;
        for row in rows {
            let (rule_id, rule_name, level, total) = row?;
            if overview.by_rule.last().is_none_or(|last| last.rule_id != rule_id) {
                overview.by_rule.push(RuleCount { rule_id, rule_name, low: 0, medium: 0, high: 0 });
            }
            if let Some(last) = overview.by_rule.last_mut() {
                match Level::parse(&level) {
                    Some(Level::Low) => last.low += total,
                    Some(Level::Medium) => last.medium += total,
                    Some(Level::High) => last.high += total,
                    None => return Err(AppError::Storage(format!("a finding has the unreadable confidence \"{level}\""))),
                }
            }
        }
        let mut statement = self.conn.prepare(SQL_TOP_FILES)?;
        let rows = statement.query_map([TOP_FILES], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?, row.get::<_, i64>(2)?, row.get::<_, i64>(3)?))
        })?;
        for row in rows {
            let (root, path, total, top_score) = row?;
            overview.top_files.push(FileCount { root, path, total, top_score: u8::try_from(top_score).unwrap_or(100) });
        }
        Ok(overview)
    }
}
