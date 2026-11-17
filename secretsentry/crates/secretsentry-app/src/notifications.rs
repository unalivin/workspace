use std::cell::RefCell;

use app_core::Result;
use app_core::meta;
use app_core::notifier::{self, Autostart, Runner};
use app_core::notify::{self, Filter, Rule};
use app_core::records;
use app_core::service::App;
use app_core::settings;
use app_core::theme::Severity;
use serde::Serialize;

use crate::cli::{Call, Command, Env, enabled_text, failure, number, switch, text, usage};

pub(crate) fn notifications_command() -> Command {
    Command::group(
        "notifications",
        "Read notifications and manage their rules",
        vec![
            Command::leaf("list", "List notifications, newest first", run_list).flags(&[
                switch("unread", "only unread notifications"),
                text("severity", "", "only this severity"),
                text("event", "", "only this event type"),
                number("limit", "200", "largest number of rows"),
                text("format", "table", "table or json"),
            ]),
            Command::leaf("read", "Mark one notification, or all of them, as read", run_read).args("<id|all>"),
            Command::group(
                "rules",
                "Show or change the rule of each event type",
                vec![
                    Command::leaf("list", "List the rule of every event type", run_rules_list),
                    Command::leaf("set", "Change the rule of an event type", run_rules_set).args("<event>").flags(&[
                        text("enabled", "", "true or false"),
                        text("min-severity", "", "lowest severity that raises a notification"),
                        text("quiet-start", "", "start of quiet hours as HH:MM, empty for none"),
                        text("quiet-end", "", "end of quiet hours as HH:MM, empty for none"),
                        text("dedup-minutes", "", "minutes in which a repeated event is not raised again, 0 for off"),
                    ]),
                    Command::leaf("reset", "Return the rule of an event type to its defaults", run_rules_reset)
                        .args("<event>"),
                ],
            ),
        ],
    )
}

#[derive(Serialize)]
struct NotificationJson {
    id: i64,
    event_type: String,
    severity: &'static str,
    title: String,
    record_kind: String,
    record_id: i64,
    created_at: String,
    read: bool,
}

fn run_list(env: &mut Env<'_>, call: &Call, app: &mut App) -> Result<()> {
    call.no_arguments()?;
    let severity = match call.text("severity") {
        "" => None,
        raw => Some(records::parse_severity(raw)?),
    };
    let event = call.text("event");
    if !event.is_empty() && notify::event_type(event).is_none() {
        let known: Vec<&str> = notify::EVENT_TYPES.iter().map(|event| event.id).collect();
        return Err(usage(format!("unknown event type \"{event}\": use one of {}", known.join(", "))));
    }
    let list = app.notifications().list(&Filter {
        unread_only: call.truth("unread"),
        severity,
        event_type: event.to_string(),
        limit: call.number("limit"),
    })?;
    match call.text("format") {
        "json" => {
            let rows: Vec<NotificationJson> = list
                .iter()
                .map(|item| NotificationJson {
                    id: item.id,
                    event_type: item.event_type.clone(),
                    severity: item.severity.as_str(),
                    title: item.title.clone(),
                    record_kind: item.record_kind.clone(),
                    record_id: item.record_id,
                    created_at: item.created_at.to_rfc3339(),
                    read: !item.unread(),
                })
                .collect();
            return env.json(&rows);
        }
        "table" => {}
        other => return Err(usage(format!("unknown format \"{other}\": use table or json"))),
    }
    if list.is_empty() {
        env.line("No notifications. They appear when a record needs attention or is completed.");
        return Ok(());
    }
    let rows: Vec<Vec<String>> = list
        .iter()
        .map(|item| {
            vec![
                item.id.to_string(),
                app.format_time(item.created_at),
                format!("{} {}", item.severity.glyph(), item.severity.label()),
                if item.unread() { "unread".to_string() } else { "read".to_string() },
                item.title.clone(),
                if item.record_id == 0 { "-".to_string() } else { format!("{} {}", item.record_kind, item.record_id) },
            ]
        })
        .collect();
    env.table(&["ID", "WHEN", "SEVERITY", "STATE", "TITLE", "LINK"], &rows);
    Ok(())
}

