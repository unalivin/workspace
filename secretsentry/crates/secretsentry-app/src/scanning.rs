use std::collections::BTreeSet;
use std::path::Path;
use std::sync::atomic::AtomicBool;

use app_core::baseline::Baseline;
use app_core::confidence::Level;
use app_core::export::{self, Format, Item};
use app_core::findings::{Finding, Query, Scope};
use app_core::git::HistoryOptions;
use app_core::meta;
use app_core::scans::{self, Scan};
use app_core::service::App;
use app_core::settings;
use app_core::tree;
use app_core::words;
use app_core::{AppError, Result};
use serde::Serialize;

use crate::cli::{Call, Command, EXIT_FINDINGS, Env, number, switch, text, usage};

pub(crate) fn level_text(finding: &Finding) -> String {
    format!("{} {}", finding.level().label(), finding.score)
}

pub(crate) fn findings_table(env: &mut Env<'_>, items: &[Item]) {
    let rows: Vec<Vec<String>> = items
        .iter()
        .map(|item| {
            let finding = &item.finding;
            let place = item.occurrences.first().map_or_else(String::new, |place| {
                if place.commit_id.is_empty() {
                    place.line.to_string()
                } else {
                    format!("{} in {}", place.line, &place.commit_id[..place.commit_id.len().min(10)])
                }
            });
            vec![
                finding.id.to_string(),
                level_text(finding),
                finding.rule_name.clone(),
                finding.path.clone(),
                place,
                finding.preview.clone(),
                finding.state_label().to_string(),
                finding.status.label().to_string(),
            ]
        })
        .collect();
    env.table(&["ID", "CONFIDENCE", "RULE", "PATH", "LINE", "PREVIEW", "STATE", "DECISION"], &rows);
}

#[derive(Serialize)]
struct RuleCountJson {
    rule_id: String,
    rule_name: String,
    low: i64,
    medium: i64,
    high: i64,
}

#[derive(Serialize)]
struct FileCountJson {
    root: String,
    path: String,
    findings: i64,
    top_score: u8,
}

#[derive(Serialize)]
struct ScanJson {
    id: i64,
    kind: &'static str,
    root: String,
    status: &'static str,
    started_at: String,
    finished_at: Option<String>,
    duration_ms: i64,
    files_scanned: i64,
    files_skipped: i64,
    bytes_scanned: i64,
    commits_scanned: i64,
    rules_used: i64,
    findings_seen: i64,
    findings_new: i64,
    problem: String,
    demo: bool,
}

fn scan_json(scan: &Scan) -> ScanJson {
    ScanJson {
        id: scan.id,
        kind: scan.kind.as_str(),
        root: scan.root.clone(),
        status: scan.status.as_str(),
        started_at: scan.started_at.to_rfc3339(),
        finished_at: scan.finished_at.map(|moment| moment.to_rfc3339()),
        duration_ms: scan.duration_ms,
        files_scanned: scan.files_scanned,
        files_skipped: scan.files_skipped,
        bytes_scanned: scan.bytes_scanned,
        commits_scanned: scan.commits_scanned,
        rules_used: scan.rules_used,
        findings_seen: scan.findings_seen,
        findings_new: scan.findings_new,
        problem: scan.problem.clone(),
        demo: scan.demo,
    }
}

#[derive(Serialize)]
struct OverviewJson {
    needs_attention: i64,
    new_findings: i64,
    baseline_findings: i64,
    allowlisted_findings: i64,
    decided_findings: i64,
    gone_findings: i64,
    demo_findings: i64,
    scans: i64,
    failed_scans: i64,
    unread_notifications: i64,
    by_rule: Vec<RuleCountJson>,
    top_files: Vec<FileCountJson>,
    last_scan: Option<ScanJson>,
}

