use app_core::Result;
use app_core::guide;
use app_core::meta;
use app_core::service::{self, App, Location, MoveMode};
use app_core::settings::{self, Value};
use app_core::theme::{self, Mode, Profile};
use app_core::userconfig;
use serde::Serialize;

use crate::cli::{Call, Command, Env, failure, switch, text, usage};

pub(crate) fn config_command() -> Command {
    Command::group(
        "config",
        "Show and change settings",
        vec![
            Command::leaf("list", "List every setting with its value and where the value comes from", run_list)
                .flags(&[text("format", "table", "table or json")]),
            Command::leaf("get", "Print the value of one setting", run_get).args("<key>"),
            Command::leaf("set", "Change a setting", run_set).args("<key> <value>"),
            Command::leaf("reset", "Return one setting to its default", run_reset).args("<key>"),
            Command::group(
                "theme",
                "Choose the color theme",
                vec![
                    Command::leaf("list", "List the color themes", run_theme_list),
                    Command::leaf("set", "Choose a color theme", run_theme_set).args("<name>"),
                    Command::leaf("show", "Show the theme, mode and colors in use", run_theme_show),
                ],
            ),
            Command::group(
                "launcher",
                "Choose which front end opens by default",
                vec![
                    Command::leaf("show", "Show the remembered front end", run_launcher_show),
                    Command::leaf("set", "Remember a front end", run_launcher_set).args("<gui|tui>"),
                    Command::leaf("reset", "Forget the choice and ask again", run_launcher_reset),
                ],
            ),
        ],
    )
}

fn source_text(value: &Value) -> String {
    if value.overridden_by.is_empty() {
        value.source.label().to_string()
    } else {
        format!("overridden by {}", value.overridden_by)
    }
}

fn shown(raw: &str) -> &str {
    if raw.is_empty() { "(empty)" } else { raw }
}

#[derive(Serialize)]
struct SettingJson {
    key: String,
    value: String,
    default: &'static str,
    source: &'static str,
    #[serde(skip_serializing_if = "String::is_empty")]
    overridden_by: String,
    #[serde(skip_serializing_if = "String::is_empty")]
    problem: String,
}

fn run_list(env: &mut Env<'_>, call: &Call, app: &mut App) -> Result<()> {
    call.no_arguments()?;
    let values = app.settings().list()?;
    match call.text("format") {
        "json" => {
            let rows: Vec<SettingJson> = values
                .iter()
                .map(|value| SettingJson {
                    key: value.key.clone(),
                    value: value.raw.clone(),
                    default: value.definition.default,
                    source: value.source.label(),
                    overridden_by: value.overridden_by.clone(),
                    problem: value.override_error.clone(),
                })
                .collect();
            return env.json(&rows);
        }
        "table" => {}
        other => return Err(usage(format!("unknown format \"{other}\": use table or json"))),
    }
    let rows: Vec<Vec<String>> = values
        .iter()
        .map(|value| {
            vec![
                value.key.clone(),
                shown(&value.raw).to_string(),
                source_text(value),
                value.definition.label.to_string(),
            ]
        })
        .collect();
    env.table(&["KEY", "VALUE", "SOURCE", "MEANING"], &rows);
    for value in values.iter().filter(|value| !value.override_error.is_empty()) {
        env.note(format!("note: {}: {}", value.key, value.override_error));
    }
    Ok(())
}

fn one_key(call: &Call) -> Result<&str> {
    match call.args.as_slice() {
        [key] => Ok(key),
        _ => Err(usage(format!("give one setting key: run \"{} config list\" to see them", meta::SLUG))),
    }
}

fn run_get(env: &mut Env<'_>, call: &Call, app: &mut App) -> Result<()> {
    let value = app.settings().get(one_key(call)?)?;
    env.line(&value.raw);
    if !value.overridden_by.is_empty() {
        env.note(format!(
            "note: overridden by {}, the stored value is \"{}\"",
            value.overridden_by,
            value.stored.as_deref().unwrap_or("")
        ));
    }
    Ok(())
}

