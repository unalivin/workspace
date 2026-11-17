use std::collections::{BTreeMap, BTreeSet};
use std::io::{BufRead, Write};

use app_core::launcher::{self, Action, PromptResult, Session};
use app_core::meta;
use app_core::notifier::{Autostart, Deliver};
use app_core::service::{self, App, Options};
use app_core::settings;
use app_core::{AppError, Result};

use crate::{config, notifications, records};

pub const EXIT_OK: i32 = 0;
pub const EXIT_FAILURE: i32 = 1;
pub const EXIT_USAGE: i32 = 2;

pub type FrontEnd<'a> = dyn Fn(&mut App) -> Result<String> + 'a;
pub type Chooser<'a> = dyn Fn(&str) -> Result<PromptResult> + 'a;
pub type WindowOpener<'a> = dyn Fn(&str, &[String]) -> Result<()> + 'a;

#[derive(Clone, Copy, Default)]
pub struct Hooks<'a> {
    pub open_gui: Option<&'a FrontEnd<'a>>,
    pub open_tui: Option<&'a FrontEnd<'a>>,
    pub show_chooser: Option<&'a Chooser<'a>>,
    pub open_terminal_window: Option<&'a WindowOpener<'a>>,
    pub deliver: Option<&'a Deliver<'a>>,
    pub autostart: Option<&'a dyn Fn() -> Autostart>,
    pub wait: Option<&'a dyn Fn(i64) -> bool>,
}

pub struct Env<'a> {
    pub args: Vec<String>,
    pub stdin: &'a mut dyn BufRead,
    pub stdout: &'a mut dyn Write,
    pub stderr: &'a mut dyn Write,
    pub options: Options,
    pub session: Session,
    pub executable: String,
    pub hooks: Hooks<'a>,
}

pub(crate) fn usage(message: impl Into<String>) -> AppError {
    AppError::Usage(message.into())
}