fn run_overview(env: &mut Env<'_>, call: &Call, app: &mut App) -> Result<()> {
    call.no_arguments()?;
    let overview = app.findings().overview()?;
    let last = app.scans().latest()?;
    match call.text("format") {
        "json" => {
            return env.json(&OverviewJson {
                needs_attention: overview.attention,
                new_findings: overview.new,
                baseline_findings: overview.baseline,
                allowlisted_findings: overview.allowlisted,
                decided_findings: overview.decided,
                gone_findings: overview.gone,
                demo_findings: overview.demo,
                scans: overview.scans,
                failed_scans: overview.failed_scans,
                unread_notifications: overview.unread_notifications,
                by_rule: overview
                    .by_rule
                    .iter()
                    .map(|row| RuleCountJson {
                        rule_id: row.rule_id.clone(),
                        rule_name: row.rule_name.clone(),
                        low: row.low,
                        medium: row.medium,
                        high: row.high,
                    })
                    .collect(),
                top_files: overview
                    .top_files
                    .iter()
                    .map(|row| FileCountJson {
                        root: row.root.clone(),
                        path: row.path.clone(),
                        findings: row.total,
                        top_score: row.top_score,
                    })
                    .collect(),
                last_scan: last.as_ref().map(scan_json),
            });
        }
        "table" => {}
        other => return Err(usage(format!("unknown format \"{other}\": use table or json"))),
    }
    env.table(
        &["MEASURE", "COUNT"],
        &[
            vec!["Findings that need attention".to_string(), overview.attention.to_string()],
            vec!["New findings".to_string(), overview.new.to_string()],
            vec!["Findings in the baseline".to_string(), overview.baseline.to_string()],
            vec!["Allowlisted findings".to_string(), overview.allowlisted.to_string()],
            vec!["Decided findings".to_string(), overview.decided.to_string()],
            vec!["Findings no longer found".to_string(), overview.gone.to_string()],
            vec!["Failed scans".to_string(), overview.failed_scans.to_string()],
            vec!["Unread notifications".to_string(), overview.unread_notifications.to_string()],
        ],
    );
    let Some(last) = last else {
        env.line(format!(
            "\nNo scan yet. Scan a directory with \"{slug} scan <directory>\" or load samples with \"{slug} demo load\".",
            slug = meta::SLUG
        ));
        return Ok(());
    };
    env.line(format!(
        "\nLast scan: {} of {}, {}, {} in {}, {}",
        last.kind.label(),
        last.root,
        last.status.label().to_lowercase(),
        last.scope_label(),
        last.duration_label(),
        app.format_time(last.started_at)
    ));
    if !overview.by_rule.is_empty() {
        env.line("\nFindings that need attention by rule and confidence");
        let rows: Vec<Vec<String>> = overview
            .by_rule
            .iter()
            .map(|row| {
                vec![
                    row.rule_name.clone(),
                    row.high.to_string(),
                    row.medium.to_string(),
                    row.low.to_string(),
                    row.total().to_string(),
                ]
            })
            .collect();
        env.table(&["RULE", "HIGH", "MEDIUM", "LOW", "TOTAL"], &rows);
    }
    if !overview.top_files.is_empty() {
        env.line("\nFiles with the most findings");
        let rows: Vec<Vec<String>> = overview
            .top_files
            .iter()
            .map(|row| vec![row.path.clone(), row.total.to_string(), row.top_score.to_string(), row.root.clone()])
            .collect();
        env.table(&["PATH", "FINDINGS", "TOP SCORE", "SCANNED DIRECTORY"], &rows);
    }
    if overview.demo > 0 {
        env.line(format!(
            "\nDemo data is loaded: {} invented findings belong to the scanned directory \"{}\".",
            overview.demo,
            app_core::demo::ROOT
        ));
    }
    Ok(())
}

pub(crate) fn overview_command() -> Command {
    Command::leaf("overview", "Show what needs attention", run_overview).flags(&[text(
        "format",
        "table",
        "table or json",
    )])
}

fn positive(call: &Call, flag: &str, fallback: i64) -> Result<i64> {
    if !call.given(flag) {
        return Ok(fallback);
    }
    match call.number(flag) {
        value if value > 0 => Ok(value),
        _ => Err(usage(format!("flag --{flag} needs a number above zero"))),
    }
}

fn threshold(raw: &str) -> Result<Option<Level>> {
    match raw {
        "never" => Ok(None),
        other => Level::parse(other).map(Some).ok_or_else(|| {
            usage(format!("unknown confidence \"{other}\" for --fail-on: use low, medium, high or never"))
        }),
    }
}

