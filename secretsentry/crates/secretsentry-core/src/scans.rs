use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::path::Path;
use std::sync::atomic::AtomicBool;
use std::time::Instant;

use rusqlite::{Connection, OptionalExtension, Row, Transaction, TransactionBehavior};

use crate::allowlists::{self, Allowlists};
use crate::baseline::Baseline;
use crate::clock::{Clock, Timestamp, Zone};
use crate::demo;
use crate::error::{AppError, Result};
use crate::findings::{self, CommitRecord, Recorded, Recording, RuleLabel, Sighting, Source};
use crate::git;
use crate::notify::{self, Event};
use crate::rulebook::{self, Rulebook};
use crate::rules;
use crate::scanner::{Allowed, Candidate, Detector};
use crate::theme::Severity;
use crate::tree;

pub const RECORD_KIND: &str = "scan";
pub const MAX_CANDIDATES_PER_SCAN: usize = 100_000;
pub const MAX_SIGHTINGS_PER_FINDING: usize = 5;
const LATEST_TIME: i64 = 253_402_300_799;
pub const MAX_PROBLEM_LENGTH: usize = 1000;
pub const DEFAULT_LIMIT: i64 = 100;
pub const INTERRUPTED: &str = "The scan was interrupted before it finished. Start it again.";

const SQL_START: &str = "INSERT INTO scans (kind, root, started_at, rules_used, is_demo) VALUES (?1, ?2, ?3, ?4, ?5)";
const SQL_INTERRUPT: &str = "UPDATE scans SET status = 'failed', problem = ?1, finished_at = ?2 WHERE status = 'running' AND root = ?3 AND kind = ?4";
const SQL_FINISH: &str = "UPDATE scans SET status = ?1, finished_at = ?2, duration_ms = ?3, files_scanned = ?4, files_skipped = ?5, bytes_scanned = ?6, commits_scanned = ?7, findings_seen = ?8, findings_new = ?9, problem = ?10 WHERE id = ?11";
const SQL_GET: &str = "SELECT id, kind, root, status, started_at, finished_at, duration_ms, files_scanned, files_skipped, bytes_scanned, commits_scanned, rules_used, findings_seen, findings_new, problem, is_demo FROM scans WHERE id = ?1";
const SQL_LIST: &str = "SELECT id, kind, root, status, started_at, finished_at, duration_ms, files_scanned, files_skipped, bytes_scanned, commits_scanned, rules_used, findings_seen, findings_new, problem, is_demo FROM scans WHERE (?1 = '' OR kind = ?1) ORDER BY id DESC LIMIT ?2";
const SQL_COUNT: &str = "SELECT count(*) FROM scans";
const SQL_COUNT_DEMO: &str = "SELECT count(*) FROM scans WHERE is_demo = 1";
const SQL_DELETE_DEMO: &str = "DELETE FROM scans WHERE is_demo = 1";

pub const HOT_QUERIES: &[(&str, &str, usize)] = &[("scan by identifier", SQL_GET, 1)];

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Kind {
    Tree,
    History,
}

impl Kind {
    pub const ALL: [Kind; 2] = [Kind::Tree, Kind::History];

