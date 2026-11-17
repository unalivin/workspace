use std::io::Read;
use std::path::Path;

use app_core::meta;
use app_core::records::{self, Input, Query, Record};
use app_core::service::App;
use app_core::theme::Severity;
use app_core::transfer::{self, Format};
use app_core::{AppError, Result};
use serde::Serialize;

use crate::cli::{Call, Command, Env, capitalize, enabled_text, failure, number, switch, text, usage};

#[derive(Serialize)]
struct RecordJson {
    id: i64,
    name: String,
    status: &'static str,
    severity: &'static str,
    note: String,
    enabled: bool,
    demo: bool,
    revision: i64,
    created_at: String,
    updated_at: String,
}

fn to_json(record: &Record) -> RecordJson {
    RecordJson {
        id: record.id,
        name: record.name.clone(),
        status: record.status.as_str(),
        severity: record.severity.as_str(),
        note: record.note.clone(),
        enabled: record.enabled,
        demo: record.demo,
        revision: record.revision,
        created_at: record.created_at.to_rfc3339(),
        updated_at: record.updated_at.to_rfc3339(),
    }
}

const INPUT_FLAGS: [crate::cli::Flag; 4] = [
    text("name", "", "name, 1 to 200 characters"),
    text("status", "", "open, in_progress or done"),
    text("severity", "", "info, low, medium, high or critical"),
    text("note", "", "free text, up to 4000 characters"),
];

#[derive(Serialize)]
struct BreakdownJson {
    status: &'static str,
    severity: &'static str,
    total: i64,
    demo: i64,
}

#[derive(Serialize)]
struct OverviewJson {
    open_records: i64,
    urgent_records: i64,
    disabled_records: i64,
    demo_records: i64,
    unread_notifications: i64,
    breakdown: Vec<BreakdownJson>,
}