fn precedence_note(env: &mut Env<'_>, value: &Value, saved: bool) {
    if !value.overridden_by.is_empty() {
        let lead = if saved { "note: saved, but the" } else { "note: the" };
        env.note(format!("{lead} {} still takes precedence with \"{}\"", value.overridden_by, value.raw));
    }
}

fn run_set(env: &mut Env<'_>, call: &Call, app: &mut App) -> Result<()> {
    let [key, raw] = call.args.as_slice() else {
        return Err(usage(format!(
            "give a key and a value, for example: {} config set {} compact",
            meta::SLUG,
            settings::KEY_DENSITY
        )));
    };
    let value = app.settings().set(key, raw)?;
    env.line(format!("{} = {}", value.key, shown(value.stored.as_deref().unwrap_or(""))));
    precedence_note(env, &value, true);
    Ok(())
}

fn run_reset(env: &mut Env<'_>, call: &Call, app: &mut App) -> Result<()> {
    let value = app.settings().reset(one_key(call)?)?;
    env.line(format!("{} is back to its default: {}", value.key, shown(value.definition.default)));
    precedence_note(env, &value, false);
    Ok(())
}

fn run_theme_list(env: &mut Env<'_>, call: &Call, app: &mut App) -> Result<()> {
    call.no_arguments()?;
    let current = app.settings().text(settings::KEY_THEME)?;
    let accent = |name: &str, mode: Mode| {
        theme::resolve(&theme::Options { name: name.to_string(), mode, ..theme::Options::default() })
            .palette
            .accent
            .hex()
    };
    let rows: Vec<Vec<String>> = theme::names()
        .into_iter()
        .map(|name| {
            vec![
                name.to_string(),
                theme::label(name).to_string(),
                accent(name, Mode::Light),
                accent(name, Mode::Dark),
                if name == current { "in use".to_string() } else { String::new() },
            ]
        })
        .collect();
    env.table(&["NAME", "LABEL", "LIGHT ACCENT", "DARK ACCENT", ""], &rows);
    Ok(())
}

fn run_theme_set(env: &mut Env<'_>, call: &Call, app: &mut App) -> Result<()> {
    let [name] = call.args.as_slice() else {
        return Err(usage(format!("give one theme name: {}", theme::names().join(", "))));
    };
    let value = app.settings().set(settings::KEY_THEME, name)?;
    let stored = value.stored.clone().unwrap_or_default();
    env.line(format!("Color theme: {stored} ({})", theme::label(&stored)));
    precedence_note(env, &value, true);
    Ok(())
}

fn run_theme_show(env: &mut Env<'_>, call: &Call, app: &mut App) -> Result<()> {
    call.no_arguments()?;
    let values = app.settings().effective()?;
    let raw = |key: &str| values.get(key).map(|value| value.raw.clone()).unwrap_or_default();
    let mode = raw(settings::KEY_MODE);
    let tokens = app.appearance(mode == "dark")?;
    let mode_text = if mode == "system" {
        "system (the terminal cannot ask the system, light colors are listed)".to_string()
    } else {
        mode
    };
    let profile = Profile::detect(&env.options.env, raw(settings::KEY_MONOCHROME) == "true");
    let theme_source = values.get(settings::KEY_THEME).map(source_text).unwrap_or_default();
    let measured = |color: theme::Color| {
        format!("{} (contrast {:.1}:1 on surface)", color.hex(), theme::contrast(color, tokens.palette.surface))
    };
    let listed = if tokens.dark { "dark" } else { "light" };
    env.table(
        &["ITEM", "VALUE"],
        &[
            vec!["Theme".to_string(), format!("{} ({}), {theme_source}", tokens.name, tokens.label)],
            vec!["Mode".to_string(), mode_text],
            vec!["Colors listed".to_string(), listed.to_string()],
            vec!["Terminal colors".to_string(), profile.label().to_string()],
            vec!["Density".to_string(), raw(settings::KEY_DENSITY)],
            vec!["Text size".to_string(), format!("{} %", raw(settings::KEY_TEXT_SCALE))],
            vec!["Reduced motion".to_string(), raw(settings::KEY_REDUCED_MOTION)],
            vec!["Background".to_string(), tokens.palette.background.hex()],
            vec!["Surface".to_string(), tokens.palette.surface.hex()],
            vec!["Text".to_string(), measured(tokens.palette.text)],
            vec!["Accent".to_string(), measured(tokens.palette.accent)],
        ],
    );
    for warning in &tokens.warnings {
        env.note(format!("note: {warning}"));
    }
    Ok(())
}

