use std::collections::BTreeMap;

use rusqlite::{Connection, OptionalExtension};

use crate::clock::Clock;
use crate::error::{AppError, Result};
use crate::meta;
use crate::platform::Env;
use crate::theme;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    Enum,
    Bool,
    Int,
    Color,
    Text,
    Columns,
    Sort,
}

impl Kind {
    pub fn label(self) -> &'static str {
        match self {
            Kind::Enum => "enum",
            Kind::Bool => "bool",
            Kind::Int => "int",
            Kind::Color => "color",
            Kind::Text => "text",
            Kind::Columns => "columns",
            Kind::Sort => "sort",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Source {
    Default,
    Stored,
    Environment,
    Flag,
}

impl Source {
    pub fn label(self) -> &'static str {
        match self {
            Source::Default => "default",
            Source::Stored => "stored",
            Source::Environment => "environment",
            Source::Flag => "flag",
        }
    }
}

pub const KEY_MODE: &str = "appearance.mode";
pub const KEY_THEME: &str = "appearance.theme";
pub const KEY_ACCENT: &str = "appearance.accent";
pub const KEY_DENSITY: &str = "appearance.density";
pub const KEY_TEXT_SCALE: &str = "appearance.text_scale";
pub const KEY_REDUCED_MOTION: &str = "appearance.reduced_motion";
pub const KEY_MONOCHROME: &str = "appearance.monochrome";
pub const KEY_TIME_ZONE: &str = "display.time_zone";
pub const KEY_SIDEBAR_COLLAPSED: &str = "window.sidebar_collapsed";
pub const KEY_WINDOW_WIDTH: &str = "window.width";
pub const KEY_WINDOW_HEIGHT: &str = "window.height";
pub const KEY_REFRESH_SECONDS: &str = "data.refresh_seconds";
pub const KEY_CONFIRM: &str = "safety.confirm_destructive";
pub const KEY_NOTIFIER_INTERVAL: &str = "notifier.interval_seconds";
pub const KEY_NOTIFIER_TITLES: &str = "notifier.show_titles";
pub const KEY_RECORD_COLUMNS: &str = "table.records.columns";
pub const KEY_RECORD_SORT: &str = "table.records.sort";

const SQL_SELECT_ONE: &str = "SELECT value FROM settings WHERE key = ?1";
const SQL_SELECT_ALL: &str = "SELECT key, value FROM settings ORDER BY key";
const SQL_UPSERT: &str = "INSERT INTO settings (key, value, updated_at) VALUES (?1, ?2, ?3) ON CONFLICT (key) DO UPDATE SET value = excluded.value, updated_at = excluded.updated_at";
const SQL_DELETE: &str = "DELETE FROM settings WHERE key = ?1";

pub const RECORD_COLUMNS: &[&str] = &["id", "name", "status", "severity", "updated", "state"];
pub const RECORD_SORT_KEYS: &[&str] = &["name", "status", "severity", "updated"];

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Definition {
    pub key: &'static str,
    pub group: &'static str,
    pub label: &'static str,
    pub help: &'static str,
    pub kind: Kind,
    pub default: &'static str,
    pub options: &'static [&'static str],
    pub pinned: &'static [&'static str],
    pub min: i64,
    pub max: i64,
}

const fn definition(
    key: &'static str,
    group: &'static str,
    label: &'static str,
    help: &'static str,
    kind: Kind,
    default: &'static str,
) -> Definition {
    Definition { key, group, label, help, kind, default, options: &[], pinned: &[], min: 0, max: 0 }
}

const fn choice(mut base: Definition, options: &'static [&'static str]) -> Definition {
    base.options = options;
    base
}

const fn range(mut base: Definition, min: i64, max: i64) -> Definition {
    base.min = min;
    base.max = max;
    base
}

const fn pinned(mut base: Definition, columns: &'static [&'static str]) -> Definition {
    base.pinned = columns;
    base
}

