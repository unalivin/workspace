use std::path::Path;

use app_core::allow::{self, EntrySpec};
use app_core::allowlists::StoredEntry;
use app_core::entropy;
use app_core::export::{self, Format, Item};
use app_core::files;
use app_core::findings::{self, Status};
use app_core::meta;
use app_core::rulebook::{self, StoredRule};
use app_core::rules::RuleSpec;
use app_core::scanner::Allowed;
use app_core::service::App;
use app_core::theme::Severity;
use app_core::{AppError, Result};
use serde::Serialize;

use crate::cli::{Call, Command, Env, enabled_text, number, switch, text, usage};
use crate::scanning::{FILTER_FLAGS, findings_table, level_text, query_from};

const MAX_SAMPLE_BYTES: u64 = 64 << 10;

fn run_findings_list(env: &mut Env<'_>, call: &Call, app: &mut App) -> Result<()> {
    call.no_arguments()?;
    let sort = call.text("sort").to_string();
    let descending = match call.text("order") {
        "" => matches!(sort.as_str(), "score" | "seen"),
        "asc" => false,
        "desc" => true,
        other => return Err(usage(format!("unknown order \"{other}\": use asc or desc"))),
    };
    let query = findings::Query {
        sort,
        descending,
        limit: call.number("limit"),
        offset: call.number("offset"),
        ..query_from(call)?
    };
    let total = app.findings().total(&query)?;
    let items = export::collect(&app.findings(), &query)?;
    match call.text("format") {
        "table" => {}
        other => {
            let format = Format::parse(other)
                .map_err(|_| usage(format!("unknown format \"{other}\": use table, json or sarif")))?;
            let text = export::render(format, &items, app.now())?;
            env.print(text);
            return Ok(());
        }
    }
    if items.is_empty() {
        if app.findings().count()? == 0 {
            env.line(format!("No findings yet. Scan a directory with \"{} scan <directory>\".", meta::SLUG));
        } else {
            env.line("No findings match. Remove a filter or use --scope all.");
        }
        return Ok(());
    }
    findings_table(env, &items);
    if (items.len() as i64) < total {
        env.line(format!("\nShowing {} of {total} findings. Use --limit and --offset to see more.", items.len()));
    }
    Ok(())
}

fn allowance_text(app: &App, finding: &findings::Finding) -> String {
    match finding.allowed {
        Allowed::No => "no".to_string(),
        Allowed::Marker => format!("yes, by the marker {} on the line", allow::LINE_MARKER),
        Allowed::Entry(id) => match app.allowlists().get(id) {
            Ok(entry) => format!("yes, by entry {id} ({} {})", entry.spec.kind.label().to_lowercase(), entry.spec.pattern),
            Err(_) => format!("yes, by entry {id}, which was removed since"),
        },
    }
}