fn front_end_label(mode: &str) -> &'static str {
    match mode {
        userconfig::FRONT_END_GUI => "gui (dashboard window)",
        userconfig::FRONT_END_TUI => "tui (terminal UI)",
        _ => "not set, the application asks at start",
    }
}

fn run_launcher_show(env: &mut Env<'_>, call: &Call, app: &mut App) -> Result<()> {
    call.no_arguments()?;
    env.line(format!("Open with: {}", front_end_label(app.front_end())));
    let variable = meta::env_name("ui");
    if let Some(value) = env.options.env.get(&variable) {
        env.note(format!("note: overridden by environment variable {variable}={value}"));
    }
    Ok(())
}

fn run_launcher_set(env: &mut Env<'_>, call: &Call, app: &mut App) -> Result<()> {
    let mode = match call.args.as_slice() {
        [mode] if mode == userconfig::FRONT_END_GUI || mode == userconfig::FRONT_END_TUI => mode,
        _ => return Err(usage("give gui or tui")),
    };
    app.remember_front_end(mode)?;
    env.line(format!("Open with: {}", front_end_label(mode)));
    Ok(())
}

fn run_launcher_reset(env: &mut Env<'_>, call: &Call, app: &mut App) -> Result<()> {
    call.no_arguments()?;
    app.remember_front_end("")?;
    env.line(format!("Open with: {}", front_end_label("")));
    Ok(())
}

fn location_source(location: &Location) -> String {
    if !location.overridden_by.is_empty() {
        location.overridden_by.clone()
    } else if location.source == "saved" {
        "saved location".to_string()
    } else {
        "default location".to_string()
    }
}

pub(crate) fn database_command() -> Command {
    Command::group(
        "db",
        "Create, locate, move and check the database",
        vec![
            Command::leaf("init", "Create the database if it does not exist", |env, call, app| {
                call.no_arguments()?;
                let path = app.location().path.display().to_string();
                if app.created_database() {
                    env.line(format!("Database ready at {path}"));
                } else {
                    env.line(format!("The database already exists at {path}"));
                }
                Ok(())
            }),
            Command::standalone("path", "Print the database path and what decides it", |env, call| {
                call.no_arguments()?;
                let location = service::locate(&env.app_options(&call.globals, false))?;
                env.line(location.path.display().to_string());
                let state = if location.exists { "exists" } else { "not created yet" };
                env.note(format!("set by: {}, {state}", location_source(&location)));
                Ok(())
            }),
            Command::leaf(
                "move",
                "Copy the database to a new location and use it, or switch to another database",
                run_move,
            )
            .args("<path>")
            .flags(&[
                switch(
                    "switch",
                    "use the database at the path as it is, or create an empty one there, instead of copying",
                ),
                switch("yes", "do not ask for confirmation"),
                switch("dry-run", "show what would change without changing it"),
            ]),
            Command::leaf("check", "Run the integrity and foreign-key checks", |env, call, app| {
                call.no_arguments()?;
                app.store().integrity_check()?;
                app.store().foreign_key_check()?;
                let version = app.store().schema_version()?;
                env.line(format!("Database is healthy: integrity check ok, foreign keys ok, schema version {version}"));
                Ok(())
            }),
        ],
    )
}