fn run_overview(env: &mut Env<'_>, call: &Call, app: &mut App) -> Result<()> {
    call.no_arguments()?;
    let overview = app.records().overview()?;
    match call.text("format") {
        "json" => {
            return env.json(&OverviewJson {
                open_records: overview.open,
                urgent_records: overview.urgent,
                disabled_records: overview.disabled,
                demo_records: overview.demo,
                unread_notifications: overview.unread_notifications,
                breakdown: overview
                    .breakdown
                    .iter()
                    .map(|row| BreakdownJson {
                        status: row.status.as_str(),
                        severity: row.severity.as_str(),
                        total: row.total,
                        demo: row.demo,
                    })
                    .collect(),
            });
        }
        "table" => {}
        other => return Err(usage(format!("unknown format \"{other}\": use table or json"))),
    }
    env.table(
        &["MEASURE", "COUNT"],
        &[
            vec!["Open records".to_string(), overview.open.to_string()],
            vec!["Open records with high or critical severity".to_string(), overview.urgent.to_string()],
            vec!["Disabled records".to_string(), overview.disabled.to_string()],
            vec!["Demo records".to_string(), overview.demo.to_string()],
            vec!["Unread notifications".to_string(), overview.unread_notifications.to_string()],
        ],
    );
    if overview.total() == 0 && overview.disabled == 0 {
        env.line(format!(
            "\nNo records yet. Add one with \"{slug} records add --name <name>\" or load samples with \"{slug} demo load\".",
            slug = meta::SLUG
        ));
        return Ok(());
    }
    env.line("\nOpen and in-progress records by severity");
    let rows: Vec<Vec<String>> = Severity::ALL
        .iter()
        .rev()
        .map(|level| vec![format!("{} {}", level.glyph(), level.label()), overview.by_severity(*level).to_string()])
        .collect();
    env.table(&["SEVERITY", "COUNT"], &rows);
    if overview.demo > 0 {
        env.line(format!("\nDemo data is loaded: {} sample records are labeled \"Demo:\".", overview.demo));
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

pub(crate) fn records_command() -> Command {
    Command::group(
        "records",
        "Manage records",
        vec![
            Command::leaf("list", "List, search, filter and sort records", run_list).flags(&[
                text("search", "", "words to find in names and notes"),
                text("status", "", "only this status"),
                text("severity", "", "only this severity"),
                switch("all", "include disabled records"),
                text("sort", "name", "name, status, severity or updated"),
                switch("desc", "sort in descending order"),
                number("limit", "500", "largest number of rows"),
                text("format", "table", "table, json or csv"),
            ]),
            Command::leaf("show", "Show one record", run_show).args("<id>").flags(&[text(
                "format",
                "table",
                "table or json",
            )]),
            Command::leaf("add", "Add a record", run_add).flags(&[
                INPUT_FLAGS[0],
                INPUT_FLAGS[1],
                INPUT_FLAGS[2],
                INPUT_FLAGS[3],
                switch("disabled", "add the record in the disabled state"),
            ]),
            Command::leaf("edit", "Change fields of a record", run_edit).args("<id>").flags(&INPUT_FLAGS),
            Command::leaf("enable", "Enable a record", |env, call, app| run_toggle(env, call, app, true)).args("<id>"),
            Command::leaf("disable", "Disable a record", |env, call, app| run_toggle(env, call, app, false))
                .args("<id>"),
            Command::leaf("remove", "Remove a record", run_remove).args("<id>").flags(&[
                switch("yes", "do not ask for confirmation"),
                switch("dry-run", "show what would be removed without removing it"),
            ]),
            Command::leaf(
                "import",
                "Import records from a CSV or JSON file, or from standard input with -",
                run_import,
            )
            .args("<file|->")
            .flags(&[
                text("format", "", "csv or json, by default taken from the file extension"),
                switch("dry-run", "validate and report without storing anything"),
            ]),
            Command::leaf("export", "Export records as CSV or JSON", run_export).flags(&[
                text("format", "csv", "csv or json"),
                text("output", "", "file to write, by default standard output"),
                switch("force", "overwrite the output file if it exists"),
            ]),
        ],
    )
}

fn status_text(record: &Record) -> String {
    format!("{} {}", record.status.glyph(), record.status.label())
}

fn severity_text(severity: Severity) -> String {
    format!("{} {}", severity.glyph(), severity.label())
}

fn optional_status(raw: &str) -> Result<Option<records::Status>> {
    if raw.is_empty() { Ok(None) } else { records::parse_status(raw).map(Some) }
}

fn optional_severity(raw: &str) -> Result<Option<Severity>> {
    if raw.is_empty() { Ok(None) } else { records::parse_severity(raw).map(Some) }
}

fn run_list(env: &mut Env<'_>, call: &Call, app: &mut App) -> Result<()> {
    call.no_arguments()?;
    let query = Query {
        search: call.text("search").to_string(),
        status: optional_status(call.text("status"))?,
        severity: optional_severity(call.text("severity"))?,
        include_disabled: call.truth("all"),
        sort: call.text("sort").to_string(),
        descending: call.truth("desc"),
        limit: call.number("limit"),
        offset: 0,
    };
    let list = app.records().list(&query)?;
    match call.text("format") {
        "json" => return env.json(&list.iter().map(to_json).collect::<Vec<_>>()),
        "csv" => {
            let exported = transfer::export(Format::Csv, &list)?;
            env.print(exported);
            return Ok(());
        }
        "table" => {}
        other => return Err(usage(format!("unknown format \"{other}\": use table, json or csv"))),
    }
    if list.is_empty() {
        if query.search.is_empty() && query.status.is_none() && query.severity.is_none() {
            env.line(format!("No records yet. Add one with \"{} records add --name <name>\".", meta::SLUG));
        } else {
            env.line("No records match. Remove a filter or add --all to include disabled records.");
        }
        return Ok(());
    }
    let rows: Vec<Vec<String>> = list
        .iter()
        .map(|record| {
            vec![
                record.id.to_string(),
                record.name.clone(),
                status_text(record),
                severity_text(record.severity),
                enabled_text(record.enabled).to_string(),
                app.format_time(record.updated_at),
            ]
        })
        .collect();
    env.table(&["ID", "NAME", "STATUS", "SEVERITY", "ENABLED", "UPDATED"], &rows);
    Ok(())
}

fn run_show(env: &mut Env<'_>, call: &Call, app: &mut App) -> Result<()> {
    let id = call.identifier("record")?;
    let record = app.records().get(id)?;
    match call.text("format") {
        "json" => env.json(&to_json(&record)),
        "table" => {
            let name = if record.demo { format!("{} (demo data)", record.name) } else { record.name.clone() };
            env.table(
                &["FIELD", "VALUE"],
                &[
                    vec!["ID".to_string(), record.id.to_string()],
                    vec!["Name".to_string(), name],
                    vec!["Status".to_string(), status_text(&record)],
                    vec!["Severity".to_string(), severity_text(record.severity)],
                    vec!["Enabled".to_string(), enabled_text(record.enabled).to_string()],
                    vec!["Revision".to_string(), record.revision.to_string()],
                    vec!["Created".to_string(), app.format_time(record.created_at)],
                    vec!["Updated".to_string(), app.format_time(record.updated_at)],
                    vec!["Note".to_string(), record.note.replace('\n', " / ")],
                ],
            );
            Ok(())
        }
        other => Err(usage(format!("unknown format \"{other}\": use table or json"))),
    }
}

fn apply_input_flags(call: &Call, input: &mut Input) -> Result<bool> {
    let mut changed = false;
    if call.given("name") {
        input.name = call.text("name").to_string();
        changed = true;
    }
    if call.given("status") {
        input.status = records::parse_status(call.text("status"))?;
        changed = true;
    }
    if call.given("severity") {
        input.severity = records::parse_severity(call.text("severity"))?;
        changed = true;
    }
    if call.given("note") {
        input.note = call.text("note").to_string();
        changed = true;
    }
    Ok(changed)
}

fn run_add(env: &mut Env<'_>, call: &Call, app: &mut App) -> Result<()> {
    call.no_arguments()?;
    if !call.given("name") {
        return Err(usage("a name is required: add --name <name>"));
    }
    let mut input = Input { enabled: !call.truth("disabled"), ..Input::default() };
    apply_input_flags(call, &mut input)?;
    let created = app.records().create(&input)?;
    env.line(format!("Added record {}: {}", created.id, created.name));
    Ok(())
}

fn run_edit(env: &mut Env<'_>, call: &Call, app: &mut App) -> Result<()> {
    let id = call.identifier("record")?;
    let current = app.records().get(id)?;
    let mut change = current.input();
    if !apply_input_flags(call, &mut change)? {
        return Err(usage("nothing to change: give at least one of --name, --status, --severity or --note"));
    }
    let updated = app.records().update(id, current.revision, &change)?;
    env.line(format!("Updated record {} (revision {})", updated.id, updated.revision));
    Ok(())
}

fn run_toggle(env: &mut Env<'_>, call: &Call, app: &mut App, enabled: bool) -> Result<()> {
    let id = call.identifier("record")?;
    let updated = app.records().set_enabled(id, enabled)?;
    env.line(format!("Record {} is {}", updated.id, if updated.enabled { "enabled" } else { "disabled" }));
    Ok(())
}

fn run_remove(env: &mut Env<'_>, call: &Call, app: &mut App) -> Result<()> {
    let id = call.identifier("record")?;
    let record = app.records().get(id)?;
    let statement = format!("This removes record {} ({}) permanently.", record.id, record.name);
    if call.truth("dry-run") {
        env.line(format!("Dry run: {statement} Nothing was changed."));
        return Ok(());
    }
    env.confirm(call, Some(&*app), &statement)?;
    app.records().delete(id)?;
    env.line(format!("Removed record {id}"));
    Ok(())
}

fn format_for(name: &str, explicit: &str) -> Result<Format> {
    if explicit.is_empty() && !name.to_lowercase().ends_with(".csv") && !name.to_lowercase().ends_with(".json") {
        return Err(usage("the format cannot be told from the file name: add --format csv or --format json"));
    }
    Format::for_file(name, explicit)
}

fn run_import(env: &mut Env<'_>, call: &Call, app: &mut App) -> Result<()> {
    let [source] = call.args.as_slice() else {
        return Err(usage("give one file to import, or - to read standard input"));
    };
    let format = format_for(source, call.text("format"))?;
    let raw = if source == "-" {
        let mut raw = Vec::new();
        Read::take(&mut *env.stdin, transfer::MAX_IMPORT_BYTES as u64 + 1)
            .read_to_end(&mut raw)
            .map_err(|error| failure(format!("cannot read standard input: {error}")))?;
        raw
    } else {
        app_core::files::read_limited(Path::new(source), transfer::MAX_IMPORT_BYTES as u64)
            .map_err(|error| failure(format!("cannot open the import file: {error}")))?
    };
    let result = transfer::import(&app.records(), &raw, format, call.truth("dry-run"))?;
    env.line(capitalize(&result.summary()));
    for problem in &result.problems {
        env.note(format!("rejected {problem}"));
    }
    if !result.problems.is_empty() && result.created + result.updated + result.skipped == 0 {
        return Err(failure("no row could be imported: fix the rows listed above"));
    }
    Ok(())
}

fn run_export(env: &mut Env<'_>, call: &Call, app: &mut App) -> Result<()> {
    call.no_arguments()?;
    let output = call.text("output");
    if output.is_empty() {
        let (content, _) = app.export_text(Format::parse(call.text("format"))?)?;
        env.print(content);
        return Ok(());
    }
    let format = if call.given("format") {
        Format::parse(call.text("format"))?
    } else {
        Format::for_file(output, "").unwrap_or(Format::Csv)
    };
    match app.export_file(Path::new(output), format.label(), call.truth("force")) {
        Ok(count) => {
            env.line(format!("Exported {count} records to {output}"));
            Ok(())
        }
        Err(AppError::Conflict(_)) => {
            Err(usage(format!("{output} already exists: choose another name or add --force to overwrite it")))
        }
        Err(error) => Err(error),
    }
}

pub(crate) fn demo_command() -> Command {
    Command::group(
        "demo",
        "Load or remove labeled demo data",
        vec![
            Command::leaf("load", "Add sample records labeled as demo data", |env, call, app| {
                call.no_arguments()?;
                let added = app.records().load_demo()?;
                if added == 0 {
                    env.line("Demo data is already loaded.");
                } else {
                    env.line(format!("Added {added} demo records. Their names start with \"Demo:\"."));
                }
                Ok(())
            }),
            Command::leaf("remove", "Remove every demo record", |env, call, app| {
                call.no_arguments()?;
                let count = app.records().remove_demo(true)?;
                let statement = format!("This removes {count} demo records. Records you added yourself stay.");
                if call.truth("dry-run") {
                    env.line(format!("Dry run: {statement} Nothing was changed."));
                    return Ok(());
                }
                if count == 0 {
                    env.line("There is no demo data to remove.");
                    return Ok(());
                }
                env.confirm(call, Some(&*app), &statement)?;
                let removed = app.records().remove_demo(false)?;
                env.line(format!("Removed {removed} demo records"));
                Ok(())
            })
            .flags(&[
                switch("yes", "do not ask for confirmation"),
                switch("dry-run", "show how many records would be removed"),
            ]),
        ],
    )
}