fn merged(app: &App, scan_ids: &[i64]) -> Result<Vec<Item>> {
    let mut seen = BTreeSet::new();
    let mut items = Vec::new();
    for scan_id in scan_ids {
        let query = Query { scan_id: *scan_id, limit: app_core::findings::MAX_LIMIT, ..Query::default() };
        for item in export::collect(&app.findings(), &query)? {
            if seen.insert(item.finding.id) {
                items.push(item);
            }
        }
    }
    items.sort_by(|first, second| {
        second
            .finding
            .score
            .cmp(&first.finding.score)
            .then_with(|| first.finding.path.cmp(&second.finding.path))
            .then_with(|| first.finding.id.cmp(&second.finding.id))
    });
    Ok(items)
}

fn write_report(env: &mut Env<'_>, call: &Call, app: &App, format: Format, items: &[Item]) -> Result<()> {
    let output = call.text("output");
    if output.is_empty() {
        let text = export::render(format, items, app.now())?;
        env.print(text);
        return Ok(());
    }
    match export::write(Path::new(output), format, items, app.now(), call.truth("force")) {
        Ok(count) => {
            env.note(format!("Wrote {} to {output}", words::count(count as i64, "finding", "findings")));
            Ok(())
        }
        Err(AppError::Conflict(_)) => {
            Err(usage(format!("{output} already exists: choose another name or add --force to overwrite it")))
        }
        Err(error) => Err(error),
    }
}

fn run_scan(env: &mut Env<'_>, call: &Call, app: &mut App) -> Result<()> {
    let [directory] = call.args.as_slice() else {
        return Err(usage("give exactly one directory to scan"));
    };
    let (files, history) = match call.text("source") {
        "files" => (true, false),
        "history" => (false, true),
        "all" => (true, true),
        other => return Err(usage(format!("unknown source \"{other}\": use files, history or all"))),
    };
    let report = match call.text("format") {
        "table" => None,
        other => Some(
            Format::parse(other).map_err(|_| usage(format!("unknown format \"{other}\": use table, json or sarif")))?,
        ),
    };
    if report.is_none() && call.given("output") {
        return Err(usage("--output needs --format json or --format sarif"));
    }
    let output = call.text("output");
    if !output.is_empty() && Path::new(output).exists() && !call.truth("force") {
        return Err(usage(format!("{output} already exists: choose another name or add --force to overwrite it")));
    }
    let fail_on = threshold(call.text("fail-on"))?;
    let max_file_kib = positive(call, "max-file-kib", app.settings().number(settings::KEY_SCAN_MAX_FILE_KIB)?)?;
    let max_commits = positive(call, "max-commits", app.settings().number(settings::KEY_SCAN_MAX_COMMITS)?)?;
    let max_bytes = u64::try_from(max_file_kib).unwrap_or(1024).saturating_mul(1024);
    let known_path = call.text("baseline");
    let write_path = call.text("write-baseline");
    if !write_path.is_empty() && write_path != known_path && Path::new(write_path).exists() && !call.truth("force") {
        return Err(usage(format!("{write_path} already exists: choose another name or add --force to overwrite it")));
    }
    let mut baseline = if !known_path.is_empty() {
        Some(Baseline::load(Path::new(known_path))?)
    } else if !write_path.is_empty() {
        Some(Baseline::fresh()?)
    } else {
        None
    };
    let cancel = AtomicBool::new(false);
    let root = Path::new(directory);
    let mut finished: Vec<Scan> = Vec::new();
    let mut portable: BTreeSet<String> = BTreeSet::new();
    {
        let scans = match &baseline {
            Some(known) => app.scans().with_baseline(known),
            None => app.scans(),
        };
        if files {
            let options = tree::Options { max_file_bytes: max_bytes, ..tree::Options::default() };
            let outcome = scans.scan_tree(root, &options, &cancel)?;
            portable.extend(outcome.portable);
            finished.push(outcome.scan);
        }
        if history {
            let options = HistoryOptions {
                max_blob_bytes: max_bytes,
                max_commits: u64::try_from(max_commits).unwrap_or(u64::MAX),
            };
            let outcome = scans.scan_history(root, &options, &cancel)?;
            portable.extend(outcome.portable);
            finished.push(outcome.scan);
        }
    }
    if let Some(written) = baseline.as_mut().filter(|_| !write_path.is_empty()) {
        written.replace_entries(portable)?;
        written.save(Path::new(write_path), true)?;
        env.note(format!(
            "Wrote a baseline with {} to {write_path}",
            words::count(written.len() as i64, "finding", "findings")
        ));
        for scan in &finished {
            app.findings().accept_scan(scan.id)?;
        }
    }
    let scan_ids: Vec<i64> = finished.iter().map(|scan| scan.id).collect();
    let items = merged(app, &scan_ids)?;
    for scan in finished.iter().filter(|scan| !scan.problem.is_empty()) {
        env.note(format!("note: {}", scan.problem));
    }
    match report {
        Some(format) => write_report(env, call, app, format, &items)?,
        None => {
            for scan in &finished {
                env.line(format!(
                    "{}: {}, {} in {}, {} skipped, {} rules",
                    scan.kind.label(),
                    scan.status.label().to_lowercase(),
                    scan.scope_label(),
                    scan.duration_label(),
                    scan.files_skipped,
                    scan.rules_used
                ));
            }
            let attention = items.iter().filter(|item| item.finding.needs_attention()).count();
            let new = items.iter().filter(|item| item.finding.needs_attention() && !item.finding.baseline).count();
            if items.is_empty() {
                env.line(format!(
                    "No likely secrets found in {}.",
                    finished.first().map_or(directory.as_str(), |scan| scan.root.as_str())
                ));
            } else {
                env.line(format!(
                    "{} in {}: {} need attention, {} of them new\n",
                    words::count(items.len() as i64, "finding", "findings"),
                    finished.first().map_or(directory.as_str(), |scan| scan.root.as_str()),
                    attention,
                    new
                ));
                findings_table(env, &items);
            }
        }
    }
    if let Some(level) = fail_on {
        let failing = items
            .iter()
            .filter(|item| {
                item.finding.needs_attention() && !item.finding.baseline && item.finding.score >= level.lowest_score()
            })
            .count();
        if failing > 0 {
            env.note(format!(
                "{} with {} confidence or higher: exit code {EXIT_FINDINGS}",
                words::count(failing as i64, "new finding", "new findings"),
                level.as_str()
            ));
            env.exit_code = EXIT_FINDINGS;
        }
    }
    Ok(())
}