fn run_move(env: &mut Env<'_>, call: &Call, app: &mut App) -> Result<()> {
    let [target] = call.args.as_slice() else {
        return Err(usage("give the new database path"));
    };
    let mode = if call.truth("switch") { MoveMode::Switch } else { MoveMode::Copy };
    let plan = app.plan_move(target, mode).map_err(|error| usage(error.to_string()))?;
    if call.truth("dry-run") {
        env.line(format!("Dry run: {} Nothing was changed.", plan.statement));
        return Ok(());
    }
    env.confirm(call, Some(&*app), &plan.statement)?;
    app.move_database(&plan)?;
    env.line(format!("Now using {}", app.location().path.display()));
    Ok(())
}

pub(crate) fn run_diagnostics(env: &mut Env<'_>, call: &Call, app: &mut App) -> Result<()> {
    call.no_arguments()?;
    let report = app.diagnostics(env.hooks.open_gui.is_some());
    let rows: Vec<Vec<String>> =
        report.lines().into_iter().map(|(label, value)| vec![label.to_string(), value]).collect();
    env.table(&["ITEM", "VALUE"], &rows);
    if report.integrity != "ok" {
        return Err(failure(
            "the integrity check failed: restore the database from a backup or export the records and create a new database",
        ));
    }
    Ok(())
}

pub(crate) fn run_guide(env: &mut Env<'_>, call: &Call) -> Result<()> {
    let usage_guide = service::usage_guide();
    if call.args.is_empty() {
        env.line(format!("{} usage guide\n\nContents:", meta::APP_NAME));
        for section in &usage_guide.sections {
            env.line(format!("{}{}", "  ".repeat(section.level), section.title));
        }
        env.line(format!("\nRead a topic with \"{} guide <words>\".", meta::SLUG));
        return Ok(());
    }
    let words = call.args.join(" ");
    let hits = usage_guide.search(&words);
    if hits.is_empty() {
        return Err(failure(format!(
            "no section of the usage guide mentions \"{words}\": run \"{} guide\" for the contents",
            meta::SLUG
        )));
    }
    for (index, hit) in hits.iter().enumerate() {
        if index > 0 {
            env.line("");
        }
        let section = &usage_guide.sections[hit.section];
        env.line(format!(
            "{}\n{}\n\n{}",
            section.title,
            "=".repeat(section.title.chars().count()),
            guide::plain(&section.body)
        ));
    }
    Ok(())
}

pub(crate) fn run_about(env: &mut Env<'_>, call: &Call, app: &mut App) -> Result<()> {
    call.no_arguments()?;
    if call.truth("issue") {
        let issue = app.diagnostics(env.hooks.open_gui.is_some()).issue_report();
        env.line(format!(
            "Found a bug? Please report it on GitHub.\n\nThe report form is prefilled with exactly this text and nothing else:\n\n{}\nOpen this link in a browser to continue:\n{}",
            issue.body, issue.url
        ));
        return Ok(());
    }
    env.table(
        &["ITEM", "VALUE"],
        &[
            vec!["Application".to_string(), format!("{} {}", meta::APP_NAME, meta::VERSION)],
            vec!["About".to_string(), meta::TAGLINE.to_string()],
            vec!["Publisher".to_string(), meta::PUBLISHER.to_string()],
            vec!["License".to_string(), meta::LICENSE.to_string()],
            vec!["Source Code".to_string(), service::source_link().url],
            vec!["Report a bug".to_string(), format!("{}/issues/new", meta::REPOSITORY_URL)],
            vec!["Report a vulnerability".to_string(), service::advisory_link().url],
        ],
    );
    env.line(format!(
        "\nRun \"{} about --issue\" to preview a bug report. Nothing is opened or sent by this command.",
        meta::SLUG
    ));
    Ok(())
}

pub(crate) fn run_licenses(env: &mut Env<'_>, call: &Call) -> Result<()> {
    call.no_arguments()?;
    env.line(format!("{}\n\n{}", meta::LICENSE_TEXT, meta::NOTICES_TEXT));
    Ok(())
}