pub const DEFINITIONS: &[Definition] = &[
    choice(
        definition(KEY_MODE, "Appearance", "Mode", "Light, dark or follow the operating system.", Kind::Enum, "system"),
        &["system", "light", "dark"],
    ),
    choice(
        definition(
            KEY_THEME,
            "Appearance",
            "Color theme",
            "Named palette for accents and surfaces.",
            Kind::Enum,
            "slate",
        ),
        &["slate", "blue", "green", "crimson", "amber", "purple", "contrast"],
    ),
    definition(
        KEY_ACCENT,
        "Appearance",
        "Custom accent",
        "Optional accent color as #RRGGBB. It is adjusted when needed to keep WCAG AA contrast. Empty uses the theme accent.",
        Kind::Color,
        "",
    ),
    choice(
        definition(
            KEY_DENSITY,
            "Appearance",
            "Density",
            "Row height and spacing of tables and lists.",
            Kind::Enum,
            "comfortable",
        ),
        &["comfortable", "compact"],
    ),
    range(
        definition(KEY_TEXT_SCALE, "Appearance", "Text size", "Text size in percent of the default.", Kind::Int, "100"),
        80,
        200,
    ),
    definition(
        KEY_REDUCED_MOTION,
        "Appearance",
        "Reduced motion",
        "Turn off animations and transitions.",
        Kind::Bool,
        "false",
    ),
    definition(
        KEY_MONOCHROME,
        "Appearance",
        "Monochrome terminal",
        "Draw the terminal UI without colors.",
        Kind::Bool,
        "false",
    ),
    choice(
        definition(
            KEY_TIME_ZONE,
            "Units",
            "Time zone",
            "Show timestamps in local time or in UTC.",
            Kind::Enum,
            "local",
        ),
        &["local", "utc"],
    ),
    definition(
        KEY_SIDEBAR_COLLAPSED,
        "Window",
        "Sidebar collapsed",
        "Show the sidebar as icons only.",
        Kind::Bool,
        "false",
    ),
    range(
        definition(KEY_WINDOW_WIDTH, "Window", "Window width", "Dashboard window width in pixels.", Kind::Int, "1180"),
        640,
        10000,
    ),
    range(
        definition(
            KEY_WINDOW_HEIGHT,
            "Window",
            "Window height",
            "Dashboard window height in pixels.",
            Kind::Int,
            "760",
        ),
        480,
        10000,
    ),
    range(
        definition(
            KEY_REFRESH_SECONDS,
            "Data",
            "Terminal refresh interval",
            "Seconds between automatic refreshes of the terminal UI.",
            Kind::Int,
            "2",
        ),
        1,
        60,
    ),
    definition(
        KEY_CONFIRM,
        "Safe defaults",
        "Confirm destructive actions",
        "Ask before deleting or overwriting data.",
        Kind::Bool,
        "true",
    ),
    range(
        definition(
            KEY_NOTIFIER_INTERVAL,
            "Notifications",
            "Notifier interval",
            "Seconds between checks of the background notifier.",
            Kind::Int,
            "60",
        ),
        15,
        3600,
    ),
    definition(
        KEY_NOTIFIER_TITLES,
        "Notifications",
        "Show titles in system notifications",
        "Off shows only a count. On adds the notification title, never record content.",
        Kind::Bool,
        "false",
    ),
    pinned(
        choice(
            definition(
                KEY_RECORD_COLUMNS,
                "Tables",
                "Record table columns",
                "Columns of the record table, separated by commas. The name column is always shown.",
                Kind::Columns,
                "id,name,status,severity,updated,state",
            ),
            RECORD_COLUMNS,
        ),
        &["name"],
    ),
    choice(
        definition(
            KEY_RECORD_SORT,
            "Tables",
            "Record table sort",
            "Sort column and direction of the record table, for example updated:desc.",
            Kind::Sort,
            "updated:desc",
        ),
        RECORD_SORT_KEYS,
    ),
];

const UNKNOWN_HELP: &str = "Stored by another version of the application. It is kept and not used.";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Value {
    pub definition: Definition,
    pub key: String,
    pub raw: String,
    pub stored: Option<String>,
    pub source: Source,
    pub overridden_by: String,
    pub override_error: String,
    pub unknown: bool,
}