fn run_findings_show(env: &mut Env<'_>, call: &Call, app: &mut App) -> Result<()> {
    let id = call.identifier("finding")?;
    let finding = app.findings().get(id)?;
    let occurrences = app.findings().occurrences(id)?;
    match call.text("format") {
        "table" => {}
        other => {
            let format = Format::parse(other)
                .map_err(|_| usage(format!("unknown format \"{other}\": use table, json or sarif")))?;
            let text = export::render(format, &[Item { finding, occurrences }], app.now())?;
            env.print(text);
            return Ok(());
        }
    }
    let place = match (finding.in_tree, finding.in_history) {
        (true, true) => "files and Git history",
        (true, false) => "files",
        (false, true) => "Git history",
        (false, false) => "no longer found",
    };
    env.table(
        &["FIELD", "VALUE"],
        &[
            vec!["ID".to_string(), finding.id.to_string()],
            vec!["Rule".to_string(), format!("{} ({})", finding.rule_name, finding.rule_id)],
            vec!["Severity".to_string(), format!("{} {}", finding.severity.glyph(), finding.severity.label())],
            vec!["Confidence".to_string(), level_text(&finding)],
            vec!["Value".to_string(), format!("{} ({} characters, never stored)", finding.preview, finding.length)],
            vec!["Path".to_string(), finding.path.clone()],
            vec![
                "Scanned directory".to_string(),
                if finding.demo { format!("{} (demo data)", finding.root) } else { finding.root.clone() },
            ],
            vec!["Found in".to_string(), place.to_string()],
            vec!["State".to_string(), finding.state_label().to_string()],
            vec!["Decision".to_string(), finding.status.label().to_string()],
            vec!["Note".to_string(), finding.note.replace('\n', " / ")],
            vec!["In the baseline".to_string(), enabled_text(finding.baseline).to_string()],
            vec!["Allowlisted".to_string(), allowance_text(app, &finding)],
            vec!["First seen".to_string(), app.format_time(finding.first_seen_at)],
            vec!["Last seen".to_string(), app.format_time(finding.last_seen_at)],
            vec!["Fingerprint".to_string(), finding.fingerprint.clone()],
            vec!["Revision".to_string(), finding.revision.to_string()],
        ],
    );
    env.line("\nWhy this confidence");
    let reasons: Vec<Vec<String>> =
        finding.reasons.iter().map(|reason| vec![format!("{:+}", reason.effect), reason.text.clone()]).collect();
    env.table(&["EFFECT", "REASON"], &reasons);
    env.line("\nWhere it was seen");
    let places: Vec<Vec<String>> = occurrences
        .iter()
        .map(|place| {
            vec![
                place.line.to_string(),
                place.column.to_string(),
                if place.commit_id.is_empty() {
                    "files".to_string()
                } else {
                    format!(
                        "{} {} {}",
                        &place.commit_id[..place.commit_id.len().min(10)],
                        place.authored_at.map_or_else(String::new, |moment| app.format_time(moment)),
                        place.subject
                    )
                    .trim()
                    .to_string()
                },
                place.context.clone(),
            ]
        })
        .collect();
    env.table(&["LINE", "COLUMN", "SOURCE", "CONTEXT"], &places);
    Ok(())
}

fn run_findings_decide(env: &mut Env<'_>, call: &Call, app: &mut App) -> Result<()> {
    let id = call.identifier("finding")?;
    if !call.given("status") && !call.given("note") {
        return Err(usage("nothing to change: give --status, --note or both"));
    }
    let current = app.findings().get(id)?;
    let status = if call.given("status") {
        Status::parse(call.text("status")).ok_or_else(|| {
            usage(format!(
                "unknown status \"{}\": use open, confirmed, false_positive, accepted_risk or resolved",
                call.text("status")
            ))
        })?
    } else {
        current.status
    };
    let note = if call.given("note") { call.text("note") } else { current.note.as_str() };
    let decided = app.findings().decide(id, current.revision, status, note)?;
    env.line(format!("Finding {} is {} (revision {})", decided.id, decided.status.label().to_lowercase(), decided.revision));
    Ok(())
}

pub(crate) fn findings_command() -> Command {
    let mut list = FILTER_FLAGS.to_vec();
    list.extend([
        text("sort", "score", "score, rule, path, status or seen"),
        text("order", "", "asc or desc, by default desc for score and seen"),
        number("limit", "200", "largest number of rows"),
        number("offset", "0", "rows to skip"),
        text("format", "table", "table, json or sarif"),
    ]);
    Command::group(
        "findings",
        "Review findings and record decisions",
        vec![
            Command::leaf("list", "List, search, filter and sort findings", run_findings_list).flags(&list),
            Command::leaf("show", "Show one finding with its reasons and places", run_findings_show)
                .args("<id>")
                .flags(&[text("format", "table", "table, json or sarif")]),
            Command::leaf("decide", "Record a decision and a note for a finding", run_findings_decide)
                .args("<id>")
                .flags(&[
                    text("status", "", "open, confirmed, false_positive, accepted_risk or resolved"),
                    text("note", "", "free text, up to 2000 characters"),
                ]),
        ],
    )
}