fn run_read(env: &mut Env<'_>, call: &Call, app: &mut App) -> Result<()> {
    let [target] = call.args.as_slice() else {
        return Err(usage("give one notification identifier or the word all"));
    };
    if target == "all" {
        let changed = app.notifications().mark_all_read()?;
        env.line(format!("Marked {changed} notifications as read"));
        return Ok(());
    }
    let id = match target.parse::<i64>() {
        Ok(id) if id > 0 => id,
        _ => {
            return Err(usage(format!(
                "\"{target}\" is not a notification identifier: use the number from the ID column or the word all"
            )));
        }
    };
    app.notifications().mark_read(id)?;
    env.line(format!("Marked notification {id} as read"));
    Ok(())
}

fn quiet_text(rule: &Rule) -> String {
    if rule.quiet_start.is_empty() { "none".to_string() } else { format!("{} to {}", rule.quiet_start, rule.quiet_end) }
}

fn run_rules_list(env: &mut Env<'_>, call: &Call, app: &mut App) -> Result<()> {
    call.no_arguments()?;
    let rows: Vec<Vec<String>> = app
        .notifications()
        .rules()?
        .iter()
        .map(|rule| {
            vec![
                rule.event_type.clone(),
                notify::event_type(&rule.event_type).map(|event| event.label).unwrap_or("").to_string(),
                enabled_text(rule.enabled).to_string(),
                rule.min_severity.label().to_string(),
                quiet_text(rule),
                rule.dedup_minutes.to_string(),
                if rule.customized { "changed".to_string() } else { "default".to_string() },
            ]
        })
        .collect();
    env.table(&["EVENT", "MEANING", "ENABLED", "MIN SEVERITY", "QUIET HOURS", "DEDUP MINUTES", "SOURCE"], &rows);
    Ok(())
}

fn event_argument(call: &Call) -> Result<&str> {
    match call.args.as_slice() {
        [event] => Ok(event),
        _ => {
            let names: Vec<&str> = notify::EVENT_TYPES.iter().map(|event| event.id).collect();
            Err(usage(format!("give one event type: {}", names.join(", "))))
        }
    }
}

fn run_rules_set(env: &mut Env<'_>, call: &Call, app: &mut App) -> Result<()> {
    let event = event_argument(call)?;
    let mut rule = app.notifications().rule(event)?;
    let mut changed = false;
    if call.given("enabled") {
        rule.enabled = match call.text("enabled").to_lowercase().as_str() {
            "true" | "yes" | "on" | "1" => true,
            "false" | "no" | "off" | "0" => false,
            other => return Err(usage(format!("--enabled needs true or false, found \"{other}\""))),
        };
        changed = true;
    }
    if call.given("min-severity") {
        let raw = call.text("min-severity").to_lowercase();
        rule.min_severity = Severity::parse(&raw).ok_or_else(|| {
            usage(format!("--min-severity needs one of {}, found \"{raw}\"", Severity::names().join(", ")))
        })?;
        changed = true;
    }
    if call.given("quiet-start") {
        rule.quiet_start = call.text("quiet-start").to_string();
        changed = true;
    }
    if call.given("quiet-end") {
        rule.quiet_end = call.text("quiet-end").to_string();
        changed = true;
    }
    if call.given("dedup-minutes") {
        let raw = call.text("dedup-minutes");
        rule.dedup_minutes =
            raw.parse().map_err(|_| usage(format!("--dedup-minutes needs a whole number, found \"{raw}\"")))?;
        changed = true;
    }
    if !changed {
        return Err(usage(
            "nothing to change: give at least one of --enabled, --min-severity, --quiet-start, --quiet-end or --dedup-minutes",
        ));
    }
    app.notifications().set_rule(&rule)?;
    env.line(format!(
        "Rule for {}: enabled {}, minimum severity {}, quiet hours {}, de-duplication {} minutes",
        rule.event_type,
        enabled_text(rule.enabled),
        rule.min_severity.label(),
        quiet_text(&rule),
        rule.dedup_minutes
    ));
    Ok(())
}

fn run_rules_reset(env: &mut Env<'_>, call: &Call, app: &mut App) -> Result<()> {
    let event = event_argument(call)?;
    app.notifications().reset_rule(event)?;
    env.line(format!("Rule for {event} is back to its defaults"));
    Ok(())
}