impl Value {
    pub fn bool(&self) -> bool {
        self.raw == "true"
    }

    pub fn int(&self) -> i64 {
        self.raw.parse().unwrap_or(0)
    }

    pub fn locked(&self) -> bool {
        matches!(self.source, Source::Environment | Source::Flag)
    }
}

pub fn lookup(key: &str) -> Option<&'static Definition> {
    DEFINITIONS.iter().find(|definition| definition.key == key)
}

pub fn env_name(key: &str) -> String {
    meta::env_name(key)
}

pub fn split_columns(value: &str) -> Vec<String> {
    value.split(',').map(str::trim).filter(|column| !column.is_empty()).map(str::to_string).collect()
}

pub fn split_sort(value: &str) -> (String, bool) {
    match value.split_once(':') {
        Some((key, direction)) => (key.to_string(), direction == "desc"),
        None => (value.to_string(), false),
    }
}

fn unknown_setting(key: &str) -> AppError {
    AppError::invalid(key, "unknown setting", "List the settings to see the available keys")
}

pub fn normalize(definition: &Definition, raw: &str) -> Result<String> {
    let value = raw.trim();
    let fail = |problem: String, fix: String| Err(AppError::invalid(definition.key, problem, fix));
    match definition.kind {
        Kind::Enum => match definition.options.iter().find(|option| option.eq_ignore_ascii_case(value)) {
            Some(option) => Ok((*option).to_string()),
            None => fail(
                format!("\"{raw}\" is not one of the allowed values"),
                format!("Use one of: {}", definition.options.join(", ")),
            ),
        },
        Kind::Bool => match value.to_lowercase().as_str() {
            "true" | "on" | "yes" | "1" => Ok("true".to_string()),
            "false" | "off" | "no" | "0" => Ok("false".to_string()),
            _ => fail(format!("\"{raw}\" is not a yes or no value"), "Use true or false".to_string()),
        },
        Kind::Int => {
            let fix = format!("Use a number from {} to {}", definition.min, definition.max);
            match value.parse::<i64>() {
                Err(_) => fail(format!("\"{raw}\" is not a whole number"), fix),
                Ok(number) if number < definition.min || number > definition.max => {
                    fail(format!("{number} is outside the allowed range"), fix)
                }
                Ok(number) => Ok(number.to_string()),
            }
        }
        Kind::Color => {
            if value.is_empty() {
                return Ok(String::new());
            }
            if !value.starts_with('#') || theme::Color::parse(value).is_err() {
                return fail(
                    format!("\"{raw}\" is not a color"),
                    "Use the form #RRGGBB, for example #1257C4, or leave it empty".to_string(),
                );
            }
            Ok(value.to_uppercase())
        }
        Kind::Columns => {
            let fix = format!("Use some of: {}", definition.options.join(", "));
            if value.len() > 400 {
                return fail("the list is too long".to_string(), fix);
            }
            let wanted = split_columns(&value.to_lowercase());
            if let Some(stranger) = wanted.iter().find(|column| !definition.options.contains(&column.as_str())) {
                return fail(format!("\"{stranger}\" is not a column of this table"), fix);
            }
            let ordered: Vec<&str> = definition
                .options
                .iter()
                .copied()
                .filter(|option| definition.pinned.contains(option) || wanted.iter().any(|column| column == option))
                .collect();
            Ok(ordered.join(","))
        }
        Kind::Sort => {
            let lowered = value.to_lowercase();
            let (key, direction) = match lowered.split_once(':') {
                Some((key, direction)) => (key.trim(), direction.trim()),
                None => (lowered.trim(), "asc"),
            };
            let fix = format!(
                "Use a column ({}) followed by :asc or :desc, for example {}",
                definition.options.join(", "),
                definition.default
            );
            if !definition.options.contains(&key) {
                return fail(format!("\"{key}\" is not a sortable column"), fix);
            }
            if direction != "asc" && direction != "desc" {
                return fail(format!("\"{direction}\" is not a direction"), fix);
            }
            Ok(format!("{key}:{direction}"))
        }
        Kind::Text => Ok(value.to_string()),
    }
}

