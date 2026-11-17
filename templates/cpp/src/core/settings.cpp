#include "core/settings.hpp"

#include <algorithm>

#include "core/error.hpp"
#include "core/meta.hpp"
#include "core/text.hpp"
#include "core/theme.hpp"

namespace app::settings {

namespace {

constexpr std::string_view sql_select_one = "SELECT value FROM settings WHERE key = ?1";
constexpr std::string_view sql_select_all = "SELECT key, value FROM settings ORDER BY key";
constexpr std::string_view sql_upsert =
    "INSERT INTO settings (key, value, updated_at) VALUES (?1, ?2, ?3) ON CONFLICT (key) DO UPDATE SET value = "
    "excluded.value, updated_at = excluded.updated_at";
constexpr std::string_view sql_delete = "DELETE FROM settings WHERE key = ?1";
constexpr std::string_view unknown_help = "Stored by another version of the application. It is kept and not used.";

Definition definition(std::string_view key, std::string_view group, std::string_view label, std::string_view help,
                      Kind kind, std::string_view fallback) {
  Definition out;
  out.key = key;
  out.group = group;
  out.label = label;
  out.help = help;
  out.kind = kind;
  out.fallback = fallback;
  return out;
}

Definition choice(Definition base, std::vector<std::string_view> options) {
  base.options = std::move(options);
  return base;
}

Definition range(Definition base, std::int64_t min, std::int64_t max) {
  base.min = min;
  base.max = max;
  return base;
}

Definition pinned(Definition base, std::vector<std::string_view> columns) {
  base.pinned = std::move(columns);
  return base;
}

std::vector<Definition> build_definitions() {
  return {
      choice(definition(key_mode, "Appearance", "Mode", "Light, dark or follow the operating system.", Kind::Enum,
                        "system"),
             {"system", "light", "dark"}),
      choice(definition(key_theme, "Appearance", "Color theme", "Named palette for accents and surfaces.", Kind::Enum,
                        "slate"),
             {"slate", "blue", "green", "crimson", "amber", "purple", "contrast"}),
      definition(key_accent, "Appearance", "Custom accent",
                 "Optional accent color as #RRGGBB. It is adjusted when needed to keep WCAG AA contrast. Empty uses "
                 "the theme accent.",
                 Kind::Color, ""),
      choice(definition(key_density, "Appearance", "Density", "Row height and spacing of tables and lists.", Kind::Enum,
                        "comfortable"),
             {"comfortable", "compact"}),
      range(definition(key_text_scale, "Appearance", "Text size", "Text size in percent of the default.", Kind::Int,
                       "100"),
            80, 200),
      definition(key_reduced_motion, "Appearance", "Reduced motion", "Turn off animations and transitions.", Kind::Bool,
                 "false"),
      definition(key_monochrome, "Appearance", "Monochrome terminal", "Draw the terminal UI without colors.",
                 Kind::Bool, "false"),
      choice(definition(key_time_zone, "Units", "Time zone", "Show timestamps in local time or in UTC.", Kind::Enum,
                        "local"),
             {"local", "utc"}),
      definition(key_sidebar_collapsed, "Window", "Sidebar collapsed", "Show the sidebar as icons only.", Kind::Bool,
                 "false"),
      range(definition(key_window_width, "Window", "Window width", "Dashboard window width in pixels.", Kind::Int,
                       "1180"),
            640, 10000),
      range(definition(key_window_height, "Window", "Window height", "Dashboard window height in pixels.", Kind::Int,
                       "760"),
            480, 10000),
      range(definition(key_refresh_seconds, "Data", "Refresh interval",
                       "Seconds between checks for changes made by another window or command.", Kind::Int, "2"),
            1, 60),
      definition(key_confirm, "Safe defaults", "Confirm destructive actions",
                 "Ask before deleting or overwriting data.", Kind::Bool, "true"),
      range(definition(key_notifier_interval, "Notifications", "Notifier interval",
                       "Seconds between checks of the background notifier.", Kind::Int, "60"),
            15, 3600),
      definition(key_notifier_titles, "Notifications", "Show titles in system notifications",
                 "Off shows only a count. On adds the notification title, never record content.", Kind::Bool, "false"),
      pinned(choice(definition(key_record_columns, "Tables", "Record table columns",
                               "Columns of the record table, separated by commas. The name column is always shown.",
                               Kind::Columns, "id,name,status,severity,updated,state"),
                    record_columns()),
             {"name"}),
      choice(definition(key_record_sort, "Tables", "Record table sort",
                        "Sort column and direction of the record table, for example updated:desc.", Kind::Sort,
                        "updated:desc"),
             {"name", "status", "severity", "updated"}),
  };
}

AppError unknown_setting(std::string_view key) {
  return AppError::invalid(key, "unknown setting", "List the settings to see the available keys");
}

std::string joined(const std::vector<std::string_view>& parts) {
  std::string out;
  for (const std::string_view part : parts) {
    if (!out.empty()) {
      out.append(", ");
    }
    out.append(part);
  }
  return out;
}

bool has(const std::vector<std::string_view>& list, std::string_view wanted) {
  return std::ranges::find(list, wanted) != list.end();
}

const Definition& require(std::string_view key) {
  const Definition* found = lookup(key);
  if (found == nullptr) {
    throw unknown_setting(key);
  }
  return *found;
}

}

std::string_view label(Kind kind) {
  switch (kind) {
    case Kind::Enum:
      return "enum";
    case Kind::Bool:
      return "bool";
    case Kind::Int:
      return "int";
    case Kind::Color:
      return "color";
    case Kind::Text:
      return "text";
    case Kind::Columns:
      return "columns";
    case Kind::Sort:
      return "sort";
  }
  return "text";
}

std::string_view label(Source source) {
  switch (source) {
    case Source::Default:
      return "default";
    case Source::Stored:
      return "stored";
    case Source::Environment:
      return "environment";
    case Source::Flag:
      return "flag";
  }
  return "default";
}

bool Value::flag() const {
  return raw == "true";
}

std::int64_t Value::number() const {
  return text::parse_integer(raw).value_or(0);
}

bool Value::locked() const {
  return source == Source::Environment || source == Source::Flag;
}

const std::vector<std::string_view>& record_columns() {
  static const std::vector<std::string_view> columns = {"id", "name", "status", "severity", "updated", "state"};
  return columns;
}

const std::vector<Definition>& definitions() {
  static const std::vector<Definition> all = build_definitions();
  return all;
}

const Definition* lookup(std::string_view key) {
  for (const Definition& candidate : definitions()) {
    if (candidate.key == key) {
      return &candidate;
    }
  }
  return nullptr;
}

std::string env_name(std::string_view key) {
  return meta::env_name(key);
}

std::vector<std::string> split_columns(std::string_view value) {
  std::vector<std::string> out;
  for (const std::string& part : text::split(value, ',')) {
    const std::string_view column = text::trim_whitespace(part);
    if (!column.empty()) {
      out.emplace_back(column);
    }
  }
  return out;
}

std::pair<std::string, bool> split_sort(std::string_view value) {
  const std::size_t colon = value.find(':');
  if (colon == std::string_view::npos) {
    return {std::string(value), false};
  }
  return {std::string(value.substr(0, colon)), value.substr(colon + 1) == "desc"};
}

std::string normalize(const Definition& definition, std::string_view raw) {
  const std::string_view value = text::trim_whitespace(raw);
  const auto fail = [&definition](std::string_view problem, std::string_view fix) {
    return AppError::invalid(definition.key, problem, fix);
  };
  switch (definition.kind) {
    case Kind::Enum: {
      for (const std::string_view option : definition.options) {
        if (text::equals_ignoring_case(option, value)) {
          return std::string(option);
        }
      }
      throw fail(text::concat("\"", raw, "\" is not one of the allowed values"),
                 text::concat("Use one of: ", joined(definition.options)));
    }
    case Kind::Bool: {
      const std::string lowered = text::lower(value);
      if (lowered == "true" || lowered == "on" || lowered == "yes" || lowered == "1") {
        return "true";
      }
      if (lowered == "false" || lowered == "off" || lowered == "no" || lowered == "0") {
        return "false";
      }
      throw fail(text::concat("\"", raw, "\" is not a yes or no value"), "Use true or false");
    }
    case Kind::Int: {
      const std::string fix = text::concat("Use a number from ", definition.min, " to ", definition.max);
      const std::string_view digits = value.starts_with('+') ? value.substr(1) : value;
      const std::optional<std::int64_t> number = digits.starts_with('+') || (digits != value && digits.starts_with('-'))
                                                     ? std::nullopt
                                                     : text::parse_integer(digits);
      if (!number) {
        throw fail(text::concat("\"", raw, "\" is not a whole number"), fix);
      }
      if (*number < definition.min || *number > definition.max) {
        throw fail(text::concat(*number, " is outside the allowed range"), fix);
      }
      return std::to_string(*number);
    }
    case Kind::Color: {
      if (value.empty()) {
        return {};
      }
      if (value.front() != '#' || !theme::Color::parse(value)) {
        throw fail(text::concat("\"", raw, "\" is not a color"),
                   "Use the form #RRGGBB, for example #1257C4, or leave it empty");
      }
      return text::upper(value);
    }
    case Kind::Columns: {
      const std::string fix = text::concat("Use some of: ", joined(definition.options));
      if (value.size() > 400) {
        throw fail("the list is too long", fix);
      }
      const std::vector<std::string> wanted = split_columns(text::lower(value));
      for (const std::string& column : wanted) {
        if (!has(definition.options, column)) {
          throw fail(text::concat("\"", column, "\" is not a column of this table"), fix);
        }
      }
      std::vector<std::string_view> ordered;
      for (const std::string_view option : definition.options) {
        if (has(definition.pinned, option) || std::ranges::find(wanted, option) != wanted.end()) {
          ordered.push_back(option);
        }
      }
      std::string out;
      for (const std::string_view column : ordered) {
        if (!out.empty()) {
          out.push_back(',');
        }
        out.append(column);
      }
      return out;
    }
    case Kind::Sort: {
      const std::string lowered = text::lower(value);
      const std::size_t colon = lowered.find(':');
      const std::string_view whole(lowered);
      const std::string_view key = text::trim_whitespace(whole.substr(0, colon));
      const std::string_view direction =
          colon == std::string::npos ? std::string_view("asc") : text::trim_whitespace(whole.substr(colon + 1));
      const std::string fix = text::concat("Use a column (", joined(definition.options),
                                           ") followed by :asc or :desc, for example ", definition.fallback);
      if (!has(definition.options, key)) {
        throw fail(text::concat("\"", key, "\" is not a sortable column"), fix);
      }
      if (direction != "asc" && direction != "desc") {
        throw fail(text::concat("\"", direction, "\" is not a direction"), fix);
      }
      return text::concat(key, ":", direction);
    }
    case Kind::Text:
      return std::string(value);
  }
  return std::string(value);
}

std::vector<std::string_view> groups(const std::vector<Value>& values) {
  std::vector<std::string_view> seen;
  for (const Value& value : values) {
    if (!has(seen, value.definition.group)) {
      seen.push_back(value.definition.group);
    }
  }
  return seen;
}

Settings::Settings(const store::Store& database, clock::Clock clock, platform::Env env, const Flags& flags)
    : database_(&database), clock_(std::move(clock)), env_(std::move(env)), flags_(&flags) {}

Value Settings::resolve(const Definition& definition, std::optional<std::string> stored) const {
  Value value;
  value.definition = definition;
  value.key = std::string(definition.key);
  value.raw = std::string(definition.fallback);
  value.stored = stored;
  if (stored) {
    try {
      value.raw = normalize(definition, *stored);
      value.source = Source::Stored;
    } catch (const AppError& error) {
      value.override_error = text::concat("stored value is invalid and the default is used: ", error.message());
    }
  }
  const std::string variable = env_name(definition.key);
  if (const std::optional<std::string> found = env_.get(variable)) {
    try {
      value.raw = normalize(definition, *found);
      value.source = Source::Environment;
      value.overridden_by = text::concat("environment variable ", variable);
    } catch (const AppError& error) {
      value.override_error = text::concat("environment variable ", variable, " is ignored: ", error.message());
    }
  }
  if (const auto flag = flags_->find(definition.key); flag != flags_->end()) {
    try {
      value.raw = normalize(definition, flag->second);
      value.source = Source::Flag;
      value.overridden_by = text::concat("command-line flag --set ", definition.key);
    } catch (const AppError& error) {
      value.override_error = text::concat("command-line flag --set ", definition.key, " is ignored: ", error.message());
    }
  }
  return value;
}

Value Settings::get(std::string_view key) const {
  const Definition& definition = require(key);
  store::Statement statement = database_->prepare(sql_select_one);
  statement.bind(key);
  std::optional<std::string> stored;
  if (statement.step()) {
    stored = statement.text(0);
  }
  return resolve(definition, std::move(stored));
}

std::vector<Value> Settings::list() const {
  std::map<std::string, std::string, std::less<>> stored;
  store::Statement statement = database_->prepare(sql_select_all);
  while (statement.step()) {
    stored.emplace(statement.text(0), statement.text(1));
  }
  std::vector<Value> values;
  for (const Definition& definition : definitions()) {
    std::optional<std::string> found;
    if (const auto entry = stored.find(definition.key); entry != stored.end()) {
      found = std::move(entry->second);
      stored.erase(entry);
    }
    values.push_back(resolve(definition, std::move(found)));
  }
  for (auto& [key, raw] : stored) {
    Value value;
    value.definition = definition("", "Unknown", "", unknown_help, Kind::Text, "");
    value.key = key;
    value.raw = raw;
    value.stored = raw;
    value.source = Source::Stored;
    value.unknown = true;
    values.push_back(std::move(value));
  }
  return values;
}

Value Settings::set(std::string_view key, std::string_view raw) const {
  const Definition& definition = require(key);
  std::string normalized = normalize(definition, raw);
  database_->prepare(sql_upsert).bind(key).bind(normalized).bind(clock_.now().to_rfc3339()).run();
  return resolve(definition, std::move(normalized));
}

Value Settings::reset(std::string_view key) const {
  const Definition& definition = require(key);
  database_->prepare(sql_delete).bind(key).run();
  return resolve(definition, std::nullopt);
}

std::map<std::string, Value, std::less<>> Settings::effective() const {
  std::map<std::string, Value, std::less<>> out;
  for (Value& value : list()) {
    std::string key = value.key;
    out.emplace(std::move(key), std::move(value));
  }
  return out;
}

std::string Settings::text(std::string_view key) const {
  return get(key).raw;
}

bool Settings::flag(std::string_view key) const {
  return get(key).flag();
}

std::int64_t Settings::number(std::string_view key) const {
  return get(key).number();
}

}