pub(crate) fn scan_command() -> Command {
    Command::leaf("scan", "Scan a directory, its Git history or both for likely secrets", run_scan)
        .args("<directory>")
        .flags(&[
            text("source", "files", "what to scan: files, history or all"),
            text("baseline", "", "baseline file whose findings are known and do not count as new"),
            text("write-baseline", "", "file that receives the findings of this scan as a baseline"),
            text("format", "table", "table, json or sarif"),
            text("output", "", "file for the json or sarif report, by default standard output"),
            switch("force", "overwrite the report or baseline file if it exists"),
            text(
                "fail-on",
                "low",
                "lowest confidence of a new finding that gives exit code 3: low, medium, high or never",
            ),
            number("max-file-kib", "0", "largest file to read in KiB, by default the setting scan.max_file_kib"),
            number("max-commits", "0", "largest number of commits to read, by default the setting scan.max_commits"),
        ])
}

fn scan_rows(app: &App, scan: &Scan) -> Vec<Vec<String>> {
    vec![
        vec!["ID".to_string(), scan.id.to_string()],
        vec!["Kind".to_string(), scan.kind.label().to_string()],
        vec![
            "Scanned directory".to_string(),
            if scan.demo { format!("{} (demo data)", scan.root) } else { scan.root.clone() },
        ],
        vec!["Status".to_string(), scan.status.label().to_string()],
        vec!["Started".to_string(), app.format_time(scan.started_at)],
        vec![
            "Finished".to_string(),
            scan.finished_at.map_or_else(|| "not finished".to_string(), |moment| app.format_time(moment)),
        ],
        vec!["Duration".to_string(), scan.duration_label()],
        vec!["Scope".to_string(), scan.scope_label()],
        vec!["Skipped files".to_string(), scan.files_skipped.to_string()],
        vec!["Rules used".to_string(), scan.rules_used.to_string()],
        vec!["Findings seen".to_string(), scan.findings_seen.to_string()],
        vec!["New findings".to_string(), scan.findings_new.to_string()],
        vec!["Problem".to_string(), if scan.problem.is_empty() { "none".to_string() } else { scan.problem.clone() }],
    ]
}