    pub fn parse(text: &str) -> Option<Kind> {
        let wanted = text.trim().to_lowercase();
        Kind::ALL.into_iter().find(|kind| kind.as_str() == wanted)
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Kind::Tree => "tree",
            Kind::History => "history",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Kind::Tree => "Files",
            Kind::History => "Git history",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Status {
    Running,
    Completed,
    Failed,
    Cancelled,
}

impl Status {
    pub const ALL: [Status; 4] = [Status::Running, Status::Completed, Status::Failed, Status::Cancelled];

    pub fn parse(text: &str) -> Option<Status> {
        let wanted = text.trim().to_lowercase();
        Status::ALL.into_iter().find(|status| status.as_str() == wanted)
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Status::Running => "running",
            Status::Completed => "completed",
            Status::Failed => "failed",
            Status::Cancelled => "cancelled",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Status::Running => "Running",
            Status::Completed => "Completed",
            Status::Failed => "Failed",
            Status::Cancelled => "Cancelled",
        }
    }

    pub fn severity(self) -> Severity {
        match self {
            Status::Running | Status::Completed => Severity::Info,
            Status::Cancelled => Severity::Medium,
            Status::Failed => Severity::High,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Scan {
    pub id: i64,
    pub kind: Kind,
    pub root: String,
    pub status: Status,
    pub started_at: Timestamp,
    pub finished_at: Option<Timestamp>,
    pub duration_ms: i64,
    pub files_scanned: i64,
    pub files_skipped: i64,
    pub bytes_scanned: i64,
    pub commits_scanned: i64,
    pub rules_used: i64,
    pub findings_seen: i64,
    pub findings_new: i64,
    pub problem: String,
    pub demo: bool,
}

impl Scan {
    pub fn scope_label(&self) -> String {
        match self.kind {
            Kind::Tree => format!("{} files, {}", self.files_scanned, show_bytes(self.bytes_scanned)),
            Kind::History => format!("{} commits, {} file versions", self.commits_scanned, self.files_scanned),
        }
    }

    pub fn duration_label(&self) -> String {
        show_duration(self.duration_ms)
    }
}

pub fn show_duration(milliseconds: i64) -> String {
    let milliseconds = milliseconds.max(0);
    if milliseconds < 1000 {
        format!("{milliseconds} ms")
    } else if milliseconds < 60_000 {
        format!("{}.{} s", milliseconds / 1000, milliseconds % 1000 / 100)
    } else {
        format!("{} min {} s", milliseconds / 60_000, milliseconds % 60_000 / 1000)
    }
}

pub fn show_bytes(bytes: i64) -> String {
    let bytes = bytes.max(0);
    if bytes < 1024 {
        format!("{bytes} B")
    } else if bytes < 1024 * 1024 {
        format!("{}.{} KiB", bytes / 1024, bytes % 1024 * 10 / 1024)
    } else if bytes < 1024 * 1024 * 1024 {
        format!("{}.{} MiB", bytes / (1024 * 1024), bytes % (1024 * 1024) * 10 / (1024 * 1024))
    } else {
        format!("{}.{} GiB", bytes / (1024 * 1024 * 1024), bytes % (1024 * 1024 * 1024) / 1024 * 10 / (1024 * 1024))
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Outcome {
    pub scan: Scan,
    pub recorded: Recorded,
    pub stats: tree::Stats,
    pub portable: BTreeSet<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HistoryOutcome {
    pub scan: Scan,
    pub recorded: Recorded,
    pub stats: git::HistoryStats,
    pub portable: BTreeSet<String>,
}

fn portable_prints(sightings: &[Sighting<'_>]) -> BTreeSet<String> {
    sightings
        .iter()
        .filter(|sighting| sighting.candidate.allowed == Allowed::No && !sighting.candidate.portable.is_empty())
        .map(|sighting| sighting.candidate.portable.clone())
        .collect()
}

struct Harvest<'a> {
    source: Source,
    sightings: Vec<Sighting<'a>>,
    commits: Vec<CommitRecord>,
    complete: bool,
    cancelled: bool,
    files_scanned: u64,
    files_skipped: u64,
    bytes_scanned: u64,
    commits_scanned: u64,
    problem: String,
}

type RawScan =
    (i64, String, String, String, String, Option<String>, i64, i64, i64, i64, i64, i64, i64, i64, String, i64);

fn scan_row(row: &Row<'_>) -> rusqlite::Result<RawScan> {
    Ok((
        row.get(0)?,
        row.get(1)?,
        row.get(2)?,
        row.get(3)?,
        row.get(4)?,
        row.get(5)?,
        row.get(6)?,
        row.get(7)?,
        row.get(8)?,
        row.get(9)?,
        row.get(10)?,
        row.get(11)?,
        row.get(12)?,
        row.get(13)?,
        row.get(14)?,
        row.get(15)?,
    ))
}

fn build(raw: RawScan) -> Result<Scan> {
    let (
        id,
        kind,
        root,
        status,
        started,
        finished,
        duration_ms,
        files,
        skipped,
        bytes,
        commits,
        rules,
        seen,
        new,
        problem,
        demo,
    ) = raw;
    let unreadable = |what: &str| AppError::Storage(format!("scan {id} has an unreadable {what}"));
    Ok(Scan {
        id,
        kind: Kind::parse(&kind).ok_or_else(|| unreadable("kind"))?,
        root,
        status: Status::parse(&status).ok_or_else(|| unreadable("status"))?,
        started_at: Timestamp::parse(&started).ok_or_else(|| unreadable("start time"))?,
        finished_at: match finished {
            Some(text) => Some(Timestamp::parse(&text).ok_or_else(|| unreadable("end time"))?),
            None => None,
        },
        duration_ms,
        files_scanned: files,
        files_skipped: skipped,
        bytes_scanned: bytes,
        commits_scanned: commits,
        rules_used: rules,
        findings_seen: seen,
        findings_new: new,
        problem,
        demo: demo == 1,
    })
}

fn not_found(id: i64) -> AppError {
    AppError::NotFound(format!("scan {id} (list the scans to see valid identifiers)"))
}

pub fn root_name(root: &Path) -> Result<String> {
    tree::check_root(root)?;
    let resolved = std::fs::canonicalize(root).map_err(|error| {
        AppError::invalid(
            "path",
            format!("\"{}\" cannot be resolved ({error})", root.display()),
            "Check the spelling and the permissions of the directory",
        )
    })?;
    let shown = resolved.to_string_lossy();
    let shown = shown.strip_prefix(r"\\?\").unwrap_or(&shown);
    if shown.chars().count() > 4096 || shown.chars().any(char::is_control) {
        return Err(AppError::invalid(
            "path",
            "the directory name is too long or contains a control character",
            "Scan a directory with a shorter, plain name",
        ));
    }
    Ok(shown.to_string())
}

fn short_name(root: &str) -> &str {
    root.trim_end_matches(['/', '\\']).rsplit(['/', '\\']).next().filter(|name| !name.is_empty()).unwrap_or(root)
}

fn clip(problem: &str) -> String {
    let cleaned: String = problem.chars().map(|letter| if letter.is_control() { ' ' } else { letter }).collect();
    cleaned.chars().take(MAX_PROBLEM_LENGTH).collect()
}

pub struct Start<'a> {
    pub kind: Kind,
    pub root: &'a str,
    pub rules_used: usize,
    pub demo: bool,
    pub now: Timestamp,
}

pub fn start(conn: &Connection, start: &Start<'_>) -> Result<i64> {
    let now = start.now.to_rfc3339();
    conn.execute(SQL_INTERRUPT, (INTERRUPTED, &now, start.root, start.kind.as_str()))?;
    conn.execute(SQL_START, (start.kind.as_str(), start.root, &now, start.rules_used as i64, i64::from(start.demo)))?;
    Ok(conn.last_insert_rowid())
}

pub struct Finish<'a> {
    pub id: i64,
    pub status: Status,
    pub now: Timestamp,
    pub duration_ms: i64,
    pub files_scanned: u64,
    pub files_skipped: u64,
    pub bytes_scanned: u64,
    pub commits_scanned: u64,
    pub recorded: Recorded,
    pub problem: &'a str,
}

fn count(value: u64) -> i64 {
    i64::try_from(value).unwrap_or(i64::MAX)
}

pub fn finish(conn: &Connection, finish: &Finish<'_>) -> Result<()> {
    conn.execute(
        SQL_FINISH,
        (
            finish.status.as_str(),
            finish.now.to_rfc3339(),
            finish.duration_ms.max(0),
            count(finish.files_scanned),
            count(finish.files_skipped),
            count(finish.bytes_scanned),
            count(finish.commits_scanned),
            finish.recorded.seen as i64,
            finish.recorded.new as i64,
            clip(finish.problem),
            finish.id,
        ),
    )?;
    Ok(())
}

pub fn rule_labels(book: &Rulebook<'_>) -> Result<BTreeMap<String, RuleLabel>> {
    Ok(book
        .list()?
        .into_iter()
        .map(|rule| (rule.spec.id, RuleLabel { name: rule.spec.name, severity: rule.spec.severity }))
        .collect())
}

pub struct Scans<'a> {
    conn: &'a Connection,
    clock: Clock,
    zone: Zone,
    baseline: Option<&'a Baseline>,
}

impl<'a> Scans<'a> {
    pub fn new(conn: &'a Connection, clock: Clock, zone: Zone) -> Self {
        Scans { conn, clock, zone, baseline: None }
    }

    pub fn with_baseline(mut self, baseline: &'a Baseline) -> Self {
        self.baseline = Some(baseline);
        self
    }

    fn scan_detector(&self) -> Result<Detector> {
        let detector = self.detector()?;
        Ok(match self.baseline {
            Some(baseline) => detector.with_portable_key(baseline.key()),
            None => detector,
        })
    }

    fn begin(&self) -> Result<Transaction<'a>> {
        Ok(Transaction::new_unchecked(self.conn, TransactionBehavior::Immediate)?)
    }

    pub fn get(&self, id: i64) -> Result<Scan> {
        self.conn.query_row(SQL_GET, [id], scan_row).optional()?.map(build).transpose()?.ok_or_else(|| not_found(id))
    }

    pub fn list(&self, kind: Option<Kind>, limit: i64) -> Result<Vec<Scan>> {
        let limit = if limit <= 0 || limit > 1000 { DEFAULT_LIMIT } else { limit };
        let mut statement = self.conn.prepare(SQL_LIST)?;
        let rows = statement.query_map((kind.map_or("", Kind::as_str), limit), scan_row)?;
        let mut scans = Vec::new();
        for row in rows {
            scans.push(build(row?)?);
        }
        Ok(scans)
    }

    pub fn latest(&self) -> Result<Option<Scan>> {
        Ok(self.list(None, 1)?.into_iter().next())
    }

    pub fn count(&self) -> Result<i64> {
        Ok(self.conn.query_row(SQL_COUNT, [], |row| row.get(0))?)
    }

    pub fn detector(&self) -> Result<Detector> {
        let rules = Rulebook::new(self.conn, self.clock.clone()).enabled_specs()?;
        let allowlist = Allowlists::new(self.conn, self.clock.clone()).enabled_specs()?;
        let key = rulebook::fingerprint_key(self.conn, self.clock.now())?;
        Detector::new(&rules, &allowlist, &key)
    }

    fn fail(&self, id: i64, root: &str, started: Instant, error: &AppError) -> Result<()> {
        let transaction = self.begin()?;
        let now = self.clock.now();
        finish(
            &transaction,
            &Finish {
                id,
                status: Status::Failed,
                now,
                duration_ms: elapsed(started),
                files_scanned: 0,
                files_skipped: 0,
                bytes_scanned: 0,
                commits_scanned: 0,
                recorded: Recorded::default(),
                problem: &error.to_string(),
            },
        )?;
        notify::publish_with(
            &transaction,
            now,
            self.zone,
            &Event {
                event_type: notify::EVENT_SCAN_FAILED.to_string(),
                severity: Severity::High,
                title: format!("The scan of {} failed", short_name(root)),
                record_kind: RECORD_KIND.to_string(),
                record_id: id,
                dedup_key: format!("scan-failed:{root}"),
            },
        )?;
        transaction.commit()?;
        Ok(())
    }

    pub fn scan_tree(&self, root: &Path, options: &tree::Options, cancel: &AtomicBool) -> Result<Outcome> {
        let name = root_name(root)?;
        let detector = self.scan_detector()?;
        let started = Instant::now();
        let id = start(
            self.conn,
            &Start {
                kind: Kind::Tree,
                root: &name,
                rules_used: detector.rule_count(),
                demo: false,
                now: self.clock.now(),
            },
        )?;
        let mut candidates: Vec<Candidate> = Vec::new();
        let mut overflow = false;
        let walked = tree::walk(Path::new(&name), options, cancel, &mut |path, content| {
            if candidates.len() >= MAX_CANDIDATES_PER_SCAN {
                overflow = true;
                return;
            }
            let room = MAX_CANDIDATES_PER_SCAN - candidates.len();
            candidates.extend(detector.scan_text(path, content).candidates.into_iter().take(room));
        });
        let stats = match walked {
            Ok(stats) => stats,
            Err(error) => {
                self.fail(id, &name, started, &error)?;
                return Err(error);
            }
        };
        let problem = if overflow {
            format!(
                "Stopped collecting after {MAX_CANDIDATES_PER_SCAN} candidates. Scan a smaller directory or allowlist generated files."
            )
        } else if stats.file_limit_reached {
            format!("Stopped after {} files. Scan a smaller directory.", tree::MAX_FILES)
        } else {
            String::new()
        };
        let harvest = Harvest {
            source: Source::Tree,
            sightings: candidates.iter().map(|candidate| Sighting { candidate, commit_id: "" }).collect(),
            commits: Vec::new(),
            complete: stats.complete() && !overflow,
            cancelled: stats.cancelled,
            files_scanned: stats.files_scanned,
            files_skipped: stats.skipped(),
            bytes_scanned: stats.bytes_scanned,
            commits_scanned: 0,
            problem,
        };
        match self.store(id, &name, started, &harvest) {
            Ok(recorded) => {
                Ok(Outcome { scan: self.get(id)?, recorded, stats, portable: portable_prints(&harvest.sightings) })
            }
            Err(error) => {
                self.fail(id, &name, started, &error)?;
                Err(error)
            }
        }
    }

    pub fn scan_history(
        &self,
        root: &Path,
        options: &git::HistoryOptions,
        cancel: &AtomicBool,
    ) -> Result<HistoryOutcome> {
        let name = root_name(root)?;
        let mut repository = git::Repository::open(Path::new(&name))?;
        let detector = self.scan_detector()?;
        let started = Instant::now();
        let id = start(
            self.conn,
            &Start {
                kind: Kind::History,
                root: &name,
                rules_used: detector.rule_count(),
                demo: false,
                now: self.clock.now(),
            },
        )?;
        let mut kept: Vec<(Candidate, String)> = Vec::new();
        let mut per_finding: HashMap<String, usize> = HashMap::new();
        let mut commits: BTreeMap<String, CommitRecord> = BTreeMap::new();
        let mut overflow = false;
        let stats = git::walk_history(&mut repository, options, cancel, &mut |commit, path, content| {
            for candidate in detector.scan_text(path, content).candidates {
                let count = per_finding.entry(candidate.fingerprint.clone()).or_insert(0);
                if *count >= MAX_SIGHTINGS_PER_FINDING {
                    continue;
                }
                if kept.len() >= MAX_CANDIDATES_PER_SCAN {
                    overflow = true;
                    return;
                }
                *count += 1;
                let commit_id = commit.id.to_hex();
                commits.entry(commit_id.clone()).or_insert_with(|| CommitRecord {
                    id: commit_id.clone(),
                    author: commit.author.clone(),
                    authored_at: Timestamp::from_unix(commit.time.clamp(0, LATEST_TIME)),
                    subject: commit.subject.clone(),
                });
                kept.push((candidate, commit_id));
            }
        });
        let mut notes = Vec::new();
        if overflow {
            notes.push(format!(
                "Stopped collecting after {MAX_CANDIDATES_PER_SCAN} candidates. Allowlist generated files and scan again."
            ));
        }
        if stats.limit_reached {
            notes.push(format!(
                "Stopped at the limit of {} commits or {} objects, so older or newer history may be missing.",
                options.max_commits,
                git::MAX_VISITED
            ));
        }
        if stats.skipped_packs > 0 {
            notes.push(format!("{} pack files could not be read and were skipped.", stats.skipped_packs));
        }
        if stats.missing > 0 {
            notes.push(format!(
                "{} objects are not in the repository, which is usual for a shallow or partial clone.",
                stats.missing
            ));
        }
        let harvest = Harvest {
            source: Source::History,
            sightings: kept.iter().map(|(candidate, commit_id)| Sighting { candidate, commit_id }).collect(),
            commits: commits.into_values().collect(),
            complete: stats.complete() && !overflow && stats.skipped_packs == 0,
            cancelled: stats.cancelled,
            files_scanned: stats.files_scanned,
            files_skipped: stats.skipped(),
            bytes_scanned: stats.bytes_scanned,
            commits_scanned: stats.commits_scanned,
            problem: notes.join(" "),
        };
        match self.store(id, &name, started, &harvest) {
            Ok(recorded) => Ok(HistoryOutcome {
                scan: self.get(id)?,
                recorded,
                stats,
                portable: portable_prints(&harvest.sightings),
            }),
            Err(error) => {
                self.fail(id, &name, started, &error)?;
                Err(error)
            }
        }
    }

    pub fn demo_count(&self) -> Result<i64> {
        let scans: i64 = self.conn.query_row(SQL_COUNT_DEMO, [], |row| row.get(0))?;
        Ok(scans + findings::demo_count(self.conn)?)
    }

    pub fn load_demo(&self) -> Result<usize> {
        if self.demo_count()? > 0 {
            return Ok(0);
        }
        let key = rulebook::fingerprint_key(self.conn, self.clock.now())?;
        let builtin = rules::builtin();
        let labels: BTreeMap<String, RuleLabel> = builtin
            .iter()
            .map(|rule| (rule.id.clone(), RuleLabel { name: rule.name.clone(), severity: rule.severity }))
            .collect();
        let transaction = self.begin()?;
        let now = self.clock.now();
        let entry = demo::allowlist_entry();
        let entry_id = allowlists::insert(&transaction, now, &entry, true)?;
        let detector = Detector::new(&builtin, &[crate::allow::EntrySpec { id: entry_id, ..entry }], &key)?;

        let files = demo::tree();
        let mut candidates: Vec<Candidate> = Vec::new();
        let mut bytes = 0u64;
        for file in &files {
            bytes += file.content.len() as u64;
            candidates.extend(detector.scan_text(file.path, &file.content).candidates);
        }
        let tree_id = start(
            &transaction,
            &Start { kind: Kind::Tree, root: demo::ROOT, rules_used: detector.rule_count(), demo: true, now },
        )?;
        let sightings: Vec<Sighting<'_>> =
            candidates.iter().map(|candidate| Sighting { candidate, commit_id: "" }).collect();
        let recorded = findings::record(
            &transaction,
            &Recording {
                scan_id: tree_id,
                root: demo::ROOT,
                source: Source::Tree,
                demo: true,
                complete: true,
                now,
                known: None,
            },
            &labels,
            &sightings,
        )?;
        let mut loaded = recorded.seen;
        finish(
            &transaction,
            &Finish {
                id: tree_id,
                status: Status::Completed,
                now,
                duration_ms: demo::TREE_DURATION_MS,
                files_scanned: files.len() as u64,
                files_skipped: demo::TREE_FILES_SKIPPED,
                bytes_scanned: bytes,
                commits_scanned: 0,
                recorded,
                problem: "",
            },
        )?;

        let commits = demo::history();
        let mut kept: Vec<(Candidate, &str)> = Vec::new();
        let mut versions = 0u64;
        let mut history_bytes = 0u64;
        for commit in &commits {
            for file in &commit.files {
                versions += 1;
                history_bytes += file.content.len() as u64;
                for candidate in detector.scan_text(file.path, &file.content).candidates {
                    kept.push((candidate, commit.id.as_str()));
                }
            }
        }
        let records: Vec<CommitRecord> = commits
            .iter()
            .map(|commit| CommitRecord {
                id: commit.id.clone(),
                author: demo::AUTHOR.to_string(),
                authored_at: now.plus_seconds(-commit.days_ago * 86_400),
                subject: commit.subject.to_string(),
            })
            .collect();
        findings::record_commits(&transaction, demo::ROOT, &records)?;
        let history_id = start(
            &transaction,
            &Start { kind: Kind::History, root: demo::ROOT, rules_used: detector.rule_count(), demo: true, now },
        )?;
        let sightings: Vec<Sighting<'_>> =
            kept.iter().map(|(candidate, commit_id)| Sighting { candidate, commit_id }).collect();
        let recorded = findings::record(
            &transaction,
            &Recording {
                scan_id: history_id,
                root: demo::ROOT,
                source: Source::History,
                demo: true,
                complete: true,
                now,
                known: None,
            },
            &labels,
            &sightings,
        )?;
        loaded += recorded.new;
        finish(
            &transaction,
            &Finish {
                id: history_id,
                status: Status::Completed,
                now,
                duration_ms: demo::HISTORY_DURATION_MS,
                files_scanned: versions,
                files_skipped: 0,
                bytes_scanned: history_bytes,
                commits_scanned: commits.len() as u64,
                recorded,
                problem: "",
            },
        )?;
        transaction.commit()?;
        Ok(loaded)
    }

    pub fn remove_demo(&self, dry_run: bool) -> Result<i64> {
        let count = findings::demo_count(self.conn)?;
        if dry_run {
            return Ok(count);
        }
        let transaction = self.begin()?;
        findings::remove_demo(&transaction)?;
        findings::remove_commits(&transaction, demo::ROOT)?;
        transaction.execute(SQL_DELETE_DEMO, [])?;
        allowlists::remove_demo(&transaction)?;
        transaction.commit()?;
        Ok(count)
    }

    fn store(&self, id: i64, root: &str, started: Instant, harvest: &Harvest<'_>) -> Result<Recorded> {
        let labels = rule_labels(&Rulebook::new(self.conn, self.clock.clone()))?;
        let transaction = self.begin()?;
        let now = self.clock.now();
        findings::record_commits(&transaction, root, &harvest.commits)?;
        let recorded = findings::record(
            &transaction,
            &Recording {
                scan_id: id,
                root,
                source: harvest.source,
                demo: false,
                complete: harvest.complete,
                now,
                known: self.baseline.map(Baseline::entries),
            },
            &labels,
            &harvest.sightings,
        )?;
        finish(
            &transaction,
            &Finish {
                id,
                status: if harvest.cancelled { Status::Cancelled } else { Status::Completed },
                now,
                duration_ms: elapsed(started),
                files_scanned: harvest.files_scanned,
                files_skipped: harvest.files_skipped,
                bytes_scanned: harvest.bytes_scanned,
                commits_scanned: harvest.commits_scanned,
                recorded,
                problem: &harvest.problem,
            },
        )?;
        if recorded.new > 0 {
            let noun = if recorded.new == 1 { "new likely secret" } else { "new likely secrets" };
            let place = match harvest.source {
                Source::Tree => "in",
                Source::History => "in the history of",
            };
            notify::publish_with(
                &transaction,
                now,
                self.zone,
                &Event {
                    event_type: notify::EVENT_SCAN_FINDING.to_string(),
                    severity: recorded.top_new_severity.unwrap_or(Severity::Medium),
                    title: format!("{} {noun} {place} {}", recorded.new, short_name(root)),
                    record_kind: RECORD_KIND.to_string(),
                    record_id: id,
                    dedup_key: format!("scan:{id}"),
                },
            )?;
        }
        transaction.commit()?;
        Ok(recorded)
    }
}

fn elapsed(started: Instant) -> i64 {
    i64::try_from(started.elapsed().as_millis()).unwrap_or(i64::MAX)
}

#[cfg(test)]
mod tests {
    use std::fs;

    use super::*;
    use crate::allow::{EntrySpec, Kind as AllowKind};
    use crate::confidence::Level;
    use crate::findings::{Findings, Query, Scope, Status as Decision};
    use crate::notify::{Filter, Notifications};
    use crate::store::Store;
    use crate::testkit::{
        GitFixture, TempDir, fixed_time, sample_cloud_key, sample_hosting_token, sample_package_token,
    };

    struct Fixture {
        directory: TempDir,
        store: Store,
        clock: Clock,
        hands: crate::clock::ManualClock,
    }

    impl Fixture {
        fn new(label: &str) -> Fixture {
            let directory = TempDir::new(label);
            let store = Store::open_ready(&directory.path().join("app.db")).unwrap();
            fs::create_dir_all(directory.path().join("project")).unwrap();
            let (clock, hands) = Clock::manual(fixed_time());
            Fixture { directory, store, clock, hands }
        }

        fn root(&self) -> std::path::PathBuf {
            self.directory.path().join("project")
        }

        fn write(&self, relative: &str, content: &str) {
            let path = self.root().join(relative);
            fs::create_dir_all(path.parent().unwrap()).unwrap();
            fs::write(path, content).unwrap();
        }

        fn remove(&self, relative: &str) {
            fs::remove_file(self.root().join(relative)).unwrap();
        }

        fn scans(&self) -> Scans<'_> {
            Scans::new(self.store.conn(), self.clock.clone(), Zone::Utc)
        }

        fn findings(&self) -> Findings<'_> {
            Findings::new(self.store.conn(), self.clock.clone())
        }

        fn scan(&self) -> Outcome {
            self.hands.advance(60);
            self.scans().scan_tree(&self.root(), &tree::Options::default(), &AtomicBool::new(false)).unwrap()
        }

        fn all(&self) -> Vec<findings::Finding> {
            self.findings().list(&Query { sort: "path".to_string(), descending: false, ..Query::default() }).unwrap()
        }

        fn notifications(&self) -> Vec<notify::Notification> {
            Notifications::new(self.store.conn(), self.clock.clone(), Zone::Utc).list(&Filter::default()).unwrap()
        }
    }

    fn seeded(label: &str) -> Fixture {
        let fixture = Fixture::new(label);
        fixture.write("src/config.py", &format!("AWS_KEY = \"{}\"\nother = 1\n", sample_cloud_key()));
        fixture.write(
            "deploy/ci.yaml",
            &format!("token: {}\nagain: {}\n", sample_hosting_token(), sample_hosting_token()),
        );
        fixture.write("README.md", "No secrets here.\n");
        fixture
    }

    #[test]
    fn a_tree_scan_stores_findings_without_their_values_and_reports_the_scope() {
        let fixture = seeded("scans-first");
        let outcome = fixture.scan();
        assert_eq!(outcome.scan.status, Status::Completed);
        assert_eq!(outcome.scan.kind, Kind::Tree);
        assert_eq!((outcome.scan.files_scanned, outcome.scan.findings_seen, outcome.scan.findings_new), (3, 2, 2));
        assert_eq!(outcome.scan.rules_used, crate::rules::builtin().len() as i64);
        assert_eq!(outcome.scan.finished_at, Some(fixed_time().plus_seconds(60)));
        assert_eq!(outcome.recorded.top_new_severity, Some(Severity::Critical));
        assert!(outcome.scan.root.ends_with("project"));
        assert_eq!(outcome.scan.scope_label(), format!("3 files, {} B", outcome.scan.bytes_scanned));

        let all = fixture.all();
        assert_eq!(
            all.iter().map(|finding| finding.path.as_str()).collect::<Vec<_>>(),
            vec!["deploy/ci.yaml", "src/config.py"]
        );
        let token = &all[0];
        assert_eq!((token.rule_id.as_str(), token.rule_name.as_str()), ("github-token", "GitHub token"));
        assert_eq!(token.preview, "ghp_********");
        assert_eq!(token.length, 40);
        assert!(token.in_tree && !token.in_history && !token.baseline && !token.demo);
        assert_eq!(token.status, Decision::Open);
        assert_eq!(token.state_label(), "New");
        assert_eq!(token.level(), Level::High);
        assert!(!token.reasons.is_empty());
        assert_eq!(token.last_scan_id, Some(outcome.scan.id));
        let places = fixture.findings().occurrences(token.id).unwrap();
        assert_eq!(places.iter().map(|place| (place.line, place.column)).collect::<Vec<_>>(), vec![(1, 8), (2, 8)]);
        assert_eq!(places[0].context, "token: ghp_********");
        assert!(places.iter().all(|place| place.commit_id.is_empty() && place.scan_id == outcome.scan.id));

        let mut checked = 0;
        for entry in fs::read_dir(fixture.directory.path()).unwrap() {
            let path = entry.unwrap().path();
            if !path.is_file() {
                continue;
            }
            checked += 1;
            let bytes = fs::read(&path).unwrap();
            let stored = String::from_utf8_lossy(&bytes);
            for secret in [sample_cloud_key(), sample_hosting_token()] {
                assert!(!stored.contains(&secret), "{} holds a secret value", path.display());
                assert!(!stored.contains(&secret[8..]), "{} holds the tail of a secret value", path.display());
            }
        }
        assert!(checked >= 1);
    }

    #[test]
    fn a_second_scan_keeps_decisions_and_marks_what_is_gone_and_what_is_new() {
        let fixture = seeded("scans-again");
        fixture.scan();
        let first = fixture.all();
        let decided =
            fixture.findings().decide(first[1].id, first[1].revision, Decision::AcceptedRisk, " rotated ").unwrap();
        assert_eq!((decided.status, decided.note.as_str(), decided.revision), (Decision::AcceptedRisk, "rotated", 2));
        assert_eq!(fixture.findings().accept_baseline("").unwrap(), 2);

        fixture.remove("deploy/ci.yaml");
        fixture.write("package/.npmrc", &format!("//registry/:_authToken={}\n", sample_package_token()));
        let again = fixture.scan();
        assert_eq!((again.scan.findings_seen, again.scan.findings_new), (2, 1));

        let all = fixture.all();
        assert_eq!(all.len(), 3);
        let gone = all.iter().find(|finding| finding.path == "deploy/ci.yaml").unwrap();
        assert!(!gone.in_tree && gone.baseline);
        assert_eq!(gone.state_label(), "No longer found");
        assert_eq!(gone.first_seen_at, fixed_time().plus_seconds(60));
        assert_eq!(gone.last_seen_at, fixed_time().plus_seconds(60));
        let kept = all.iter().find(|finding| finding.path == "src/config.py").unwrap();
        assert_eq!((kept.status, kept.note.as_str(), kept.baseline), (Decision::AcceptedRisk, "rotated", true));
        assert_eq!(kept.state_label(), "Decided");
        assert_eq!(kept.last_seen_at, fixed_time().plus_seconds(120));
        assert_eq!(kept.first_seen_at, fixed_time().plus_seconds(60));
        let fresh = all.iter().find(|finding| finding.path == "package/.npmrc").unwrap();
        assert_eq!(fresh.state_label(), "New");

        let findings = fixture.findings();
        let count = |scope: Scope| findings.total(&Query { scope, ..Query::default() }).unwrap();
        assert_eq!(count(Scope::All), 3);
        assert_eq!(count(Scope::Attention), 1);
        assert_eq!(count(Scope::New), 1);
        assert_eq!(count(Scope::Baseline), 0);
        assert_eq!(count(Scope::Decided), 1);
        assert_eq!(count(Scope::Gone), 1);
        assert_eq!(count(Scope::Allowlisted), 0);
        assert_eq!(count(Scope::History), 0);

        fixture.write("deploy/ci.yaml", &format!("token: {}\n", sample_hosting_token()));
        let back = fixture.scan();
        assert_eq!(back.scan.findings_new, 0);
        let returned = fixture.all().into_iter().find(|finding| finding.path == "deploy/ci.yaml").unwrap();
        assert!(returned.in_tree && returned.baseline);
        assert_eq!(returned.state_label(), "Baseline");
        assert_eq!(fixture.findings().occurrences(returned.id).unwrap().len(), 1);
        assert_eq!(fixture.findings().reset_baseline(&back.scan.root).unwrap(), 2);
        assert_eq!(fixture.findings().reset_baseline("").unwrap(), 0);
    }

    #[test]
    fn allowlisted_findings_are_kept_and_marked_but_not_counted_as_new() {
        let fixture = seeded("scans-allow");
        fixture.write("docs/example.env", &format!("NPM_TOKEN={} # secretsentry:allow\n", sample_package_token()));
        Allowlists::new(fixture.store.conn(), fixture.clock.clone())
            .create(&EntrySpec { kind: AllowKind::Path, pattern: "deploy/".to_string(), ..EntrySpec::default() })
            .unwrap();
        let outcome = fixture.scan();
        assert_eq!((outcome.scan.findings_seen, outcome.scan.findings_new), (3, 1));
        let all = fixture.all();
        assert!(matches!(all[0].allowed, Allowed::Entry(_)));
        assert_eq!(all[1].allowed, Allowed::Marker);
        assert_eq!(all[2].allowed, Allowed::No);
        assert_eq!(all[0].state_label(), "Allowlisted");
        assert!(!all[0].needs_attention() && !all[1].needs_attention() && all[2].needs_attention());
        let overview = fixture.findings().overview().unwrap();
        assert_eq!((overview.attention, overview.new, overview.allowlisted), (1, 1, 2));

        for entry in Allowlists::new(fixture.store.conn(), fixture.clock.clone()).list().unwrap() {
            Allowlists::new(fixture.store.conn(), fixture.clock.clone()).delete(entry.spec.id).unwrap();
        }
        let after = fixture.scan();
        assert_eq!(after.scan.findings_new, 0);
        assert_eq!(fixture.all()[0].allowed, Allowed::No);
        assert_eq!(fixture.findings().overview().unwrap().attention, 2);
    }

    #[test]
    fn the_overview_counts_by_rule_confidence_and_file() {
        let fixture = seeded("scans-overview");
        fixture.write("tests/fixture.txt", &format!("key = {}\n", ["AK", "IA", "T7XN4QWZ2KPLM6R3"].concat()));
        fixture.write(
            "src/config.py",
            &format!("AWS_KEY = \"{}\"\nNPM = \"{}\"\n", sample_cloud_key(), sample_package_token()),
        );
        let outcome = fixture.scan();
        let overview = fixture.findings().overview().unwrap();
        assert_eq!(
            (overview.attention, overview.new, overview.baseline, overview.gone, overview.demo),
            (4, 4, 0, 0, 0)
        );
        assert_eq!((overview.scans, overview.failed_scans, overview.unread_notifications), (1, 0, 1));
        let aws = overview.by_rule.iter().find(|rule| rule.rule_id == "aws-access-key-id").unwrap();
        assert_eq!((aws.low, aws.medium, aws.high, aws.total()), (0, 1, 1, 2));
        assert_eq!(overview.by_level(Level::High), 3);
        assert_eq!(overview.by_level(Level::Medium), 1);
        assert_eq!(overview.by_level(Level::Low), 0);
        assert_eq!(overview.by_rule.iter().map(|rule| rule.rule_name.as_str()).collect::<Vec<_>>(), {
            let mut names: Vec<&str> = overview.by_rule.iter().map(|rule| rule.rule_name.as_str()).collect();
            names.sort_unstable();
            names
        });
        assert_eq!(overview.top_files[0].path, "src/config.py");
        assert_eq!(overview.top_files[0].total, 2);
        assert!(overview.top_files[0].top_score >= 80);
        assert_eq!(overview.top_files[0].root, outcome.scan.root);
        assert_eq!(overview.top_files.len(), 3);
        assert_eq!(fixture.findings().roots().unwrap(), vec![outcome.scan.root.clone()]);
        assert_eq!(fixture.findings().count().unwrap(), 4);
    }

    #[test]
    fn findings_are_filtered_sorted_and_paged() {
        let fixture = seeded("scans-query");
        fixture.write("tests/fixture.txt", &format!("key = {}\n", ["AK", "IA", "T7XN4QWZ2KPLM6R3"].concat()));
        let outcome = fixture.scan();
        let findings = fixture.findings();
        let paths = |query: Query| -> Vec<String> {
            findings.list(&query).unwrap().into_iter().map(|finding| finding.path).collect()
        };
        assert_eq!(paths(Query::default()), vec!["deploy/ci.yaml", "src/config.py", "tests/fixture.txt"]);
        assert_eq!(
            paths(Query { sort: "path".to_string(), ..Query::default() }),
            vec!["tests/fixture.txt", "src/config.py", "deploy/ci.yaml"]
        );
        assert_eq!(paths(Query { search: "CONFIG".to_string(), ..Query::default() }), vec!["src/config.py"]);
        assert_eq!(paths(Query { search: "github".to_string(), ..Query::default() }), vec!["deploy/ci.yaml"]);
        assert_eq!(
            paths(Query { rule_id: "aws-access-key-id".to_string(), ..Query::default() }),
            vec!["src/config.py", "tests/fixture.txt"]
        );
        assert_eq!(paths(Query { level: Some(Level::Medium), ..Query::default() }), vec!["tests/fixture.txt"]);
        assert_eq!(paths(Query { min_score: 75, ..Query::default() }), vec!["deploy/ci.yaml", "src/config.py"]);
        assert_eq!(paths(Query { status: Some(Decision::Resolved), ..Query::default() }), Vec::<String>::new());
        assert_eq!(paths(Query { root: "/elsewhere".to_string(), ..Query::default() }), Vec::<String>::new());
        assert_eq!(paths(Query { root: outcome.scan.root.clone(), ..Query::default() }).len(), 3);
        assert_eq!(paths(Query { path: "src/config.py".to_string(), ..Query::default() }), vec!["src/config.py"]);
        let page = findings.page(&Query { limit: 1, offset: 1, ..Query::default() }).unwrap();
        assert_eq!((page.total, page.findings.len()), (3, 1));
        assert_eq!(page.findings[0].path, "src/config.py");
        for key in findings::SORT_KEYS {
            for descending in [false, true] {
                assert_eq!(
                    findings.list(&Query { sort: (*key).to_string(), descending, ..Query::default() }).unwrap().len(),
                    3
                );
            }
        }
        let unknown = findings.list(&Query { sort: "colour".to_string(), ..Query::default() }).unwrap_err().to_string();
        assert_eq!(unknown, "sort: \"colour\" is not a sort key. Use one of: score, rule, path, status, seen");
    }

    #[test]
    fn decisions_need_the_current_revision_and_a_usable_note() {
        let fixture = seeded("scans-decide");
        fixture.scan();
        let findings = fixture.findings();
        let target = fixture.all().remove(0);
        let confirmed = findings.decide(target.id, 1, Decision::Confirmed, "").unwrap();
        assert!(confirmed.needs_attention());
        let stale = findings.decide(target.id, 1, Decision::Resolved, "").unwrap_err();
        assert!(matches!(stale, AppError::Conflict(_)), "{stale}");
        let missing = findings.decide(9999, 1, Decision::Resolved, "").unwrap_err();
        assert!(matches!(missing, AppError::NotFound(_)), "{missing}");
        assert!(matches!(findings.get(9999).unwrap_err(), AppError::NotFound(_)));
        let long = findings.decide(target.id, 2, Decision::Resolved, &"n".repeat(2001)).unwrap_err().to_string();
        assert!(long.starts_with("note: the note is longer than 2000 characters"), "{long}");
        let control = findings.decide(target.id, 2, Decision::Resolved, "bell\u{7}").unwrap_err().to_string();
        assert!(control.starts_with("note: the note contains a control character"), "{control}");
        let resolved = findings.decide(target.id, 2, Decision::Resolved, "line one\nline two").unwrap();
        assert_eq!((resolved.status, resolved.revision), (Decision::Resolved, 3));
        assert!(!resolved.needs_attention());
        for status in Decision::ALL {
            assert_eq!(Decision::parse(status.as_str()), Some(status));
            assert_eq!(Decision::parse(status.label()), Some(status));
        }
        assert_eq!(Decision::parse("false-positive"), Some(Decision::FalsePositive));
        assert_eq!(Decision::parse("ignored"), None);
        for scope in Scope::ALL {
            assert_eq!(Scope::parse(scope.as_str()), Some(scope));
            assert!(!scope.label().is_empty());
        }
    }

    #[test]
    fn new_findings_and_failures_are_announced_once() {
        let fixture = seeded("scans-notify");
        let first = fixture.scan();
        let announced = fixture.notifications();
        assert_eq!(announced.len(), 1);
        assert_eq!(announced[0].event_type, notify::EVENT_SCAN_FINDING);
        assert_eq!(announced[0].title, "2 new likely secrets in project");
        assert_eq!(announced[0].severity, Severity::Critical);
        assert_eq!((announced[0].record_kind.as_str(), announced[0].record_id), (RECORD_KIND, first.scan.id));
        fixture.scan();
        assert_eq!(fixture.notifications().len(), 1);
        fixture.write("package/.npmrc", &format!("//registry/:_authToken={}\n", sample_package_token()));
        fixture.scan();
        let announced = fixture.notifications();
        assert_eq!(announced.len(), 2);
        assert_eq!(announced[0].title, "1 new likely secret in project");
        assert!(!announced.iter().any(|notification| notification.title.contains("npm_")));
    }

    #[test]
    fn a_scan_that_cannot_start_names_the_problem_and_stores_nothing() {
        let fixture = seeded("scans-refused");
        let scans = fixture.scans();
        let missing = scans
            .scan_tree(&fixture.root().join("missing"), &tree::Options::default(), &AtomicBool::new(false))
            .unwrap_err()
            .to_string();
        assert!(missing.starts_with("path: ") && missing.contains("cannot be opened"), "{missing}");
        let file = scans
            .scan_tree(&fixture.root().join("README.md"), &tree::Options::default(), &AtomicBool::new(false))
            .unwrap_err()
            .to_string();
        assert!(file.contains("is not a directory"), "{file}");
        assert_eq!(scans.count().unwrap(), 0);
        assert!(scans.latest().unwrap().is_none());
        assert!(matches!(scans.get(1).unwrap_err(), AppError::NotFound(_)));
    }

    #[test]
    fn a_cancelled_scan_is_recorded_and_does_not_mark_findings_as_gone() {
        let fixture = seeded("scans-cancel");
        fixture.scan();
        let cancelled =
            fixture.scans().scan_tree(&fixture.root(), &tree::Options::default(), &AtomicBool::new(true)).unwrap();
        assert_eq!(cancelled.scan.status, Status::Cancelled);
        assert_eq!((cancelled.scan.files_scanned, cancelled.scan.findings_seen), (0, 0));
        assert!(cancelled.stats.cancelled);
        assert!(fixture.all().iter().all(|finding| finding.in_tree));
        let listed = fixture.scans().list(None, 0).unwrap();
        assert_eq!(
            listed.iter().map(|scan| scan.status).collect::<Vec<_>>(),
            vec![Status::Cancelled, Status::Completed]
        );
        assert_eq!(fixture.scans().list(Some(Kind::History), 10).unwrap().len(), 0);
        assert_eq!(fixture.scans().latest().unwrap().unwrap().id, cancelled.scan.id);
    }

    #[test]
    fn an_interrupted_scan_is_marked_failed_when_the_next_one_starts() {
        let fixture = seeded("scans-interrupted");
        let root = root_name(&fixture.root()).unwrap();
        let abandoned = start(
            fixture.store.conn(),
            &Start { kind: Kind::Tree, root: &root, rules_used: 1, demo: false, now: fixed_time() },
        )
        .unwrap();
        assert_eq!(fixture.scans().get(abandoned).unwrap().status, Status::Running);
        fixture.scan();
        let marked = fixture.scans().get(abandoned).unwrap();
        assert_eq!((marked.status, marked.problem.as_str()), (Status::Failed, INTERRUPTED));
        assert_eq!(marked.finished_at, Some(fixed_time().plus_seconds(60)));
    }

    #[test]
    fn a_failure_is_stored_with_its_reason_and_announced() {
        let fixture = seeded("scans-failed");
        let root = root_name(&fixture.root()).unwrap();
        let scans = fixture.scans();
        let id = start(
            fixture.store.conn(),
            &Start { kind: Kind::Tree, root: &root, rules_used: 1, demo: false, now: fixed_time() },
        )
        .unwrap();
        scans
            .fail(id, &root, Instant::now(), &AppError::Io("the directory cannot be read:\tdenied".to_string()))
            .unwrap();
        let failed = scans.get(id).unwrap();
        assert_eq!((failed.status, failed.problem.as_str()), (Status::Failed, "the directory cannot be read: denied"));
        assert_eq!(failed.status.severity(), Severity::High);
        let announced = fixture.notifications();
        assert_eq!(announced.len(), 1);
        assert_eq!(
            (announced[0].event_type.as_str(), announced[0].title.as_str()),
            (notify::EVENT_SCAN_FAILED, "The scan of project failed")
        );
        assert_eq!(fixture.findings().overview().unwrap().failed_scans, 1);
    }

    struct Repo {
        fixture: Fixture,
        git: GitFixture,
        first: String,
        app: String,
    }

    fn repo(label: &str) -> Repo {
        let fixture = Fixture::new(label);
        let git = GitFixture::create(&fixture.root());
        let app = git.blob("print('hi')\n");
        let secret = git.blob(&format!("TOKEN={}\n", sample_hosting_token()));
        let cleaned = git.blob("TOKEN=\n");
        let deploy = git.blob(&format!("export AWS_KEY=\"{}\"\n", sample_cloud_key()));
        let tree_one = git.tree(&[("100644", "app.py", &app), ("100644", "config.env", &secret)]);
        let tree_two = git.tree(&[("100644", "app.py", &app), ("100644", "config.env", &cleaned)]);
        let tree_three = git.tree(&[
            ("100644", "app.py", &app),
            ("100644", "config.env", &cleaned),
            ("100755", "deploy.sh", &deploy),
        ]);
        let first = git.commit(&tree_one, &[], 1_700_000_000, "add configuration");
        let second = git.commit(&tree_two, &[&first], 1_700_000_100, &format!("remove {}", sample_hosting_token()));
        let third = git.commit(&tree_three, &[&second], 1_700_000_200, "add the deploy script");
        git.set_reference("refs/heads/main", &third);
        fixture.write("app.py", "print('hi')\n");
        fixture.write("config.env", "TOKEN=\n");
        fixture.write("deploy.sh", &format!("export AWS_KEY=\"{}\"\n", sample_cloud_key()));
        Repo { fixture, git, first, app }
    }

    impl Repo {
        fn scan(&self) -> HistoryOutcome {
            self.fixture.hands.advance(60);
            self.fixture
                .scans()
                .scan_history(&self.fixture.root(), &git::HistoryOptions::default(), &AtomicBool::new(false))
                .unwrap()
        }
    }

    #[test]
    fn a_history_scan_finds_a_removed_secret_and_names_the_commit_that_added_it() {
        let repo = repo("scans-history");
        let outcome = repo.scan();
        assert_eq!((outcome.scan.kind, outcome.scan.status), (Kind::History, Status::Completed));
        assert_eq!((outcome.scan.commits_scanned, outcome.scan.files_scanned), (3, 4));
        assert_eq!((outcome.scan.findings_seen, outcome.scan.findings_new), (2, 2));
        assert_eq!(outcome.scan.scope_label(), "3 commits, 4 file versions");
        assert_eq!(outcome.scan.problem, "");
        assert!(outcome.stats.complete());

        let all = repo.fixture.all();
        assert_eq!(
            all.iter().map(|finding| finding.path.as_str()).collect::<Vec<_>>(),
            vec!["config.env", "deploy.sh"]
        );
        let removed = &all[0];
        assert_eq!(removed.rule_id, "github-token");
        assert!(removed.in_history && !removed.in_tree);
        assert_eq!(removed.state_label(), "New");
        let places = repo.fixture.findings().occurrences(removed.id).unwrap();
        assert_eq!(places.len(), 1);
        assert_eq!((places[0].line, places[0].column, places[0].context.as_str()), (1, 7, "TOKEN=ghp_********"));
        assert_eq!(places[0].commit_id, repo.first);
        assert_eq!((places[0].author.as_str(), places[0].subject.as_str()), ("Sample Author", "add configuration"));
        assert_eq!(places[0].authored_at, Some(Timestamp::from_unix(1_700_000_000)));
        assert_eq!(repo.fixture.notifications()[0].title, "2 new likely secrets in the history of project");

        let tree = repo.fixture.scan();
        assert_eq!((tree.scan.findings_seen, tree.scan.findings_new), (1, 0));
        let all = repo.fixture.all();
        assert_eq!(all.len(), 2);
        assert!(all[1].in_tree && all[1].in_history);
        let both = repo.fixture.findings().occurrences(all[1].id).unwrap();
        assert_eq!(both.iter().map(|place| place.commit_id.len()).collect::<Vec<_>>(), vec![0, 40]);
        assert_eq!((both[0].author.as_str(), both[0].authored_at), ("", None));
        let history = repo.fixture.findings().total(&Query { scope: Scope::History, ..Query::default() }).unwrap();
        assert_eq!(history, 2);

        let mut checked = 0;
        for entry in fs::read_dir(repo.fixture.directory.path()).unwrap() {
            let path = entry.unwrap().path();
            if path.is_file() {
                checked += 1;
                let bytes = fs::read(&path).unwrap();
                assert!(
                    !String::from_utf8_lossy(&bytes).contains(&sample_hosting_token()),
                    "{} holds a secret",
                    path.display()
                );
            }
        }
        assert!(checked >= 1);
    }

    #[test]
    fn a_rewritten_history_marks_its_findings_as_gone_from_the_history() {
        let repo = repo("scans-rewritten");
        repo.scan();
        repo.fixture.scan();
        let tree = repo.git.tree(&[("100644", "app.py", &repo.app)]);
        let fresh = repo.git.commit(&tree, &[], 1_700_000_300, "start again");
        repo.git.set_reference("refs/heads/main", &fresh);
        let again = repo.scan();
        assert_eq!((again.scan.commits_scanned, again.scan.files_scanned, again.scan.findings_seen), (1, 1, 0));
        let all = repo.fixture.all();
        assert!(!all[0].in_history && !all[0].in_tree);
        assert_eq!(all[0].state_label(), "No longer found");
        assert!(!all[1].in_history && all[1].in_tree);
        assert_eq!(all[1].state_label(), "New");
        assert_eq!(repo.fixture.scans().list(Some(Kind::History), 10).unwrap().len(), 2);
    }

    #[test]
    fn a_history_scan_needs_a_repository_and_reports_what_it_could_not_read() {
        let plain = seeded("scans-no-repository");
        let refused = plain
            .scans()
            .scan_history(&plain.root(), &git::HistoryOptions::default(), &AtomicBool::new(false))
            .unwrap_err()
            .to_string();
        assert!(refused.starts_with("path: ") && refused.contains("is not a Git repository"), "{refused}");
        assert_eq!(plain.scans().count().unwrap(), 0);

        let repo = repo("scans-shallow");
        fs::remove_file(repo.git.object_path(&repo.first)).unwrap();
        let shallow = repo.scan();
        assert_eq!(shallow.scan.status, Status::Completed);
        assert_eq!(shallow.scan.commits_scanned, 2);
        assert_eq!(
            shallow.scan.problem,
            "1 objects are not in the repository, which is usual for a shallow or partial clone."
        );
        assert_eq!(shallow.scan.findings_seen, 1);

        let limited = repo
            .fixture
            .scans()
            .scan_history(
                &repo.fixture.root(),
                &git::HistoryOptions { max_commits: 1, ..git::HistoryOptions::default() },
                &AtomicBool::new(false),
            )
            .unwrap();
        assert!(
            limited.scan.problem.starts_with("Stopped at the limit of 1 commits or 5000000 objects"),
            "{}",
            limited.scan.problem
        );
        assert!(repo.fixture.all().iter().all(|finding| finding.in_history));

        let cancelled = repo
            .fixture
            .scans()
            .scan_history(&repo.fixture.root(), &git::HistoryOptions::default(), &AtomicBool::new(true))
            .unwrap();
        assert_eq!(cancelled.scan.status, Status::Cancelled);
        assert!(repo.fixture.all().iter().all(|finding| finding.in_history));
    }

    #[test]
    fn history_findings_are_grouped_by_the_commit_that_added_them() {
        let repo = repo("scans-commits");
        repo.scan();
        let groups = repo.fixture.findings().commits(50).unwrap();
        assert_eq!(
            groups.iter().map(|group| group.subject.as_str()).collect::<Vec<_>>(),
            vec!["add the deploy script", "add configuration"]
        );
        assert_eq!(groups[1].commit_id, repo.first);
        assert_eq!(groups[1].short_id(), &repo.first[..10]);
        assert_eq!((groups[1].findings, groups[1].attention, groups[1].author.as_str()), (1, 1, "Sample Author"));
        assert_eq!(groups[1].authored_at, Some(Timestamp::from_unix(1_700_000_000)));
        assert_eq!(groups[1].top_score, repo.fixture.all()[0].score);
        for group in &groups {
            let query = Query { commit_id: group.commit_id.clone(), scope: Scope::History, ..Query::default() };
            assert_eq!(repo.fixture.findings().total(&query).unwrap(), group.findings);
        }
        let first = repo.fixture.all()[0].clone();
        repo.fixture.findings().decide(first.id, first.revision, Decision::FalsePositive, "").unwrap();
        assert_eq!(repo.fixture.findings().commits(50).unwrap()[1].attention, 0);
        assert_eq!(repo.fixture.findings().commits(1).unwrap().len(), 1);
        let by_scan = Query { scan_id: repo.fixture.scans().latest().unwrap().unwrap().id, ..Query::default() };
        assert_eq!(repo.fixture.findings().total(&by_scan).unwrap(), 2);
        assert_eq!(repo.fixture.findings().total(&Query { scan_id: 999, ..Query::default() }).unwrap(), 0);
    }

    #[test]
    fn a_baseline_file_carries_known_findings_to_another_database() {
        let first = seeded("scans-baseline-writer");
        let mut baseline = Baseline::with_key(&[9u8; 32]);
        first.hands.advance(60);
        let written = first
            .scans()
            .with_baseline(&baseline)
            .scan_tree(&first.root(), &tree::Options::default(), &AtomicBool::new(false))
            .unwrap();
        assert_eq!((written.scan.findings_new, written.portable.len()), (2, 2));
        assert!(first.all().iter().all(|finding| !finding.baseline));
        baseline.replace_entries(written.portable.iter().cloned()).unwrap();
        let file = first.directory.path().join("baseline.json");
        baseline.save(&file, false).unwrap();
        let stored = fs::read_to_string(&file).unwrap();
        assert!(!stored.contains(&sample_cloud_key()) && !stored.contains(&sample_hosting_token()));

        let second = seeded("scans-baseline-reader");
        second.write("package/.npmrc", &format!("//registry/:_authToken={}\n", sample_package_token()));
        let loaded = Baseline::load(&file).unwrap();
        second.hands.advance(60);
        let outcome = second
            .scans()
            .with_baseline(&loaded)
            .scan_tree(&second.root(), &tree::Options::default(), &AtomicBool::new(false))
            .unwrap();
        assert_eq!((outcome.scan.findings_seen, outcome.scan.findings_new), (3, 1));
        assert_eq!(outcome.recorded.top_new_severity, Some(Severity::High));
        assert_eq!(outcome.portable.len(), 3);
        let all = second.all();
        assert_eq!(
            all.iter().map(|finding| (finding.path.as_str(), finding.baseline)).collect::<Vec<_>>(),
            vec![("deploy/ci.yaml", true), ("package/.npmrc", false), ("src/config.py", true)]
        );
        assert_eq!(second.findings().total(&Query { scope: Scope::New, ..Query::default() }).unwrap(), 1);
        assert_eq!(second.notifications()[0].title, "1 new likely secret in project");
        assert_ne!(first.all()[0].fingerprint, all[0].fingerprint);

        let third = seeded("scans-baseline-later");
        third.scan();
        assert!(third.all().iter().all(|finding| !finding.baseline));
        third.hands.advance(60);
        third
            .scans()
            .with_baseline(&loaded)
            .scan_tree(&third.root(), &tree::Options::default(), &AtomicBool::new(false))
            .unwrap();
        assert!(third.all().iter().all(|finding| finding.baseline));
        let plain = third.scan();
        assert!(plain.portable.is_empty());
        assert!(third.all().iter().all(|finding| finding.baseline));
    }

    #[test]
    fn demo_data_is_labelled_loaded_once_and_removed_alone() {
        let fixture = seeded("scans-demo");
        fixture.scan();
        let scans = fixture.scans();
        assert_eq!(scans.demo_count().unwrap(), 0);
        assert_eq!(scans.load_demo().unwrap(), 8);
        assert_eq!(scans.load_demo().unwrap(), 0);
        let demo: Vec<findings::Finding> = fixture.all().into_iter().filter(|finding| finding.demo).collect();
        let mut pairs: Vec<(&str, &str)> =
            demo.iter().map(|finding| (finding.path.as_str(), finding.rule_id.as_str())).collect();
        pairs.sort_unstable();
        assert_eq!(
            pairs,
            vec![
                (".env", "npm-token"),
                (".npmrc", "npm-token"),
                ("config/settings.py", "aws-access-key-id"),
                ("config/settings.py", "aws-secret-access-key"),
                ("demo-fixtures/payment_test.py", "stripe-secret-key"),
                ("deploy/release.sh", "github-token"),
                ("docs/setup.md", "password-in-url"),
                ("src/notify.py", "slack-webhook-url"),
            ]
        );
        let by_path = |path: &str| demo.iter().find(|finding| finding.path == path).unwrap();
        assert!(demo.iter().all(|finding| finding.root == demo::ROOT));
        assert!(by_path(".env").in_history && !by_path(".env").in_tree);
        assert!(by_path("config/settings.py").in_history && by_path("config/settings.py").in_tree);
        assert!(by_path("deploy/release.sh").in_history && by_path("deploy/release.sh").in_tree);
        assert!(!by_path(".npmrc").in_history);
        assert!(matches!(by_path("demo-fixtures/payment_test.py").allowed, Allowed::Entry(_)));
        assert!(by_path("docs/setup.md").score < by_path(".npmrc").score);
        assert_eq!(by_path(".npmrc").level(), Level::High);
        let listed = scans.list(None, 10).unwrap();
        assert_eq!(
            listed.iter().filter(|scan| scan.demo).map(|scan| scan.kind).collect::<Vec<_>>(),
            vec![Kind::History, Kind::Tree]
        );
        assert_eq!(listed[0].scope_label(), "3 commits, 4 file versions");
        assert_eq!((listed[1].files_scanned, listed[1].findings_seen, listed[1].findings_new), (7, 7, 6));
        let groups = fixture.findings().commits(10).unwrap();
        assert_eq!(
            groups.iter().map(|group| (group.subject.as_str(), group.findings)).collect::<Vec<_>>(),
            vec![("Add the release script", 1), ("Add the deployment settings", 3)]
        );
        assert!(groups.iter().all(|group| group.author == "Demo Author" && group.root == demo::ROOT));
        assert_eq!(fixture.notifications().len(), 1);
        let entries = Allowlists::new(fixture.store.conn(), fixture.clock.clone()).list().unwrap();
        assert_eq!(entries.iter().filter(|entry| entry.demo).count(), 1);

        assert_eq!(scans.remove_demo(true).unwrap(), 8);
        assert_eq!(scans.demo_count().unwrap(), 10);
        assert_eq!(scans.remove_demo(false).unwrap(), 8);
        assert_eq!(scans.demo_count().unwrap(), 0);
        assert_eq!(scans.remove_demo(false).unwrap(), 0);
        assert_eq!(fixture.all().len(), 2);
        assert_eq!(scans.count().unwrap(), 1);
        assert!(fixture.findings().commits(10).unwrap().is_empty());
        assert_eq!(Allowlists::new(fixture.store.conn(), fixture.clock.clone()).count().unwrap(), 0);
        assert_eq!(scans.load_demo().unwrap(), 8);
    }

    #[test]
    fn durations_sizes_and_names_are_shown_in_plain_units() {
        assert_eq!(show_duration(0), "0 ms");
        assert_eq!(show_duration(999), "999 ms");
        assert_eq!(show_duration(1250), "1.2 s");
        assert_eq!(show_duration(59_999), "59.9 s");
        assert_eq!(show_duration(125_000), "2 min 5 s");
        assert_eq!(show_duration(-5), "0 ms");
        assert_eq!(show_bytes(0), "0 B");
        assert_eq!(show_bytes(1023), "1023 B");
        assert_eq!(show_bytes(1536), "1.5 KiB");
        assert_eq!(show_bytes(5 * 1024 * 1024 + 512 * 1024), "5.5 MiB");
        assert_eq!(show_bytes(3 * 1024 * 1024 * 1024), "3.0 GiB");
        assert_eq!(short_name("/home/dev/project"), "project");
        assert_eq!(short_name("C:\\work\\project\\"), "project");
        assert_eq!(short_name("/"), "/");
        assert_eq!(clip(&"x".repeat(MAX_PROBLEM_LENGTH + 5)).len(), MAX_PROBLEM_LENGTH);
        for kind in Kind::ALL {
            assert_eq!(Kind::parse(kind.as_str()), Some(kind));
            assert!(!kind.label().is_empty());
        }
        for status in Status::ALL {
            assert_eq!(Status::parse(status.as_str()), Some(status));
            assert!(!status.label().is_empty());
        }
    }

    #[test]
    fn hot_queries_use_an_index() {
        let fixture = Fixture::new("scans-plan");
        for (name, sql, parameters) in HOT_QUERIES.iter().chain(findings::HOT_QUERIES) {
            let plan = fixture.store.query_plan(sql, *parameters).unwrap().join(" | ");
            assert!(plan.contains("USING") && !plan.contains("SCAN"), "{name}: {plan}");
        }
    }
}