fn rule_identifier(call: &Call) -> Result<&str> {
    match call.args.as_slice() {
        [only] => Ok(only.as_str()),
        _ => Err(usage(format!("give exactly one rule identifier: run \"{} rules list\" to see them", meta::SLUG))),
    }
}

fn origin(rule: &StoredRule) -> &'static str {
    match (rule.spec.builtin, rule.changed) {
        (true, false) => "built-in",
        (true, true) => "built-in, changed",
        (false, _) => "custom",
    }
}

#[derive(Serialize)]
struct RuleJson {
    id: String,
    name: String,
    description: String,
    pattern: String,
    secret_group: usize,
    keywords: Vec<String>,
    entropy: String,
    severity: &'static str,
    confidence: u8,
    enabled: bool,
    builtin: bool,
    changed: bool,
    revision: i64,
}

fn rule_json(rule: &StoredRule) -> RuleJson {
    RuleJson {
        id: rule.spec.id.clone(),
        name: rule.spec.name.clone(),
        description: rule.spec.description.clone(),
        pattern: rule.spec.pattern.clone(),
        secret_group: rule.spec.secret_group,
        keywords: rule.spec.keywords.clone(),
        entropy: entropy::show(rule.spec.entropy_tenths),
        severity: rule.spec.severity.as_str(),
        confidence: rule.spec.confidence,
        enabled: rule.spec.enabled,
        builtin: rule.spec.builtin,
        changed: rule.changed,
        revision: rule.revision,
    }
}

fn run_rules_list(env: &mut Env<'_>, call: &Call, app: &mut App) -> Result<()> {
    call.no_arguments()?;
    let rules = app.rulebook().list()?;
    match call.text("format") {
        "json" => return env.json(&rules.iter().map(rule_json).collect::<Vec<_>>()),
        "table" => {}
        other => return Err(usage(format!("unknown format \"{other}\": use table or json"))),
    }
    let rows: Vec<Vec<String>> = rules
        .iter()
        .map(|rule| {
            vec![
                rule.spec.id.clone(),
                rule.spec.name.clone(),
                format!("{} {}", rule.spec.severity.glyph(), rule.spec.severity.label()),
                rule.spec.confidence.to_string(),
                if rule.spec.entropy_tenths == 0 { "none".to_string() } else { entropy::show(rule.spec.entropy_tenths) },
                enabled_text(rule.spec.enabled).to_string(),
                origin(rule).to_string(),
            ]
        })
        .collect();
    env.table(&["ID", "NAME", "SEVERITY", "CONFIDENCE", "ENTROPY", "ENABLED", "ORIGIN"], &rows);
    Ok(())
}

fn run_rules_show(env: &mut Env<'_>, call: &Call, app: &mut App) -> Result<()> {
    let rule = app.rulebook().get(rule_identifier(call)?)?;
    match call.text("format") {
        "json" => env.json(&rule_json(&rule)),
        "table" => {
            env.table(
                &["FIELD", "VALUE"],
                &[
                    vec!["ID".to_string(), rule.spec.id.clone()],
                    vec!["Name".to_string(), rule.spec.name.clone()],
                    vec!["Description".to_string(), rule.spec.description.clone()],
                    vec!["Pattern".to_string(), rule.spec.pattern.clone()],
                    vec!["Secret group".to_string(), rule.spec.secret_group.to_string()],
                    vec!["Keywords".to_string(), rule.spec.keywords.join(", ")],
                    vec![
                        "Entropy threshold".to_string(),
                        if rule.spec.entropy_tenths == 0 {
                            "none".to_string()
                        } else {
                            format!("{} bits per character", entropy::show(rule.spec.entropy_tenths))
                        },
                    ],
                    vec!["Severity".to_string(), rule.spec.severity.label().to_string()],
                    vec!["Starting confidence".to_string(), rule.spec.confidence.to_string()],
                    vec!["Enabled".to_string(), enabled_text(rule.spec.enabled).to_string()],
                    vec!["Origin".to_string(), origin(&rule).to_string()],
                    vec!["Revision".to_string(), rule.revision.to_string()],
                ],
            );
            Ok(())
        }
        other => Err(usage(format!("unknown format \"{other}\": use table or json"))),
    }
}