pub(crate) fn notifier_command() -> Command {
    let dry_run = [switch("dry-run", "show what would change without changing it")];
    Command::group(
        "notifier",
        "Show system notifications while the application is closed (opt-in)",
        vec![
            Command::leaf("run", "Check for new notifications on an interval until stopped", run_notifier)
                .flags(&[switch("once", "check one time and exit")]),
            Command::standalone(
                "install",
                "Start the notifier when you sign in (per-user entry, no administrator rights)",
                run_install,
            )
            .flags(&dry_run),
            Command::standalone("uninstall", "Remove the sign-in entry", run_uninstall).flags(&dry_run),
            Command::standalone("status", "Show whether the sign-in entry exists", run_status),
        ],
    )
}

fn sleep_for(seconds: i64) -> bool {
    std::thread::sleep(std::time::Duration::from_secs(seconds.max(0).unsigned_abs()));
    true
}

fn run_notifier(env: &mut Env<'_>, call: &Call, app: &mut App) -> Result<()> {
    call.no_arguments()?;
    let Some(deliver) = env.hooks.deliver else {
        return Err(failure("this build cannot show system notifications"));
    };
    let interval = app.settings().number(settings::KEY_NOTIFIER_INTERVAL)?;
    let runner = Runner {
        database: app.location().path.clone(),
        state_dir: app.location().config_dir.clone(),
        show_titles: app.settings().flag(settings::KEY_NOTIFIER_TITLES)?,
        clock: app.options().clock.clone(),
        deliver,
    };
    if call.truth("once") {
        let shown = runner.tick()?;
        env.line(format!("Checked once: {shown} new notifications shown"));
        return Ok(());
    }
    env.note(format!(
        "{} notifier is running, checking every {} seconds. It reads the database and makes no network requests. Press Ctrl+C to stop.",
        meta::APP_NAME,
        notifier::clamp_interval(interval)
    ));
    let wait = env.hooks.wait;
    let errors = RefCell::new(&mut *env.stderr);
    let log = |line: &str| {
        let _ = writeln!(errors.borrow_mut(), "notifier: {line}");
    };
    match wait {
        Some(wait) => runner.run(interval, 0, wait, &log),
        None => runner.run(interval, 0, &sleep_for, &log),
    }
    Ok(())
}

fn autostart(env: &Env<'_>) -> Result<Autostart> {
    match env.hooks.autostart {
        Some(make) => Ok(make()),
        None => Err(failure("this build cannot manage the sign-in entry")),
    }
}

fn run_install(env: &mut Env<'_>, call: &Call) -> Result<()> {
    call.no_arguments()?;
    let entry = autostart(env)?;
    let path = entry.install(call.truth("dry-run"))?;
    if call.truth("dry-run") {
        env.line(format!(
            "Dry run: would write {} to start \"{} notifier run\" at sign-in. Nothing was changed.",
            path.display(),
            entry.executable
        ));
    } else {
        env.line(format!(
            "Wrote {}. The notifier starts the next time you sign in. Start it now with \"{} notifier run\".",
            path.display(),
            meta::SLUG
        ));
    }
    Ok(())
}

fn run_uninstall(env: &mut Env<'_>, call: &Call) -> Result<()> {
    call.no_arguments()?;
    let (removed, path) = autostart(env)?.uninstall(call.truth("dry-run"))?;
    let path = path.display();
    if !removed {
        env.line(format!("The notifier is not set to start at sign-in ({path} does not exist)."));
    } else if call.truth("dry-run") {
        env.line(format!("Dry run: would remove {path}. Nothing was changed."));
    } else {
        env.line(format!(
            "Removed {path}. A notifier that is running now keeps running until you sign out or stop it."
        ));
    }
    Ok(())
}

fn run_status(env: &mut Env<'_>, call: &Call) -> Result<()> {
    call.no_arguments()?;
    let (installed, path) = autostart(env)?.status()?;
    let path = path.display();
    if installed {
        env.line(format!("Start at sign-in: on ({path})"));
    } else {
        env.line(format!("Start at sign-in: off ({path} does not exist)"));
    }
    env.line("Display of system notifications depends on the desktop and is not verified by the automated tests.");
    Ok(())
}