pub struct Settings<'a> {
    conn: &'a Connection,
    clock: Clock,
    env: Env,
    flags: &'a BTreeMap<String, String>,
}

impl<'a> Settings<'a> {
    pub fn new(conn: &'a Connection, clock: Clock, env: Env, flags: &'a BTreeMap<String, String>) -> Self {
        Settings { conn, clock, env, flags }
    }

    fn resolve(&self, definition: &Definition, stored: Option<String>) -> Value {
        let mut value = Value {
            definition: definition.clone(),
            key: definition.key.to_string(),
            raw: definition.default.to_string(),
            stored: stored.clone(),
            source: Source::Default,
            overridden_by: String::new(),
            override_error: String::new(),
            unknown: false,
        };
        if let Some(text) = stored {
            match normalize(definition, &text) {
                Ok(normalized) => {
                    value.raw = normalized;
                    value.source = Source::Stored;
                }
                Err(error) => {
                    value.override_error = format!("stored value is invalid and the default is used: {error}")
                }
            }
        }
        let variable = env_name(definition.key);
        if let Some(text) = self.env.get(&variable) {
            match normalize(definition, &text) {
                Ok(normalized) => {
                    value.raw = normalized;
                    value.source = Source::Environment;
                    value.overridden_by = format!("environment variable {variable}");
                }
                Err(error) => value.override_error = format!("environment variable {variable} is ignored: {error}"),
            }
        }
        if let Some(text) = self.flags.get(definition.key) {
            match normalize(definition, text) {
                Ok(normalized) => {
                    value.raw = normalized;
                    value.source = Source::Flag;
                    value.overridden_by = format!("command-line flag --set {}", definition.key);
                }
                Err(error) => {
                    value.override_error = format!("command-line flag --set {} is ignored: {error}", definition.key);
                }
            }
        }
        value
    }

    pub fn get(&self, key: &str) -> Result<Value> {
        let definition = lookup(key).ok_or_else(|| unknown_setting(key))?;
        let stored: Option<String> = self.conn.query_row(SQL_SELECT_ONE, [key], |row| row.get(0)).optional()?;
        Ok(self.resolve(definition, stored))
    }

    pub fn list(&self) -> Result<Vec<Value>> {
        let mut statement = self.conn.prepare(SQL_SELECT_ALL)?;
        let rows = statement.query_map([], |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?)))?;
        let mut stored = BTreeMap::new();
        for row in rows {
            let (key, value) = row?;
            stored.insert(key, value);
        }
        let mut values: Vec<Value> =
            DEFINITIONS.iter().map(|definition| self.resolve(definition, stored.remove(definition.key))).collect();
        for (key, text) in stored {
            values.push(Value {
                definition: definition("", "Unknown", "", UNKNOWN_HELP, Kind::Text, ""),
                key,
                raw: text.clone(),
                stored: Some(text),
                source: Source::Stored,
                overridden_by: String::new(),
                override_error: String::new(),
                unknown: true,
            });
        }
        Ok(values)
    }

    pub fn set(&self, key: &str, raw: &str) -> Result<Value> {
        let definition = lookup(key).ok_or_else(|| unknown_setting(key))?;
        let normalized = normalize(definition, raw)?;
        self.conn.execute(SQL_UPSERT, (key, &normalized, self.clock.now().to_rfc3339()))?;
        Ok(self.resolve(definition, Some(normalized)))
    }

    pub fn reset(&self, key: &str) -> Result<Value> {
        let definition = lookup(key).ok_or_else(|| unknown_setting(key))?;
        self.conn.execute(SQL_DELETE, [key])?;
        Ok(self.resolve(definition, None))
    }

    pub fn effective(&self) -> Result<BTreeMap<String, Value>> {
        Ok(self.list()?.into_iter().map(|value| (value.key.clone(), value)).collect())
    }

    pub fn text(&self, key: &str) -> Result<String> {
        Ok(self.get(key)?.raw)
    }

    pub fn flag(&self, key: &str) -> Result<bool> {
        Ok(self.get(key)?.bool())
    }

    pub fn number(&self, key: &str) -> Result<i64> {
        Ok(self.get(key)?.int())
    }
}