const RULE_FLAGS: [crate::cli::Flag; 8] = [
    text("name", "", "name, 1 to 80 characters"),
    text("description", "", "what the rule finds, up to 400 characters"),
    text("pattern", "", "regular expression; it must not match empty text"),
    number("group", "0", "capture group that holds the secret, 0 for the whole match"),
    text("keywords", "", "words of which one must be in a file, separated by commas"),
    text("entropy", "", "lowest entropy of the value in bits per character, for example 3.5; 0 for none"),
    text("severity", "", "info, low, medium, high or critical"),
    number("confidence", "60", "starting confidence from 1 to 100"),
];

fn parse_entropy(raw: &str) -> Result<u32> {
    let problem = || usage(format!("\"{raw}\" is not an entropy threshold: use a number from 0 to 8 with one decimal, for example 3.5"));
    let (whole, tenth) = raw.trim().split_once('.').unwrap_or((raw.trim(), "0"));
    if whole.is_empty() || tenth.len() != 1 || !whole.bytes().chain(tenth.bytes()).all(|byte| byte.is_ascii_digit()) {
        return Err(problem());
    }
    let whole: u32 = whole.parse().map_err(|_| problem())?;
    let tenth: u32 = tenth.parse().map_err(|_| problem())?;
    if whole > 8 {
        return Err(problem());
    }
    Ok(whole * 10 + tenth)
}

fn apply_rule_flags(call: &Call, spec: &mut RuleSpec) -> Result<bool> {
    let mut changed = false;
    if call.given("name") {
        spec.name = call.text("name").to_string();
        changed = true;
    }
    if call.given("description") {
        spec.description = call.text("description").to_string();
        changed = true;
    }
    if call.given("pattern") {
        spec.pattern = call.text("pattern").to_string();
        changed = true;
    }
    if call.given("group") {
        spec.secret_group = usize::try_from(call.number("group")).map_err(|_| usage("flag --group needs a number from 0"))?;
        changed = true;
    }
    if call.given("keywords") {
        spec.keywords =
            call.text("keywords").split(',').map(str::trim).filter(|word| !word.is_empty()).map(str::to_string).collect();
        changed = true;
    }
    if call.given("entropy") {
        spec.entropy_tenths = parse_entropy(call.text("entropy"))?;
        changed = true;
    }
    if call.given("severity") {
        spec.severity = Severity::parse(call.text("severity")).ok_or_else(|| {
            usage(format!("unknown severity \"{}\": use info, low, medium, high or critical", call.text("severity")))
        })?;
        changed = true;
    }
    if call.given("confidence") {
        spec.confidence =
            u8::try_from(call.number("confidence")).map_err(|_| usage("flag --confidence needs a number from 1 to 100"))?;
        changed = true;
    }
    Ok(changed)
}

fn run_rules_add(env: &mut Env<'_>, call: &Call, app: &mut App) -> Result<()> {
    let id = rule_identifier(call)?;
    if !call.given("pattern") {
        return Err(usage("a pattern is required: add --pattern <regular expression>"));
    }
    let mut spec = RuleSpec { id: id.to_string(), name: id.to_string(), enabled: !call.truth("disabled"), ..RuleSpec::default() };
    apply_rule_flags(call, &mut spec)?;
    let created = app.rulebook().create(&spec)?;
    env.line(format!("Added rule {}: {}", created.spec.id, created.spec.name));
    Ok(())
}

fn run_rules_edit(env: &mut Env<'_>, call: &Call, app: &mut App) -> Result<()> {
    let id = rule_identifier(call)?;
    let current = app.rulebook().get(id)?;
    let mut spec = current.spec.clone();
    if !apply_rule_flags(call, &mut spec)? {
        return Err(usage(
            "nothing to change: give at least one of --name, --description, --pattern, --group, --keywords, --entropy, --severity or --confidence",
        ));
    }
    let updated = app.rulebook().update(id, current.revision, &spec)?;
    env.line(format!("Updated rule {} (revision {})", updated.spec.id, updated.revision));
    Ok(())
}