fn run_scans_list(env: &mut Env<'_>, call: &Call, app: &mut App) -> Result<()> {
    call.no_arguments()?;
    let kind = match call.text("kind") {
        "" => None,
        other => Some(
            scans::Kind::parse(other).ok_or_else(|| usage(format!("unknown kind \"{other}\": use tree or history")))?,
        ),
    };
    let list = app.scans().list(kind, call.number("limit"))?;
    match call.text("format") {
        "json" => return env.json(&list.iter().map(scan_json).collect::<Vec<_>>()),
        "table" => {}
        other => return Err(usage(format!("unknown format \"{other}\": use table or json"))),
    }
    if list.is_empty() {
        env.line(format!("No scans yet. Start one with \"{} scan <directory>\".", meta::SLUG));
        return Ok(());
    }
    let rows: Vec<Vec<String>> = list
        .iter()
        .map(|scan| {
            vec![
                scan.id.to_string(),
                app.format_time(scan.started_at),
                scan.kind.label().to_string(),
                scan.status.label().to_string(),
                scan.scope_label(),
                scan.duration_label(),
                scan.findings_seen.to_string(),
                scan.findings_new.to_string(),
                scan.root.clone(),
            ]
        })
        .collect();
    env.table(&["ID", "STARTED", "KIND", "STATUS", "SCOPE", "DURATION", "FINDINGS", "NEW", "SCANNED DIRECTORY"], &rows);
    Ok(())
}

fn run_scans_show(env: &mut Env<'_>, call: &Call, app: &mut App) -> Result<()> {
    let id = call.identifier("scan")?;
    let scan = app.scans().get(id)?;
    match call.text("format") {
        "json" => env.json(&scan_json(&scan)),
        "table" => {
            let rows = scan_rows(app, &scan);
            env.table(&["FIELD", "VALUE"], &rows);
            Ok(())
        }
        other => Err(usage(format!("unknown format \"{other}\": use table or json"))),
    }
}

pub(crate) fn scans_command() -> Command {
    Command::group(
        "scans",
        "See earlier scan runs",
        vec![
            Command::leaf("list", "List scan runs, newest first", run_scans_list).flags(&[
                text("kind", "", "only tree or only history scans"),
                number("limit", "100", "largest number of rows"),
                text("format", "table", "table or json"),
            ]),
            Command::leaf("show", "Show one scan run", run_scans_show).args("<id>").flags(&[text(
                "format",
                "table",
                "table or json",
            )]),
        ],
    )
}

#[derive(Serialize)]
struct CommitJson {
    root: String,
    commit: String,
    author: String,
    authored_at: Option<String>,
    subject: String,
    findings: i64,
    needs_attention: i64,
    top_score: u8,
}

fn run_history_list(env: &mut Env<'_>, call: &Call, app: &mut App) -> Result<()> {
    call.no_arguments()?;
    let groups = app.findings().commits(call.number("limit"))?;
    match call.text("format") {
        "json" => {
            return env.json(
                &groups
                    .iter()
                    .map(|group| CommitJson {
                        root: group.root.clone(),
                        commit: group.commit_id.clone(),
                        author: group.author.clone(),
                        authored_at: group.authored_at.map(|moment| moment.to_rfc3339()),
                        subject: group.subject.clone(),
                        findings: group.findings,
                        needs_attention: group.attention,
                        top_score: group.top_score,
                    })
                    .collect::<Vec<_>>(),
            );
        }
        "table" => {}
        other => return Err(usage(format!("unknown format \"{other}\": use table or json"))),
    }
    if groups.is_empty() {
        env.line(format!(
            "No findings in Git history yet. Scan a repository with \"{} scan <directory> --source history\".",
            meta::SLUG
        ));
        return Ok(());
    }
    let rows: Vec<Vec<String>> = groups
        .iter()
        .map(|group| {
            vec![
                group.short_id().to_string(),
                group.authored_at.map_or_else(|| "unknown".to_string(), |moment| app.format_time(moment)),
                group.author.clone(),
                group.findings.to_string(),
                group.attention.to_string(),
                group.top_score.to_string(),
                group.subject.clone(),
            ]
        })
        .collect();
    env.table(&["COMMIT", "AUTHORED", "AUTHOR", "FINDINGS", "NEED ATTENTION", "TOP SCORE", "SUBJECT"], &rows);
    env.line(format!("\nList the findings of a commit with \"{} findings list --commit <commit>\".", meta::SLUG));
    Ok(())
}