pub(crate) fn failure(message: impl Into<String>) -> AppError {
    AppError::Io(message.into())
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct Globals {
    pub database: String,
    pub overrides: BTreeMap<String, String>,
    pub ui: String,
    pub ask: bool,
    pub no_prompt: bool,
    pub help: bool,
    pub version: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Kind {
    Switch,
    Text,
    Number,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Flag {
    pub name: &'static str,
    pub kind: Kind,
    pub default: &'static str,
    pub help: &'static str,
}

pub(crate) const fn switch(name: &'static str, help: &'static str) -> Flag {
    Flag { name, kind: Kind::Switch, default: "false", help }
}

pub(crate) const fn text(name: &'static str, default: &'static str, help: &'static str) -> Flag {
    Flag { name, kind: Kind::Text, default, help }
}

pub(crate) const fn number(name: &'static str, default: &'static str, help: &'static str) -> Flag {
    Flag { name, kind: Kind::Number, default, help }
}

const GLOBAL_FLAGS: &[Flag] = &[
    text("db", "", "database file to use for this run"),
    text("set", "", "override a setting for this run, as key=value (repeatable)"),
    text("ui", "", "open the dashboard (gui) or the terminal UI (tui)"),
    switch("ask", "ask which front end to open even when a choice is remembered"),
    switch("no-prompt", "never ask, open the remembered or default front end"),
    switch("help", "show help"),
    switch("version", "print the version"),
];

pub(crate) type WithApp = fn(&mut Env<'_>, &Call, &mut App) -> Result<()>;
pub(crate) type WithoutApp = fn(&mut Env<'_>, &Call) -> Result<()>;

#[derive(Clone, Copy)]
pub(crate) enum Run {
    Database(WithApp),
    Standalone(WithoutApp),
}

pub(crate) struct Command {
    pub name: &'static str,
    pub summary: &'static str,
    pub args: &'static str,
    pub flags: Vec<Flag>,
    pub sub: Vec<Command>,
    pub run: Option<Run>,
}

impl Command {
    pub fn group(name: &'static str, summary: &'static str, sub: Vec<Command>) -> Command {
        Command { name, summary, args: "", flags: Vec::new(), sub, run: None }
    }

    pub fn leaf(name: &'static str, summary: &'static str, run: WithApp) -> Command {
        Command { name, summary, args: "", flags: Vec::new(), sub: Vec::new(), run: Some(Run::Database(run)) }
    }

    pub fn standalone(name: &'static str, summary: &'static str, run: WithoutApp) -> Command {
        Command { name, summary, args: "", flags: Vec::new(), sub: Vec::new(), run: Some(Run::Standalone(run)) }
    }

    pub fn args(mut self, args: &'static str) -> Command {
        self.args = args;
        self
    }

    pub fn flags(mut self, flags: &[Flag]) -> Command {
        self.flags = flags.to_vec();
        self
    }

    fn find(&self, name: &str) -> Option<&Command> {
        self.sub.iter().find(|child| child.name == name)
    }
}

#[derive(Debug, Clone, Default)]
pub(crate) struct Call {
    pub globals: Globals,
    pub parent: String,
    pub args: Vec<String>,
    values: BTreeMap<&'static str, String>,
    given: BTreeSet<&'static str>,
}

impl Call {
    pub fn text(&self, name: &str) -> &str {
        self.values.get(name).map(String::as_str).unwrap_or("")
    }

    pub fn given(&self, name: &str) -> bool {
        self.given.contains(name)
    }

    pub fn truth(&self, name: &str) -> bool {
        self.text(name) == "true"
    }

    pub fn number(&self, name: &str) -> i64 {
        self.text(name).parse().unwrap_or(0)
    }

    pub fn no_arguments(&self) -> Result<()> {
        match self.args.first() {
            Some(extra) => Err(usage(format!("unexpected argument \"{extra}\""))),
            None => Ok(()),
        }
    }

    pub fn identifier(&self, what: &str) -> Result<i64> {
        let [only] = self.args.as_slice() else {
            return Err(usage(format!(
                "give exactly one {what} identifier: run \"{} {} list\" to see them",
                meta::SLUG,
                self.parent
            )));
        };
        match only.parse::<i64>() {
            Ok(id) if id > 0 => Ok(id),
            _ => Err(usage(format!("\"{only}\" is not a {what} identifier: use the number from the ID column"))),
        }
    }
}

fn parse_switch(value: &str) -> Option<&'static str> {
    match value.to_lowercase().as_str() {
        "true" | "1" | "t" | "yes" | "on" => Some("true"),
        "false" | "0" | "f" | "no" | "off" => Some("false"),
        _ => None,
    }
}

type Parsed = (BTreeMap<&'static str, String>, BTreeSet<&'static str>, Vec<String>);

pub(crate) fn parse_flags(flags: &[Flag], arguments: &[String]) -> std::result::Result<Parsed, String> {
    let mut values: BTreeMap<&'static str, String> =
        flags.iter().map(|flag| (flag.name, flag.default.to_string())).collect();
    let mut given = BTreeSet::new();
    let mut positional = Vec::new();
    let mut index = 0;
    while index < arguments.len() {
        let argument = &arguments[index];
        index += 1;
        if argument == "--" {
            positional.extend_from_slice(&arguments[index..]);
            break;
        }
        if !argument.starts_with('-') || argument == "-" {
            positional.push(argument.clone());
            continue;
        }
        let stripped = argument.strip_prefix("--").unwrap_or_else(|| &argument[1..]);
        let (name, inline) = match stripped.split_once('=') {
            Some((name, value)) => (name, Some(value.to_string())),
            None => (stripped, None),
        };
        let Some(flag) = flags.iter().find(|flag| flag.name == name) else {
            return Err(format!("unknown flag --{name}"));
        };
        let value = match (flag.kind, inline) {
            (Kind::Switch, None) => "true".to_string(),
            (Kind::Switch, Some(value)) => parse_switch(&value)
                .ok_or_else(|| format!("flag --{name} needs true or false, found \"{value}\""))?
                .to_string(),
            (_, Some(value)) => value,
            (_, None) => {
                let Some(next) = arguments.get(index) else {
                    return Err(format!("flag --{name} needs a value"));
                };
                index += 1;
                next.clone()
            }
        };
        if flag.kind == Kind::Number && value.parse::<i64>().is_err() {
            return Err(format!("flag --{name} needs a whole number, found \"{value}\""));
        }
        values.insert(flag.name, value);
        given.insert(flag.name);
    }
    Ok((values, given, positional))
}

fn parse_globals(arguments: &[String]) -> std::result::Result<(Globals, Vec<String>), String> {
    let mut globals = Globals::default();
    let mut index = 0;
    while index < arguments.len() {
        let argument = &arguments[index];
        if argument == "--" {
            index += 1;
            break;
        }
        if !argument.starts_with('-') || argument == "-" {
            break;
        }
        index += 1;
        let stripped = argument.strip_prefix("--").unwrap_or_else(|| &argument[1..]);
        let (name, inline) = match stripped.split_once('=') {
            Some((name, value)) => (name, Some(value.to_string())),
            None => (stripped, None),
        };
        let name = if name == "h" { "help" } else { name };
        let Some(flag) = GLOBAL_FLAGS.iter().find(|flag| flag.name == name) else {
            return Err(format!("unknown flag --{name}"));
        };
        if flag.kind == Kind::Switch {
            let value = match inline {
                None => true,
                Some(value) => {
                    parse_switch(&value)
                        .ok_or_else(|| format!("flag --{name} needs true or false, found \"{value}\""))?
                        == "true"
                }
            };
            match name {
                "ask" => globals.ask = value,
                "no-prompt" => globals.no_prompt = value,
                "help" => globals.help = value,
                _ => globals.version = value,
            }
            continue;
        }
        let value = match inline {
            Some(value) => value,
            None => {
                let Some(next) = arguments.get(index) else {
                    return Err(format!("flag --{name} needs a value"));
                };
                index += 1;
                next.clone()
            }
        };
        match name {
            "db" => globals.database = value,
            "ui" => globals.ui = value,
            _ => {
                let Some((key, raw)) = value.split_once('=').filter(|(key, _)| !key.is_empty()) else {
                    return Err(format!("invalid value \"{value}\" for flag --set: use the form key=value"));
                };
                if settings::lookup(key).is_none() {
                    return Err(format!(
                        "invalid value \"{value}\" for flag --set: unknown setting \"{key}\": run \"{} config list\" to see the keys",
                        meta::SLUG
                    ));
                }
                globals.overrides.insert(key.to_string(), raw.to_string());
            }
        }
    }
    Ok((globals, arguments[index..].to_vec()))
}

pub(crate) fn tabulate(text: &str) -> String {
    let lines: Vec<&str> = text.split('\n').collect();
    let mut out = String::new();
    let mut start = 0;
    while start < lines.len() {
        if !lines[start].contains('\t') {
            out.push_str(lines[start]);
            if start + 1 < lines.len() {
                out.push('\n');
            }
            start += 1;
            continue;
        }
        let mut end = start;
        while end < lines.len() && lines[end].contains('\t') {
            end += 1;
        }
        let rows: Vec<Vec<&str>> = lines[start..end].iter().map(|line| line.split('\t').collect()).collect();
        let mut widths: Vec<usize> = Vec::new();
        for row in &rows {
            for (column, cell) in row.iter().enumerate().take(row.len().saturating_sub(1)) {
                if widths.len() <= column {
                    widths.resize(column + 1, 0);
                }
                widths[column] = widths[column].max(cell.chars().count() + 2);
            }
        }
        for row in &rows {
            let mut line = String::new();
            for (column, cell) in row.iter().enumerate() {
                line.push_str(cell);
                if column + 1 < row.len() {
                    let width = widths.get(column).copied().unwrap_or(0);
                    line.extend(std::iter::repeat_n(' ', width.saturating_sub(cell.chars().count())));
                }
            }
            out.push_str(line.trim_end());
            out.push('\n');
        }
        start = end;
        if start == lines.len() {
            out.pop();
        }
    }
    out
}

impl Env<'_> {
    pub(crate) fn print(&mut self, text: impl AsRef<str>) {
        let _ = self.stdout.write_all(text.as_ref().as_bytes());
    }

    pub(crate) fn line(&mut self, text: impl AsRef<str>) {
        self.print(text);
        self.print("\n");
    }

    pub(crate) fn note(&mut self, text: impl AsRef<str>) {
        let _ = self.stderr.write_all(text.as_ref().as_bytes());
        let _ = self.stderr.write_all(b"\n");
    }

    pub(crate) fn table(&mut self, header: &[&str], rows: &[Vec<String>]) {
        let mut text = header.join("\t");
        text.push('\n');
        for row in rows {
            let cells: Vec<String> = row.iter().map(|cell| cell.replace(['\t', '\n', '\r'], " ")).collect();
            text.push_str(&cells.join("\t"));
            text.push('\n');
        }
        let aligned = tabulate(&text);
        self.print(aligned);
    }

    pub(crate) fn json<T: serde::Serialize>(&mut self, value: &T) -> Result<()> {
        let text = serde_json::to_string_pretty(value)?;
        self.line(text);
        Ok(())
    }

    pub(crate) fn interactive(&self) -> bool {
        self.session.stdin_terminal && !self.session.ci()
    }

    pub(crate) fn confirm(&mut self, call: &Call, app: Option<&App>, statement: &str) -> Result<()> {
        if call.truth("yes") {
            return Ok(());
        }
        if app.is_some_and(|app| matches!(app.settings().flag(settings::KEY_CONFIRM), Ok(false))) {
            return Ok(());
        }
        if !self.interactive() {
            return Err(usage(format!(
                "{statement} This needs confirmation: repeat the command with --yes, or use --dry-run to preview"
            )));
        }
        let _ = write!(self.stderr, "{statement}\nType yes to continue: ");
        let _ = self.stderr.flush();
        let mut answer = Vec::new();
        let _ = std::io::Read::take(&mut *self.stdin, 256).read_until(b'\n', &mut answer);
        if String::from_utf8_lossy(&answer).trim().to_lowercase() != "yes" {
            return Err(failure("cancelled, nothing was changed"));
        }
        Ok(())
    }

    pub(crate) fn app_options(&self, globals: &Globals, read_only: bool) -> Options {
        Options {
            database_path: if globals.database.is_empty() { None } else { Some(globals.database.clone().into()) },
            overrides: globals.overrides.clone(),
            read_only,
            ..self.options.clone()
        }
    }
}

fn root() -> Command {
    Command::group(
        meta::SLUG,
        "",
        vec![
            records::overview_command(),
            records::records_command(),
            records::demo_command(),
            notifications::notifications_command(),
            notifications::notifier_command(),
            config::config_command(),
            config::database_command(),
            Command::leaf("diagnostics", "Show build, database and settings status", config::run_diagnostics),
            Command::standalone("guide", "Read or search the usage guide", config::run_guide).args("[search terms]"),
            Command::leaf("about", "Show version, source and bug report links", config::run_about)
                .flags(&[switch("issue", "print the bug report preview and its link")]),
            Command::standalone("licenses", "Show the license and third-party notices", config::run_licenses),
            Command::standalone("version", "Print the version", |env, _| {
                env.line(format!("{} {}", meta::SLUG, meta::VERSION));
                Ok(())
            }),
        ],
    )
}

fn describe_flags(flags: &[Flag]) -> Vec<String> {
    let mut sorted: Vec<&Flag> = flags.iter().collect();
    sorted.sort_by_key(|flag| flag.name);
    sorted
        .iter()
        .map(|flag| {
            let mut line = format!("  --{}", flag.name);
            if flag.kind != Kind::Switch {
                line.push_str(" <value>");
            }
            line.push('\t');
            line.push_str(flag.help);
            if !matches!(flag.default, "" | "false" | "0") {
                line.push_str(&format!(" (default {})", flag.default));
            }
            line
        })
        .collect()
}

fn help_text(target: &Command, path: &str) -> String {
    let mut text = String::new();
    if path.is_empty() {
        text.push_str(&format!("{} {}\n{}\n\n", meta::APP_NAME, meta::VERSION, meta::TAGLINE));
        text.push_str(&format!(
            "Usage:\n  {slug}\topen the dashboard or the terminal UI\n  {slug} <command> [arguments]\trun a command\n\nCommands:\n",
            slug = meta::SLUG
        ));
    } else {
        let mut usage_line = format!("{} {path}", meta::SLUG);
        if !target.sub.is_empty() {
            usage_line.push_str(" <command>");
        }
        if !target.args.is_empty() {
            usage_line.push(' ');
            usage_line.push_str(target.args);
        }
        if !target.flags.is_empty() {
            usage_line.push_str(" [flags]");
        }
        text.push_str(&format!("{}\n\nUsage:\n  {usage_line}\n", target.summary));
        if !target.sub.is_empty() {
            text.push_str("\nCommands:\n");
        }
    }
    for child in &target.sub {
        text.push_str(&format!("  {}\t{}\n", child.name, child.summary));
    }
    if !path.is_empty() && !target.flags.is_empty() {
        text.push_str("\nFlags:\n");
        for line in describe_flags(&target.flags) {
            text.push_str(&line);
            text.push('\n');
        }
    }
    text.push_str("\nGlobal flags:\n");
    for line in describe_flags(GLOBAL_FLAGS) {
        text.push_str(&line);
        text.push('\n');
    }
    if path.is_empty() {
        text.push_str(&format!(
            "\nEnvironment:\n  {}\tgui or tui, same as --ui\n  {}\tdatabase file, same as --db\n  {}\tdirectory for the saved database path and front-end choice\n  NO_COLOR\tturn colors off in the terminal UI\n",
            meta::env_name("ui"),
            meta::env_name("db"),
            meta::env_name("config_dir")
        ));
        text.push_str(&format!("\nRun \"{} help <command>\" for details.\n", meta::SLUG));
    }
    tabulate(&text)
}

fn leaves<'c>(parent: &'c Command, prefix: &str, all: &mut Vec<(String, &'c Command)>) {
    for child in &parent.sub {
        let path = if prefix.is_empty() { child.name.to_string() } else { format!("{prefix} {}", child.name) };
        if child.sub.is_empty() {
            all.push((path.clone(), child));
        }
        leaves(child, &path, all);
    }
}

pub fn command_paths() -> Vec<String> {
    let tree = root();
    let mut all = Vec::new();
    leaves(&tree, "", &mut all);
    let mut paths: Vec<String> = all.into_iter().map(|(path, _)| path).collect();
    paths.sort();
    paths
}

pub fn reference() -> String {
    let tree = root();
    let mut all = Vec::new();
    leaves(&tree, "", &mut all);
    let mut table = String::from("| Command | Arguments and flags | What it does |\n|---|---|---|\n");
    for (path, leaf) in all {
        let mut parts = Vec::new();
        if !leaf.args.is_empty() {
            parts.push(format!("`{}`", leaf.args.replace('|', "\\|")));
        }
        let mut names: Vec<&str> = leaf.flags.iter().map(|flag| flag.name).collect();
        names.sort_unstable();
        parts.extend(names.iter().map(|name| format!("`--{name}`")));
        if parts.is_empty() {
            parts.push("none".to_string());
        }
        table.push_str(&format!("| `{} {path}` | {} | {} |\n", meta::SLUG, parts.join(" "), leaf.summary));
    }
    table
}

fn suggestion(parent: &Command, path: &str, word: &str) -> String {
    let full = |name: &str| format!("{path} {name}").trim().to_string();
    for child in &parent.sub {
        if child.name.starts_with(word) || word.starts_with(child.name) {
            return format!("Did you mean \"{}\"?", full(child.name));
        }
    }
    let names: Vec<&str> = parent.sub.iter().map(|child| child.name).collect();
    format!("Available: {}", names.join(", "))
}

fn report(env: &mut Env<'_>, outcome: Result<()>) -> i32 {
    match outcome {
        Ok(()) => EXIT_OK,
        Err(error) => {
            env.note(format!("error: {error}"));
            if error.is_usage() { EXIT_USAGE } else { EXIT_FAILURE }
        }
    }
}

pub fn run(env: &mut Env<'_>) -> i32 {
    let arguments = env.args.clone();
    let (mut globals, mut rest) = match parse_globals(&arguments) {
        Ok(parsed) => parsed,
        Err(problem) => {
            env.note(format!("error: {problem}\nRun \"{} --help\" to see the flags.", meta::SLUG));
            return EXIT_USAGE;
        }
    };
    if globals.version {
        env.line(format!("{} {}", meta::SLUG, meta::VERSION));
        return EXIT_OK;
    }
    if rest.first().is_some_and(|word| word == "help") {
        globals.help = true;
        rest.remove(0);
    }
    let tree = root();
    let mut target = &tree;
    let mut path = String::new();
    let mut parent = String::new();
    while let Some(word) = rest.first() {
        if target.sub.is_empty() || word == "--help" || word == "-h" {
            break;
        }
        let Some(next) = target.find(word) else {
            let unknown = format!("{path} {word}").trim().to_string();
            env.note(format!("error: unknown command \"{unknown}\"\n{}", suggestion(target, &path, word)));
            return EXIT_USAGE;
        };
        parent = path.clone();
        path = format!("{path} {}", next.name).trim().to_string();
        target = next;
        rest.remove(0);
    }
    if rest.iter().take_while(|word| *word != "--").any(|word| word == "--help" || word == "-h") {
        globals.help = true;
    }
    if globals.help {
        let text = help_text(target, &path);
        env.print(text);
        return EXIT_OK;
    }
    if path.is_empty() {
        return launch(env, &globals);
    }
    let Some(runner) = target.run else {
        let text = help_text(target, &path);
        env.note(text.trim_end());
        return EXIT_USAGE;
    };
    let outcome = execute(env, globals, target, runner, path, parent, &rest);
    report(env, outcome)
}

fn execute(
    env: &mut Env<'_>,
    globals: Globals,
    target: &Command,
    runner: Run,
    path: String,
    parent: String,
    arguments: &[String],
) -> Result<()> {
    let (values, given, args) = parse_flags(&target.flags, arguments)
        .map_err(|problem| usage(format!("{problem}: run \"{} help {path}\" to see the flags", meta::SLUG)))?;
    let call = Call { globals, parent, args, values, given };
    match runner {
        Run::Standalone(run) => run(env, &call),
        Run::Database(run) => {
            let mut app = App::open(env.app_options(&call.globals, false))?;
            if app.created_database() {
                env.note(format!("Created a new database at {}", app.location().path.display()));
            }
            run(env, &call, &mut app)
        }
    }
}

fn terminal_window(env: &mut Env<'_>, globals: &Globals) -> Result<()> {
    let Some(open) = env.hooks.open_terminal_window else {
        return Err(failure(
            "no terminal window can be opened from here: start the application from a terminal with --ui tui",
        ));
    };
    let mut arguments = vec!["--ui".to_string(), "tui".to_string()];
    if !globals.database.is_empty() {
        arguments.push("--db".to_string());
        arguments.push(globals.database.clone());
    }
    for (key, value) in &globals.overrides {
        arguments.push("--set".to_string());
        arguments.push(format!("{key}={value}"));
    }
    open(&env.executable, &arguments)
}

fn launch(env: &mut Env<'_>, globals: &Globals) -> i32 {
    let location = match service::locate(&env.app_options(globals, false)) {
        Ok(location) => location,
        Err(error) => return report(env, Err(error)),
    };
    let input = env.session.input(launcher::Input {
        ui_flag: globals.ui.clone(),
        ask: globals.ask,
        no_prompt: globals.no_prompt,
        env_ui: env.options.env.get(&meta::env_name("ui")).unwrap_or_default(),
        remembered: location.config.front_end.clone(),
        gui_available: env.hooks.open_gui.is_some(),
        ..launcher::Input::default()
    });
    let decision = launcher::decide(&input);
    let mut remember = false;
    let mut mode = match decision.action {
        Action::Fail => return report(env, Err(usage(decision.reason))),
        Action::PrintHelp | Action::RunCommand => {
            env.note(format!("{}: {}.\n", meta::APP_NAME, decision.reason));
            let text = help_text(&root(), "");
            env.note(text.trim_end());
            return EXIT_OK;
        }
        Action::OpenGui => launcher::MODE_GUI.to_string(),
        Action::OpenTui => {
            if input.headless && globals.ui.is_empty() {
                env.note(format!("{}: {}.", meta::APP_NAME, decision.reason));
            }
            launcher::MODE_TUI.to_string()
        }
        Action::OpenTerminalWindow => {
            let outcome = terminal_window(env, globals);
            return report(env, outcome);
        }
        Action::PromptTerminal | Action::ShowChooser => {
            let asked = if decision.action == Action::PromptTerminal {
                launcher::prompt_terminal(&mut *env.stdin, &mut *env.stdout, &decision.default).map_err(AppError::from)
            } else {
                match env.hooks.show_chooser {
                    Some(chooser) => chooser(&decision.default),
                    None => Err(failure("this build cannot show the chooser window: pass --ui gui or --ui tui")),
                }
            };
            match asked {
                Err(error) => return report(env, Err(error)),
                Ok(result) if result.quit => return EXIT_OK,
                Ok(result) => {
                    remember = result.remember;
                    result.mode
                }
            }
        }
    };
    let mut app = match App::open(env.app_options(globals, false)) {
        Ok(app) => app,
        Err(error) => return report(env, Err(error)),
    };
    if remember && let Err(error) = app.remember_front_end(&mode) {
        env.note(format!("warning: the choice could not be saved: {error}"));
    }
    while !mode.is_empty() {
        let terminal = input.stdin_interactive && input.terminal_output;
        let next = match (mode.as_str(), env.hooks.open_gui, env.hooks.open_tui) {
            (launcher::MODE_GUI, Some(open), _) => open(&mut app),
            (launcher::MODE_TUI, _, _) if !terminal => {
                let outcome = terminal_window(env, globals);
                return report(env, outcome);
            }
            (launcher::MODE_TUI, _, Some(open)) => open(&mut app),
            _ => Err(failure(format!("this build cannot open the {mode} front end"))),
        };
        match next {
            Ok(next) => mode = next,
            Err(error) => return report(env, Err(error)),
        }
    }
    EXIT_OK
}

pub(crate) fn enabled_text(enabled: bool) -> &'static str {
    if enabled { "yes" } else { "no" }
}

pub(crate) fn capitalize(text: &str) -> String {
    let mut letters = text.chars();
    match letters.next() {
        Some(first) => first.to_uppercase().chain(letters).collect(),
        None => String::new(),
    }
}