fn run_rules_toggle(env: &mut Env<'_>, call: &Call, app: &mut App, enabled: bool) -> Result<()> {
    let updated = app.rulebook().set_enabled(rule_identifier(call)?, enabled)?;
    env.line(format!("Rule {} is {}", updated.spec.id, if updated.spec.enabled { "enabled" } else { "disabled" }));
    Ok(())
}

fn run_rules_restore(env: &mut Env<'_>, call: &Call, app: &mut App) -> Result<()> {
    let restored = app.rulebook().restore(rule_identifier(call)?)?;
    env.line(format!(
        "Rule {} is back to its built-in state ({})",
        restored.spec.id,
        if restored.spec.enabled { "enabled" } else { "disabled" }
    ));
    Ok(())
}

fn run_rules_remove(env: &mut Env<'_>, call: &Call, app: &mut App) -> Result<()> {
    let id = rule_identifier(call)?;
    let rule = app.rulebook().get(id)?;
    let statement = format!(
        "This removes the rule {} ({}) permanently. Findings it reported stay.",
        rule.spec.id, rule.spec.name
    );
    if call.truth("dry-run") {
        env.line(format!("Dry run: {statement} Nothing was changed."));
        return Ok(());
    }
    if rule.spec.builtin {
        return app.rulebook().delete(id);
    }
    env.confirm(call, Some(&*app), &statement)?;
    app.rulebook().delete(id)?;
    env.line(format!("Removed rule {id}"));
    Ok(())
}

fn run_rules_try(env: &mut Env<'_>, call: &Call, app: &mut App) -> Result<()> {
    let spec = match call.args.as_slice() {
        [] => {
            if !call.given("pattern") {
                return Err(usage("give a rule identifier, or describe a new rule with --pattern"));
            }
            let mut spec = RuleSpec { id: "trial".to_string(), name: "Trial".to_string(), ..RuleSpec::default() };
            apply_rule_flags(call, &mut spec)?;
            spec
        }
        [id] => {
            let mut spec = app.rulebook().get(id)?.spec;
            apply_rule_flags(call, &mut spec)?;
            spec
        }
        _ => return Err(usage("give at most one rule identifier")),
    };
    let sample = match (call.given("text"), call.given("file")) {
        (true, false) => call.text("text").to_string(),
        (false, true) => String::from_utf8(files::read_limited(Path::new(call.text("file")), MAX_SAMPLE_BYTES)?)
            .map_err(|_| AppError::Usage(format!("\"{}\" is not a text file", call.text("file"))))?,
        _ => return Err(usage("give the sample with exactly one of --text <text> or --file <file>")),
    };
    let hits = rulebook::try_rule(&spec, &sample)?;
    if hits.is_empty() {
        env.line("The rule reports nothing in the sample.");
        return Ok(());
    }
    let rows: Vec<Vec<String>> = hits
        .iter()
        .map(|hit| {
            vec![
                hit.line.to_string(),
                hit.column.to_string(),
                hit.preview.clone(),
                hit.length.to_string(),
                entropy::show(hit.entropy_tenths),
                hit.score.to_string(),
                hit.context.clone(),
            ]
        })
        .collect();
    env.table(&["LINE", "COLUMN", "VALUE", "LENGTH", "ENTROPY", "SCORE", "CONTEXT"], &rows);
    Ok(())
}