pub fn groups(values: &[Value]) -> Vec<&'static str> {
    let mut seen = Vec::new();
    for value in values {
        if !seen.contains(&value.definition.group) {
            seen.push(value.definition.group);
        }
    }
    seen
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::store::Store;
    use crate::testkit::{TempDir, fixed_time};
    use proptest::prelude::*;

    fn store(directory: &TempDir) -> Store {
        Store::open_ready(&directory.path().join("app.db")).unwrap()
    }

    fn service<'a>(store: &'a Store, env: &[(&str, &str)], flags: &'a BTreeMap<String, String>) -> Settings<'a> {
        Settings::new(store.conn(), Clock::fixed(fixed_time()), Env::from_pairs(env), flags)
    }

    #[test]
    fn every_default_is_valid_and_every_key_is_unique() {
        let mut keys: Vec<&str> = DEFINITIONS.iter().map(|definition| definition.key).collect();
        for definition in DEFINITIONS {
            assert_eq!(normalize(definition, definition.default).unwrap(), definition.default, "{}", definition.key);
            assert!(!definition.label.is_empty() && !definition.help.is_empty() && !definition.group.is_empty());
        }
        keys.sort_unstable();
        keys.dedup();
        assert_eq!(keys.len(), DEFINITIONS.len());
        assert_eq!(lookup(KEY_THEME).unwrap().options, theme::names().as_slice());
    }

    #[test]
    fn values_are_stored_listed_and_reset() {
        let directory = TempDir::new("settings-store");
        let store = store(&directory);
        let flags = BTreeMap::new();
        let settings = service(&store, &[], &flags);
        let initial = settings.get(KEY_DENSITY).unwrap();
        assert_eq!(
            (initial.raw.as_str(), initial.source, initial.stored.clone()),
            ("comfortable", Source::Default, None)
        );
        let changed = settings.set(KEY_DENSITY, " Compact ").unwrap();
        assert_eq!((changed.raw.as_str(), changed.source), ("compact", Source::Stored));
        assert_eq!(settings.text(KEY_DENSITY).unwrap(), "compact");
        assert_eq!(settings.list().unwrap().len(), DEFINITIONS.len());
        assert_eq!(settings.effective().unwrap()[KEY_DENSITY].raw, "compact");
        assert_eq!(settings.reset(KEY_DENSITY).unwrap().source, Source::Default);
        assert!(settings.set("no.such.key", "x").unwrap_err().to_string().contains("unknown setting"));
        assert!(settings.get("no.such.key").is_err());
        assert!(settings.flag(KEY_CONFIRM).unwrap());
        assert_eq!(settings.number(KEY_TEXT_SCALE).unwrap(), 100);
    }

    #[test]
    fn invalid_input_is_rejected_with_the_problem_and_the_fix() {
        let cases = [
            (KEY_MODE, "neon", "Use one of: system, light, dark"),
            (KEY_TEXT_SCALE, "big", "Use a number from 80 to 200"),
            (KEY_TEXT_SCALE, "500", "Use a number from 80 to 200"),
            (KEY_CONFIRM, "maybe", "Use true or false"),
            (KEY_ACCENT, "blue", "Use the form #RRGGBB"),
            (KEY_RECORD_COLUMNS, "name,owner", "Use some of: id, name"),
            (KEY_RECORD_SORT, "owner:asc", "Use a column"),
            (KEY_RECORD_SORT, "name:sideways", "Use a column"),
        ];
        for (key, raw, fix) in cases {
            let error = normalize(lookup(key).unwrap(), raw).unwrap_err().to_string();
            assert!(error.contains(fix) && error.starts_with(key), "{key}={raw}: {error}");
        }
    }

    #[test]
    fn normalization_produces_canonical_text() {
        let check =
            |key: &str, raw: &str, expected: &str| assert_eq!(normalize(lookup(key).unwrap(), raw).unwrap(), expected);
        check(KEY_CONFIRM, "YES", "true");
        check(KEY_CONFIRM, "0", "false");
        check(KEY_TEXT_SCALE, " 0120 ", "120");
        check(KEY_ACCENT, "#1257c4", "#1257C4");
        check(KEY_ACCENT, "  ", "");
        check(KEY_RECORD_COLUMNS, "state, id", "id,name,state");
        check(KEY_RECORD_COLUMNS, "", "name");
        check(KEY_RECORD_SORT, "Severity", "severity:asc");
        check(KEY_RECORD_SORT, "updated : DESC", "updated:desc");
        assert_eq!(split_sort("updated:desc"), ("updated".to_string(), true));
        assert_eq!(split_columns("id, name,,state"), vec!["id", "name", "state"]);
    }

    #[test]
    fn flag_beats_environment_beats_stored_beats_default() {
        let directory = TempDir::new("settings-precedence");
        let store = store(&directory);
        let variable = env_name(KEY_THEME);
        let none = BTreeMap::new();
        let flags = BTreeMap::from([(KEY_THEME.to_string(), "amber".to_string())]);
        service(&store, &[], &none).set(KEY_THEME, "green").unwrap();
        assert_eq!(service(&store, &[], &none).get(KEY_THEME).unwrap().raw, "green");
        let from_env = service(&store, &[(variable.as_str(), "purple")], &none).get(KEY_THEME).unwrap();
        assert_eq!((from_env.raw.as_str(), from_env.source), ("purple", Source::Environment));
        assert!(from_env.overridden_by.contains(&variable) && from_env.locked());
        assert_eq!(from_env.stored.as_deref(), Some("green"));
        let from_flag = service(&store, &[(variable.as_str(), "purple")], &flags).get(KEY_THEME).unwrap();
        assert_eq!((from_flag.raw.as_str(), from_flag.source), ("amber", Source::Flag));
        assert!(from_flag.overridden_by.contains("--set appearance.theme"));
    }

    #[test]
    fn invalid_overrides_are_ignored_and_explained() {
        let directory = TempDir::new("settings-overrides");
        let store = store(&directory);
        let variable = env_name(KEY_THEME);
        let none = BTreeMap::new();
        let value = service(&store, &[(variable.as_str(), "neon")], &none).get(KEY_THEME).unwrap();
        assert_eq!((value.raw.as_str(), value.source), ("slate", Source::Default));
        assert!(value.override_error.contains(&variable) && value.override_error.contains("is ignored"));
        let flags = BTreeMap::from([(KEY_THEME.to_string(), "neon".to_string())]);
        let value = service(&store, &[], &flags).get(KEY_THEME).unwrap();
        assert!(value.override_error.contains("--set appearance.theme is ignored"), "{}", value.override_error);
    }

    #[test]
    fn stored_values_from_other_versions_are_kept_and_not_used() {
        let directory = TempDir::new("settings-unknown");
        let store = store(&directory);
        let none = BTreeMap::new();
        store.conn().execute(SQL_UPSERT, ("future.option", "42", "2026-03-02T09:30:00Z")).unwrap();
        store.conn().execute(SQL_UPSERT, (KEY_THEME, "neon", "2026-03-02T09:30:00Z")).unwrap();
        let settings = service(&store, &[], &none);
        let values = settings.list().unwrap();
        let unknown = values.iter().find(|value| value.key == "future.option").unwrap();
        assert!(unknown.unknown && unknown.raw == "42" && unknown.definition.group == "Unknown");
        let broken = settings.get(KEY_THEME).unwrap();
        assert_eq!(broken.raw, "slate");
        assert!(broken.override_error.contains("stored value is invalid"), "{}", broken.override_error);
        assert_eq!(groups(&values).last(), Some(&"Unknown"));
        assert_eq!(groups(&values)[0], "Appearance");
    }

    proptest! {
        #[test]
        fn normalize_is_idempotent_and_never_panics(index in 0usize..DEFINITIONS.len(), raw in ".{0,60}") {
            let definition = &DEFINITIONS[index];
            if let Ok(once) = normalize(definition, &raw) {
                prop_assert_eq!(normalize(definition, &once).unwrap(), once);
            }
        }
    }
}