pub(crate) fn history_command() -> Command {
    Command::group(
        "history",
        "See findings in Git history by the commit that added them",
        vec![
            Command::leaf("list", "List commits that added findings, newest first", run_history_list)
                .flags(&[number("limit", "200", "largest number of rows"), text("format", "table", "table or json")]),
        ],
    )
}

fn run_baseline_status(env: &mut Env<'_>, call: &Call, app: &mut App) -> Result<()> {
    call.no_arguments()?;
    let root = call.text("root").to_string();
    let findings = app.findings();
    let new = findings.total(&Query { scope: Scope::New, root: root.clone(), ..Query::default() })?;
    let known = findings.total(&Query { scope: Scope::Baseline, root, ..Query::default() })?;
    env.table(
        &["MEASURE", "COUNT"],
        &[
            vec!["New findings that need attention".to_string(), new.to_string()],
            vec!["Baseline findings that need attention".to_string(), known.to_string()],
        ],
    );
    Ok(())
}

fn run_baseline_accept(env: &mut Env<'_>, call: &Call, app: &mut App) -> Result<()> {
    call.no_arguments()?;
    let changed = app.findings().accept_baseline(call.text("root"))?;
    if changed == 0 {
        env.line("Every finding is already in the baseline.");
    } else {
        env.line(format!(
            "Added {} to the baseline. Findings that appear later count as new.",
            words::count(changed as i64, "finding", "findings")
        ));
    }
    Ok(())
}

fn run_baseline_reset(env: &mut Env<'_>, call: &Call, app: &mut App) -> Result<()> {
    call.no_arguments()?;
    let root = call.text("root").to_string();
    let statement = if root.is_empty() {
        "This empties the baseline: every finding counts as new again. Decisions and notes stay.".to_string()
    } else {
        format!("This empties the baseline of {root}: its findings count as new again. Decisions and notes stay.")
    };
    if call.truth("dry-run") {
        env.line(format!("Dry run: {statement} Nothing was changed."));
        return Ok(());
    }
    env.confirm(call, Some(&*app), &statement)?;
    let changed = app.findings().reset_baseline(&root)?;
    env.line(format!("Removed {} from the baseline", words::count(changed as i64, "finding", "findings")));
    Ok(())
}

pub(crate) fn baseline_command() -> Command {
    let root = text("root", "", "only the findings of this scanned directory");
    Command::group(
        "baseline",
        "Accept the current findings as known, or start again",
        vec![
            Command::leaf("status", "Count new and baseline findings", run_baseline_status).flags(&[root]),
            Command::leaf("accept", "Mark every current finding as known", run_baseline_accept).flags(&[root]),
            Command::leaf("reset", "Empty the baseline so that every finding counts as new", run_baseline_reset).flags(
                &[
                    root,
                    switch("yes", "do not ask for confirmation"),
                    switch("dry-run", "show what would change without changing it"),
                ],
            ),
        ],
    )
}

pub(crate) const FILTER_FLAGS: [crate::cli::Flag; 10] = [
    text("search", "", "words to find in paths, rule names and notes"),
    text("rule", "", "only findings of this rule identifier"),
    text("confidence", "", "only low, medium or high confidence"),
    text("status", "", "only open, confirmed, false_positive, accepted_risk or resolved"),
    text("scope", "all", "all, attention, new, baseline, allowlisted, decided, gone or history"),
    text("root", "", "only findings of this scanned directory"),
    text("path", "", "only findings in this file path"),
    text("commit", "", "only findings added by this commit"),
    number("scan", "0", "only findings last seen by this scan"),
    number("min-score", "0", "only findings with this score or higher"),
];