pub(crate) fn rules_command() -> Command {
    let mut add = RULE_FLAGS.to_vec();
    add.push(switch("disabled", "add the rule in the disabled state"));
    let mut trial = RULE_FLAGS.to_vec();
    trial.push(text("text", "", "sample text to try the rule on"));
    trial.push(text("file", "", "file with sample text to try the rule on"));
    Command::group(
        "rules",
        "Manage detector rules",
        vec![
            Command::leaf("list", "List built-in and custom rules", run_rules_list).flags(&[text(
                "format",
                "table",
                "table or json",
            )]),
            Command::leaf("show", "Show one rule with its pattern", run_rules_show).args("<rule>").flags(&[text(
                "format",
                "table",
                "table or json",
            )]),
            Command::leaf("add", "Add a custom rule", run_rules_add).args("<rule>").flags(&add),
            Command::leaf("edit", "Change fields of a custom rule", run_rules_edit).args("<rule>").flags(&RULE_FLAGS),
            Command::leaf("enable", "Enable a rule", |env, call, app| run_rules_toggle(env, call, app, true))
                .args("<rule>"),
            Command::leaf("disable", "Disable a rule", |env, call, app| run_rules_toggle(env, call, app, false))
                .args("<rule>"),
            Command::leaf("restore", "Return a built-in rule to its built-in state", run_rules_restore).args("<rule>"),
            Command::leaf("remove", "Remove a custom rule", run_rules_remove).args("<rule>").flags(&[
                switch("yes", "do not ask for confirmation"),
                switch("dry-run", "show what would be removed without removing it"),
            ]),
            Command::leaf("try", "Try a rule on sample text without saving anything", run_rules_try)
                .args("[rule]")
                .flags(&trial),
        ],
    )
}

#[derive(Serialize)]
struct EntryJson {
    id: i64,
    kind: &'static str,
    pattern: String,
    rule_id: String,
    note: String,
    enabled: bool,
    demo: bool,
    revision: i64,
    created_at: String,
    updated_at: String,
}

fn entry_json(entry: &StoredEntry) -> EntryJson {
    EntryJson {
        id: entry.spec.id,
        kind: entry.spec.kind.as_str(),
        pattern: entry.spec.pattern.clone(),
        rule_id: entry.spec.rule_id.clone(),
        note: entry.spec.note.clone(),
        enabled: entry.spec.enabled,
        demo: entry.demo,
        revision: entry.revision,
        created_at: entry.created_at.to_rfc3339(),
        updated_at: entry.updated_at.to_rfc3339(),
    }
}

fn run_allowlist_list(env: &mut Env<'_>, call: &Call, app: &mut App) -> Result<()> {
    call.no_arguments()?;
    let entries = app.allowlists().list()?;
    match call.text("format") {
        "json" => return env.json(&entries.iter().map(entry_json).collect::<Vec<_>>()),
        "table" => {}
        other => return Err(usage(format!("unknown format \"{other}\": use table or json"))),
    }
    if entries.is_empty() {
        env.line(format!(
            "No allowlist entries yet. Add one with \"{} allowlist add --kind path --pattern 'tests/**'\".",
            meta::SLUG
        ));
        return Ok(());
    }
    let rows: Vec<Vec<String>> = entries
        .iter()
        .map(|entry| {
            vec![
                entry.spec.id.to_string(),
                entry.spec.kind.label().to_string(),
                entry.spec.pattern.clone(),
                if entry.spec.rule_id.is_empty() { "every rule".to_string() } else { entry.spec.rule_id.clone() },
                enabled_text(entry.spec.enabled).to_string(),
                entry.spec.note.clone(),
            ]
        })
        .collect();
    env.table(&["ID", "KIND", "PATTERN", "RULE", "ENABLED", "NOTE"], &rows);
    Ok(())
}

const ENTRY_FLAGS: [crate::cli::Flag; 4] = [
    text("kind", "", "path, value or fingerprint"),
    text("pattern", "", "path pattern such as tests/**, regular expression for a value, or a fingerprint"),
    text("rule", "", "only findings of this rule identifier, empty for every rule"),
    text("note", "", "why the entry exists, up to 400 characters"),
];

fn apply_entry_flags(call: &Call, spec: &mut EntrySpec) -> Result<bool> {
    let mut changed = false;
    if call.given("kind") {
        spec.kind = allow::Kind::parse(call.text("kind"))
            .ok_or_else(|| usage(format!("unknown kind \"{}\": use path, value or fingerprint", call.text("kind"))))?;
        changed = true;
    }
    if call.given("pattern") {
        spec.pattern = call.text("pattern").to_string();
        changed = true;
    }
    if call.given("rule") {
        spec.rule_id = call.text("rule").to_string();
        changed = true;
    }
    if call.given("note") {
        spec.note = call.text("note").to_string();
        changed = true;
    }
    Ok(changed)
}

