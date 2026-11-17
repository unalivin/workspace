#include <cmath>
#include <utility>

#include <nlohmann/json.hpp>

#include "app/command.hpp"
#include "core/files.hpp"
#include "core/guide.hpp"
#include "core/meta.hpp"
#include "core/text.hpp"
#include "core/theme.hpp"
#include "core/userconfig.hpp"

namespace app::cli {

namespace {

using settings::Value;

std::string source_text(const Value& value) {
  if (value.overridden_by.empty()) {
    return std::string(settings::label(value.source));
  }
  return text::concat("overridden by ", value.overridden_by);
}

std::string_view shown(std::string_view raw) {
  return raw.empty() ? std::string_view("(empty)") : raw;
}

void run_list(Env& env, const Call& call, service::App& app) {
  call.no_arguments();
  const std::vector<Value> values = app.settings().list();
  const std::string_view format = call.text("format");
  if (format == "json") {
    nlohmann::ordered_json rows = nlohmann::ordered_json::array();
    for (const Value& value : values) {
      nlohmann::ordered_json row;
      row["key"] = value.key;
      row["value"] = value.raw;
      row["default"] = value.definition.fallback;
      row["source"] = settings::label(value.source);
      if (!value.overridden_by.empty()) {
        row["overridden_by"] = value.overridden_by;
      }
      if (!value.override_error.empty()) {
        row["problem"] = value.override_error;
      }
      rows.push_back(std::move(row));
    }
    json(env, rows);
    return;
  }
  if (format != "table") {
    throw usage(text::concat("unknown format \"", format, "\": use table or json"));
  }
  std::vector<std::vector<std::string>> rows;
  for (const Value& value : values) {
    rows.push_back({value.key, std::string(shown(value.raw)), source_text(value), std::string(value.definition.label)});
  }
  table(env, {"KEY", "VALUE", "SOURCE", "MEANING"}, rows);
  for (const Value& value : values) {
    if (!value.override_error.empty()) {
      note(env, text::concat("note: ", value.key, ": ", value.override_error));
    }
  }
}

const std::string& one_key(const Call& call) {
  if (call.args.size() != 1) {
    throw usage(text::concat("give one setting key: run \"", meta::slug, " config list\" to see them"));
  }
  return call.args.front();
}

void run_get(Env& env, const Call& call, service::App& app) {
  const Value value = app.settings().get(one_key(call));
  line(env, value.raw);
  if (!value.overridden_by.empty()) {
    note(env, text::concat("note: overridden by ", value.overridden_by, ", the stored value is \"",
                           value.stored.value_or(""), "\""));
  }
}

void precedence_note(Env& env, const Value& value, bool saved) {
  if (!value.overridden_by.empty()) {
    note(env, text::concat(saved ? "note: saved, but the" : "note: the", " ", value.overridden_by,
                           " still takes precedence with \"", value.raw, "\""));
  }
}

void run_set(Env& env, const Call& call, service::App& app) {
  if (call.args.size() != 2) {
    throw usage(text::concat("give a key and a value, for example: ", meta::slug, " config set ", settings::key_density,
                             " compact"));
  }
  const Value value = app.settings().set(call.args[0], call.args[1]);
  line(env, text::concat(value.key, " = ", shown(value.stored.value_or(""))));
  precedence_note(env, value, true);
}

void run_reset(Env& env, const Call& call, service::App& app) {
  const Value value = app.settings().reset(one_key(call));
  line(env, text::concat(value.key, " is back to its default: ", shown(value.definition.fallback)));
  precedence_note(env, value, false);
}

std::string accent_of(std::string_view name, theme::Mode mode) {
  theme::Options options;
  options.name = std::string(name);
  options.mode = mode;
  return theme::resolve(options).palette.accent.hex();
}

void run_theme_list(Env& env, const Call& call, service::App& app) {
  call.no_arguments();
  const std::string current = app.settings().text(settings::key_theme);
  std::vector<std::vector<std::string>> rows;
  for (const std::string& name : theme::names()) {
    rows.push_back({name, std::string(theme::label(name)), accent_of(name, theme::Mode::Light),
                    accent_of(name, theme::Mode::Dark), name == current ? "in use" : ""});
  }
  table(env, {"NAME", "LABEL", "LIGHT ACCENT", "DARK ACCENT", ""}, rows);
}

void run_theme_set(Env& env, const Call& call, service::App& app) {
  if (call.args.size() != 1) {
    throw usage(text::concat("give one theme name: ", text::join(theme::names(), ", ")));
  }
  const Value value = app.settings().set(settings::key_theme, call.args.front());
  const std::string stored = value.stored.value_or("");
  line(env, text::concat("Color theme: ", stored, " (", theme::label(stored), ")"));
  precedence_note(env, value, true);
}

std::string one_decimal(double value) {
  const long long tenths = std::llround(value * 10.0);
  return text::concat(tenths / 10, ".", tenths % 10);
}

void run_theme_show(Env& env, const Call& call, service::App& app) {
  call.no_arguments();
  const auto values = app.settings().effective();
  const auto raw = [&values](std::string_view key) {
    const auto found = values.find(key);
    return found == values.end() ? std::string() : found->second.raw;
  };
  const std::string mode = raw(settings::key_mode);
  const theme::Tokens tokens = app.appearance(mode == "dark");
  const std::string mode_text =
      mode == "system" ? std::string("system (the terminal cannot ask the system, light colors are listed)") : mode;
  const theme::Profile profile = theme::detect_profile(env.options.env, raw(settings::key_monochrome) == "true");
  const auto theme_value = values.find(settings::key_theme);
  const std::string theme_source = theme_value == values.end() ? std::string() : source_text(theme_value->second);
  const auto measured = [&tokens](theme::Color color) {
    return text::concat(color.hex(), " (contrast ", one_decimal(theme::contrast(color, tokens.palette.surface)),
                        ":1 on surface)");
  };
  table(env, {"ITEM", "VALUE"},
        {
            {"Theme", text::concat(tokens.name, " (", tokens.label, "), ", theme_source)},
            {"Mode", mode_text},
            {"Colors listed", tokens.dark ? "dark" : "light"},
            {"Terminal colors", std::string(theme::label(profile))},
            {"Density", raw(settings::key_density)},
            {"Text size", text::concat(raw(settings::key_text_scale), " %")},
            {"Reduced motion", raw(settings::key_reduced_motion)},
            {"Background", tokens.palette.background.hex()},
            {"Surface", tokens.palette.surface.hex()},
            {"Text", measured(tokens.palette.text)},
            {"Accent", measured(tokens.palette.accent)},
        });
  for (const std::string& warning : tokens.warnings) {
    note(env, text::concat("note: ", warning));
  }
}

std::string_view front_end_label(std::string_view mode) {
  if (mode == userconfig::front_end_gui) {
    return "gui (dashboard window)";
  }
  if (mode == userconfig::front_end_tui) {
    return "tui (terminal UI)";
  }
  return "not set, the application asks at start";
}

void run_launcher_show(Env& env, const Call& call, service::App& app) {
  call.no_arguments();
  line(env, text::concat("Open with: ", front_end_label(app.front_end())));
  const std::string variable = meta::env_name("ui");
  if (const std::optional<std::string> value = env.options.env.get(variable)) {
    note(env, text::concat("note: overridden by environment variable ", variable, "=", *value));
  }
}

void run_launcher_set(Env& env, const Call& call, service::App& app) {
  if (call.args.size() != 1 ||
      (call.args.front() != userconfig::front_end_gui && call.args.front() != userconfig::front_end_tui)) {
    throw usage("give gui or tui");
  }
  app.remember_front_end(call.args.front());
  line(env, text::concat("Open with: ", front_end_label(call.args.front())));
}

void run_launcher_reset(Env& env, const Call& call, service::App& app) {
  call.no_arguments();
  app.remember_front_end("");
  line(env, text::concat("Open with: ", front_end_label("")));
}

std::string location_source(const service::Location& location) {
  if (!location.overridden_by.empty()) {
    return location.overridden_by;
  }
  return location.source == "saved" ? "saved location" : "default location";
}

void run_init(Env& env, const Call& call, service::App& app) {
  call.no_arguments();
  const std::string path = files::shown(app.location().path);
  if (app.created_database()) {
    line(env, text::concat("Database ready at ", path));
  } else {
    line(env, text::concat("The database already exists at ", path));
  }
}

void run_path(Env& env, const Call& call) {
  call.no_arguments();
  const service::Location location = service::locate(app_options(env, call.globals, false));
  line(env, files::shown(location.path));
  note(env, text::concat("set by: ", location_source(location), ", ", location.exists ? "exists" : "not created yet"));
}

void run_move(Env& env, const Call& call, service::App& app) {
  if (call.args.size() != 1) {
    throw usage("give the new database path");
  }
  const service::MoveMode mode = call.truth("switch") ? service::MoveMode::Switch : service::MoveMode::Copy;
  service::MovePlan plan;
  try {
    plan = app.plan_move(call.args.front(), mode);
  } catch (const AppError& error) {
    throw usage(error.message());
  }
  if (call.truth("dry-run")) {
    line(env, text::concat("Dry run: ", plan.statement, " Nothing was changed."));
    return;
  }
  confirm(env, call, &app, plan.statement);
  app.move_database(plan);
  line(env, text::concat("Now using ", files::shown(app.location().path)));
}

void run_check(Env& env, const Call& call, service::App& app) {
  call.no_arguments();
  app.store().integrity_check();
  app.store().foreign_key_check();
  line(env, text::concat("Database is healthy: integrity check ok, foreign keys ok, schema version ",
                         app.store().schema_version()));
}

}

Command config_command() {
  return Command::group(
      "config", "Show and change settings",
      {
          Command::leaf("list", "List every setting with its value and where the value comes from", run_list)
              .with({text_flag("format", "table", "table or json")}),
          Command::leaf("get", "Print the value of one setting", run_get).taking("<key>"),
          Command::leaf("set", "Change a setting", run_set).taking("<key> <value>"),
          Command::leaf("reset", "Return one setting to its default", run_reset).taking("<key>"),
          Command::group("theme", "Choose the color theme",
                         {
                             Command::leaf("list", "List the color themes", run_theme_list),
                             Command::leaf("set", "Choose a color theme", run_theme_set).taking("<name>"),
                             Command::leaf("show", "Show the theme, mode and colors in use", run_theme_show),
                         }),
          Command::group("launcher", "Choose which front end opens by default",
                         {
                             Command::leaf("show", "Show the remembered front end", run_launcher_show),
                             Command::leaf("set", "Remember a front end", run_launcher_set).taking("<gui|tui>"),
                             Command::leaf("reset", "Forget the choice and ask again", run_launcher_reset),
                         }),
      });
}

Command database_command() {
  return Command::group(
      "db", "Create, locate, move and check the database",
      {
          Command::leaf("init", "Create the database if it does not exist", run_init),
          Command::standalone("path", "Print the database path and what decides it", run_path),
          Command::leaf("move", "Copy the database to a new location and use it, or switch to another database",
                        run_move)
              .taking("<path>")
              .with({
                  toggle("switch",
                         "use the database at the path as it is, or create an empty one there, instead of copying"),
                  toggle("yes", "do not ask for confirmation"),
                  toggle("dry-run", "show what would change without changing it"),
              }),
          Command::leaf("check", "Run the integrity and foreign-key checks", run_check),
      });
}

void run_diagnostics(Env& env, const Call& call, service::App& app) {
  call.no_arguments();
  const service::Diagnostics report = app.diagnostics(static_cast<bool>(env.hooks.open_gui));
  std::vector<std::vector<std::string>> rows;
  for (const auto& [label, value] : report.lines()) {
    rows.push_back({std::string(label), value});
  }
  table(env, {"ITEM", "VALUE"}, rows);
  if (report.integrity != "ok") {
    throw failure(
        "the integrity check failed: restore the database from a backup or export the records and create a "
        "new database");
  }
}

void run_guide(Env& env, const Call& call) {
  const guide::Guide usage_guide = service::usage_guide();
  if (call.args.empty()) {
    line(env, text::concat(meta::app_name, " usage guide\n\nContents:"));
    for (const guide::Section& section : usage_guide.sections) {
      line(env, text::concat(std::string(section.level * 2, ' '), section.title));
    }
    line(env, text::concat("\nRead a topic with \"", meta::slug, " guide <words>\"."));
    return;
  }
  const std::string words = text::join(call.args, " ");
  const std::vector<guide::Hit> hits = usage_guide.search(words);
  if (hits.empty()) {
    throw failure(text::concat("no section of the usage guide mentions \"", words, "\": run \"", meta::slug,
                               " guide\" for the contents"));
  }
  for (std::size_t index = 0; index < hits.size(); ++index) {
    if (index > 0) {
      line(env, "");
    }
    const guide::Section& section = usage_guide.sections.at(hits[index].section);
    line(env, text::concat(section.title, "\n", std::string(text::length(section.title), '='), "\n\n",
                           guide::plain(section.body)));
  }
}

void run_about(Env& env, const Call& call, service::App& app) {
  call.no_arguments();
  if (call.truth("issue")) {
    const service::IssueReport issue = app.diagnostics(static_cast<bool>(env.hooks.open_gui)).issue_report();
    line(env, text::concat("Found a bug? Please report it on GitHub.\n\nThe report form is prefilled with exactly "
                           "this text and nothing else:\n\n",
                           issue.body, "\nOpen this link in a browser to continue:\n", issue.url));
    return;
  }
  table(env, {"ITEM", "VALUE"},
        {
            {"Application", text::concat(meta::app_name, " ", meta::version())},
            {"About", std::string(meta::tagline)},
            {"Publisher", std::string(meta::publisher)},
            {"License", std::string(meta::license)},
            {"Source Code", service::source_link().url},
            {"Report a bug", text::concat(meta::repository_url, "/issues/new")},
            {"Report a vulnerability", service::advisory_link().url},
        });
  line(env, text::concat("\nRun \"", meta::slug,
                         " about --issue\" to preview a bug report. Nothing is opened or sent by this command."));
}

void run_licenses(Env& env, const Call& call) {
  call.no_arguments();
  line(env, text::concat(meta::license_text(), "\n\n", meta::notices_text()));
}

}