pub(crate) fn query_from(call: &Call) -> Result<Query> {
    let level = match call.text("confidence") {
        "" => None,
        other => Some(
            Level::parse(other)
                .ok_or_else(|| usage(format!("unknown confidence \"{other}\": use low, medium or high")))?,
        ),
    };
    let status = match call.text("status") {
        "" => None,
        other => Some(app_core::findings::Status::parse(other).ok_or_else(|| {
            usage(format!("unknown status \"{other}\": use open, confirmed, false_positive, accepted_risk or resolved"))
        })?),
    };
    let scope = Scope::parse(call.text("scope")).ok_or_else(|| {
        usage(format!(
            "unknown scope \"{}\": use all, attention, new, baseline, allowlisted, decided, gone or history",
            call.text("scope")
        ))
    })?;
    let min_score = u8::try_from(call.number("min-score").clamp(0, 100)).unwrap_or(0);
    Ok(Query {
        search: call.text("search").to_string(),
        rule_id: call.text("rule").to_string(),
        level,
        status,
        scope,
        root: call.text("root").to_string(),
        path: call.text("path").to_string(),
        min_score,
        scan_id: call.number("scan"),
        commit_id: call.text("commit").to_string(),
        ..Query::default()
    })
}

fn run_export(env: &mut Env<'_>, call: &Call, app: &mut App) -> Result<()> {
    let [target] = call.args.as_slice() else {
        return Err(usage("give one file to write, or - for standard output"));
    };
    let query = Query {
        limit: app_core::findings::MAX_LIMIT,
        sort: "path".to_string(),
        descending: false,
        ..query_from(call)?
    };
    let items = export::collect(&app.findings(), &query)?;
    if target == "-" {
        let format = Format::parse(if call.given("format") { call.text("format") } else { "json" })?;
        let text = export::render(format, &items, app.now())?;
        env.print(text);
        return Ok(());
    }
    let format = Format::for_file(target, call.text("format"))?;
    match export::write(Path::new(target), format, &items, app.now(), call.truth("force")) {
        Ok(count) => {
            env.line(format!(
                "Exported {} as {} to {target}",
                words::count(count as i64, "finding", "findings"),
                format.label()
            ));
            Ok(())
        }
        Err(AppError::Conflict(_)) => {
            Err(usage(format!("{target} already exists: choose another name or add --force to overwrite it")))
        }
        Err(error) => Err(error),
    }
}

pub(crate) fn export_command() -> Command {
    let mut flags = FILTER_FLAGS.to_vec();
    flags.push(text("format", "", "json or sarif, by default taken from the file extension"));
    flags.push(switch("force", "overwrite the file if it exists"));
    Command::leaf("export", "Write findings as JSON or SARIF with redacted values", run_export)
        .args("<file|->")
        .flags(&flags)
}

pub(crate) fn demo_command() -> Command {
    Command::group(
        "demo",
        "Load or remove labeled demo data",
        vec![
            Command::leaf("load", "Add an invented scan with findings labeled as demo data", |env, call, app| {
                call.no_arguments()?;
                let added = app.scans().load_demo()?;
                if added == 0 {
                    env.line("Demo data is already loaded.");
                } else {
                    env.line(format!(
                        "Added {} demo findings. Their scanned directory is \"{}\".",
                        added,
                        app_core::demo::ROOT
                    ));
                }
                Ok(())
            }),
            Command::leaf("remove", "Remove every demo scan, finding and allowlist entry", |env, call, app| {
                call.no_arguments()?;
                if app.scans().demo_count()? == 0 {
                    env.line("There is no demo data to remove.");
                    return Ok(());
                }
                let count = app.scans().remove_demo(true)?;
                let statement =
                    format!("This removes {count} demo findings with their scans. Your own scans and findings stay.");
                if call.truth("dry-run") {
                    env.line(format!("Dry run: {statement} Nothing was changed."));
                    return Ok(());
                }
                env.confirm(call, Some(&*app), &statement)?;
                let removed = app.scans().remove_demo(false)?;
                env.line(format!("Removed {removed} demo findings"));
                Ok(())
            })
            .flags(&[
                switch("yes", "do not ask for confirmation"),
                switch("dry-run", "show how many findings would be removed"),
            ]),
        ],
    )
}