fn run_allowlist_add(env: &mut Env<'_>, call: &Call, app: &mut App) -> Result<()> {
    call.no_arguments()?;
    if !call.given("kind") || !call.given("pattern") {
        return Err(usage("a kind and a pattern are required: add --kind <path|value|fingerprint> --pattern <pattern>"));
    }
    let mut spec = EntrySpec { enabled: !call.truth("disabled"), ..EntrySpec::default() };
    apply_entry_flags(call, &mut spec)?;
    let created = app.allowlists().create(&spec)?;
    env.line(format!(
        "Added allowlist entry {}: {} {}. It applies from the next scan.",
        created.spec.id,
        created.spec.kind.label().to_lowercase(),
        created.spec.pattern
    ));
    Ok(())
}

fn run_allowlist_edit(env: &mut Env<'_>, call: &Call, app: &mut App) -> Result<()> {
    let id = call.identifier("allowlist entry")?;
    let current = app.allowlists().get(id)?;
    let mut spec = current.spec.clone();
    if !apply_entry_flags(call, &mut spec)? {
        return Err(usage("nothing to change: give at least one of --kind, --pattern, --rule or --note"));
    }
    let updated = app.allowlists().update(id, current.revision, &spec)?;
    env.line(format!("Updated allowlist entry {} (revision {})", updated.spec.id, updated.revision));
    Ok(())
}

fn run_allowlist_toggle(env: &mut Env<'_>, call: &Call, app: &mut App, enabled: bool) -> Result<()> {
    let id = call.identifier("allowlist entry")?;
    let updated = app.allowlists().set_enabled(id, enabled)?;
    env.line(format!(
        "Allowlist entry {} is {}",
        updated.spec.id,
        if updated.spec.enabled { "enabled" } else { "disabled" }
    ));
    Ok(())
}

fn run_allowlist_remove(env: &mut Env<'_>, call: &Call, app: &mut App) -> Result<()> {
    let id = call.identifier("allowlist entry")?;
    let entry = app.allowlists().get(id)?;
    let statement = format!(
        "This removes allowlist entry {} ({} {}) permanently. Findings it matched are reported again by the next scan.",
        entry.spec.id,
        entry.spec.kind.label().to_lowercase(),
        entry.spec.pattern
    );
    if call.truth("dry-run") {
        env.line(format!("Dry run: {statement} Nothing was changed."));
        return Ok(());
    }
    env.confirm(call, Some(&*app), &statement)?;
    app.allowlists().delete(id)?;
    env.line(format!("Removed allowlist entry {id}"));
    Ok(())
}

pub(crate) fn allowlist_command() -> Command {
    let mut add = ENTRY_FLAGS.to_vec();
    add.push(switch("disabled", "add the entry in the disabled state"));
    Command::group(
        "allowlist",
        "Manage allowlist entries for test and example data",
        vec![
            Command::leaf("list", "List allowlist entries", run_allowlist_list).flags(&[text(
                "format",
                "table",
                "table or json",
            )]),
            Command::leaf("add", "Add an allowlist entry", run_allowlist_add).flags(&add),
            Command::leaf("edit", "Change fields of an allowlist entry", run_allowlist_edit)
                .args("<id>")
                .flags(&ENTRY_FLAGS),
            Command::leaf("enable", "Enable an allowlist entry", |env, call, app| {
                run_allowlist_toggle(env, call, app, true)
            })
            .args("<id>"),
            Command::leaf("disable", "Disable an allowlist entry", |env, call, app| {
                run_allowlist_toggle(env, call, app, false)
            })
            .args("<id>"),
            Command::leaf("remove", "Remove an allowlist entry", run_allowlist_remove).args("<id>").flags(&[
                switch("yes", "do not ask for confirmation"),
                switch("dry-run", "show what would be removed without removing it"),
            ]),
        ],
    )
}
