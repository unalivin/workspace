#include "core/service.hpp"

#include <algorithm>
#include <system_error>

#include "core/error.hpp"
#include "core/files.hpp"
#include "core/text.hpp"

namespace app::service {

namespace fs = std::filesystem;

namespace {

constexpr std::string_view sql_copy_into = "VACUUM INTO ?1";
constexpr std::int64_t export_limit = 10000;

fs::path full_path(const fs::path& path) {
  std::error_code problem;
  const fs::path resolved = fs::absolute(path, problem);
  if (problem) {
    throw AppError::usage(
        text::concat("the path \"", files::shown(path), "\" cannot be resolved: ", problem.message()));
  }
  return resolved.lexically_normal();
}

bool has_content(const fs::path& path) {
  std::error_code problem;
  if (!fs::is_regular_file(path, problem) || problem) {
    return false;
  }
  const std::uintmax_t size = fs::file_size(path, problem);
  return !problem && size > 0;
}

store::Store open_store(const Options& options, const Location& location) {
  if (options.read_only && !location.exists) {
    throw AppError::storage(text::concat("the database \"", files::shown(location.path),
                                         "\" does not exist yet: start the application once to create it"));
  }
  store::Store database = store::Store::open(location.path, store::Options{options.read_only, std::nullopt});
  if (!options.read_only) {
    database.migrate(store::embedded_migrations());
  }
  return database;
}

std::string_view yes_no(bool value) {
  return value ? "yes" : "no";
}

std::string list_or_none(const std::vector<std::string>& items) {
  return items.empty() ? std::string("none") : text::join(items, ", ");
}

}

Options Options::system() {
  return {};
}

Location locate(const Options& options) {
  Location location;
  location.config_dir = options.config_dir ? *options.config_dir : platform::config_dir(options.os, options.env);
  location.config = userconfig::load(location.config_dir);
  const std::string variable = meta::env_name("db");
  fs::path path;
  if (options.database_path) {
    path = *options.database_path;
    location.source = "flag";
    location.overridden_by = "command-line flag --db";
  } else if (const std::optional<std::string> from_env = options.env.get(variable)) {
    path = files::from_utf8(*from_env);
    location.source = "environment";
    location.overridden_by = text::concat("environment variable ", variable);
  } else if (!location.config.database_path.empty()) {
    path = files::from_utf8(location.config.database_path);
    location.source = "saved";
  } else {
    path = platform::default_database_path(options.os, options.env);
    location.source = "default";
  }
  location.path = full_path(path);
  location.exists = has_content(location.path);
  return location;
}

App::App(store::Store store, Location location, Options options, bool created, std::int64_t seen)
    : store_(std::move(store)),
      location_(std::move(location)),
      options_(std::move(options)),
      created_(created),
      seen_(seen) {}

App App::open(Options options) {
  Location location = locate(options);
  return open_at(std::move(options), std::move(location));
}

App App::open_at(Options options, Location location) {
  store::Store database = open_store(options, location);
  const bool created = !location.exists;
  location.exists = true;
  const std::int64_t seen = database.data_version();
  return {std::move(database), std::move(location), std::move(options), created, seen};
}

const store::Store& App::store() const {
  return store_;
}

settings::Settings App::settings() const {
  return {store_, options_.clock, options_.env, options_.overrides};
}

records::Records App::records() const {
  return {store_, options_.clock, options_.zone};
}

notify::Notifications App::notifications() const {
  return {store_, options_.clock, options_.zone};
}

const Location& App::location() const {
  return location_;
}

const Options& App::options() const {
  return options_;
}

bool App::created_database() const {
  return created_;
}

void App::dismiss_welcome() {
  created_ = false;
}

clock::Timestamp App::now() const {
  return options_.clock.now();
}

const std::string& App::front_end() const {
  return location_.config.front_end;
}

void App::remember_front_end(std::string_view mode) {
  if (!mode.empty() && mode != userconfig::front_end_gui && mode != userconfig::front_end_tui) {
    throw AppError::usage(
        text::concat("unknown front end \"", mode, "\": use gui, tui or an empty value to ask every time"));
  }
  userconfig::Config config = location_.config;
  config.front_end = std::string(mode);
  userconfig::save(location_.config_dir, config);
  location_.config = std::move(config);
}

MovePlan App::plan_move(std::string_view target, MoveMode mode) const {
  if (!location_.overridden_by.empty()) {
    throw AppError::usage(text::concat("the database location is set by the ", location_.overridden_by,
                                       ", so the saved location would not take effect: remove the override first"));
  }
  const std::string_view wanted = text::trim_whitespace(target);
  if (wanted.empty()) {
    throw AppError::usage("the new database path is empty: enter a file path");
  }
  MovePlan plan;
  plan.to = full_path(files::from_utf8(wanted));
  plan.from = location_.path;
  plan.mode = mode;
  if (plan.to == plan.from) {
    throw AppError::usage("the database is already at this location");
  }
  if (files::is_directory(plan.to)) {
    throw AppError::usage(
        text::concat("\"", files::shown(plan.to), "\" is a directory: add a file name such as ", meta::slug, ".db"));
  }
  plan.target_exists = files::exists(plan.to);
  const std::string shown = files::shown(plan.to);
  const std::string untouched = text::concat("The file at ", files::shown(plan.from), " stays untouched.");
  if (mode == MoveMode::Copy && plan.target_exists) {
    throw AppError::conflict(text::concat("\"", shown,
                                          "\" already exists and would be overwritten: choose a new file name, or "
                                          "switch to the existing database instead"));
  }
  if (mode == MoveMode::Copy) {
    plan.statement = text::concat("Copy the current database to ", shown, " and use the copy from now on. ", untouched);
  } else if (plan.target_exists) {
    plan.statement = text::concat("Use the existing database at ", shown, " from now on. ", untouched);
  } else {
    plan.statement = text::concat("Create a new empty database at ", shown, " and use it from now on. ", untouched);
  }
  return plan;
}

void App::move_database(const MovePlan& plan) {
  if (plan.from != location_.path || plan.to.empty()) {
    throw AppError::usage("the plan does not match the open database: plan the change again");
  }
  if (plan.mode == MoveMode::Copy) {
    if (plan.to.has_parent_path()) {
      files::create_private_dir(plan.to.parent_path());
    }
    try {
      store_.prepare(sql_copy_into).bind(files::shown(plan.to)).run();
    } catch (const AppError& error) {
      throw AppError::storage(
          text::concat("cannot copy the database to \"", files::shown(plan.to), "\": ", error.message()));
    }
    files::restrict(plan.to, false);
  }
  Location target;
  target.path = plan.to;
  target.source = "saved";
  target.exists = has_content(plan.to);
  target.config_dir = location_.config_dir;
  target.config = location_.config;
  App candidate = open_at(options_, std::move(target));
  userconfig::Config config = location_.config;
  config.database_path = files::shown(plan.to);
  userconfig::save(location_.config_dir, config);
  candidate.location_.config = std::move(config);
  *this = std::move(candidate);
}

bool App::poll() {
  const std::int64_t version = store_.data_version();
  const bool changed = version != seen_;
  seen_ = version;
  return changed;
}

theme::Tokens App::appearance(bool system_dark) const {
  const auto values = settings().effective();
  const auto raw = [&values](std::string_view key) {
    const auto found = values.find(key);
    return found == values.end() ? std::string() : found->second.raw;
  };
  theme::Options options;
  options.name = raw(settings::key_theme);
  options.mode = theme::parse_mode(raw(settings::key_mode));
  options.system_dark = system_dark;
  options.accent = raw(settings::key_accent);
  options.density = raw(settings::key_density);
  const auto scale = values.find(settings::key_text_scale);
  options.text_scale = scale == values.end() ? 100 : scale->second.number();
  const auto motion = values.find(settings::key_reduced_motion);
  options.reduced_motion = motion != values.end() && motion->second.flag();
  return theme::resolve(options);
}

clock::Zone App::display_zone() const {
  try {
    if (settings().text(settings::key_time_zone) == "utc") {
      return clock::Zone::utc();
    }
  } catch (const AppError&) {
    return options_.zone;
  }
  return options_.zone;
}

std::string App::format_time(clock::Timestamp moment) const {
  return display_zone().display(moment);
}

transfer::ImportResult App::import_file(const fs::path& path, std::string_view format, bool dry_run) const {
  const transfer::Format chosen = transfer::format_for_file(files::shown(path), format);
  std::string raw;
  try {
    raw = files::read_limited(path, transfer::max_import_bytes);
  } catch (const AppError& error) {
    throw AppError::io(text::concat("cannot open the import file: ", error.message()));
  }
  return transfer::import_records(records(), raw, chosen, dry_run);
}

Exported App::export_text(transfer::Format format) const {
  records::Query query;
  query.include_disabled = true;
  query.limit = export_limit;
  const std::vector<records::Record> list = records().list(query);
  return {transfer::export_text(format, list), list.size()};
}

std::size_t App::export_file(const fs::path& path, std::string_view format, bool overwrite) const {
  if (path.empty()) {
    throw AppError::usage("give the file to write, for example records.csv");
  }
  const transfer::Format chosen = transfer::format_for_file(files::shown(path), format);
  const Exported exported = export_text(chosen);
  files::write_private(path, exported.text, overwrite);
  return exported.count;
}

Diagnostics App::diagnostics(bool dashboard) const {
  Diagnostics report;
  report.build = meta::build(dashboard);
  report.database_path = files::shown(location_.path);
  report.database_source = std::string(location_.source);
  report.config_dir = files::shown(location_.config_dir);
  report.integrity = "ok";
  report.front_end = front_end().empty() ? std::string("ask every time") : front_end();
  const auto attempt = [&report](std::string_view what, const auto& action) {
    try {
      action();
      return true;
    } catch (const AppError& error) {
      report.problems.push_back(text::concat(what, ": ", error.message()));
      return false;
    }
  };
  attempt("schema version", [&] { report.schema_version = store_.schema_version(); });
  attempt("journal mode", [&] { report.journal_mode = store_.journal_mode(); });
  attempt("foreign keys", [&] { report.foreign_keys = store_.foreign_keys_enabled(); });
  attempt("full-text search", [&] { report.full_text_search = store_.has_full_text_search(); });
  if (!attempt("integrity check", [&] { store_.integrity_check(); })) {
    report.integrity = "failed";
  }
  attempt("foreign key check", [&] { store_.foreign_key_check(); });
  attempt("record count", [&] { report.records = records().count(); });
  attempt("settings", [&] {
    for (const settings::Value& value : settings().list()) {
      if (value.unknown) {
        continue;
      }
      if (!value.overridden_by.empty()) {
        report.overridden.push_back(value.key);
      } else if (value.stored && value.raw != value.definition.fallback) {
        report.changed.push_back(value.key);
      }
      if (!value.override_error.empty()) {
        report.problems.push_back(text::concat(value.key, ": ", value.override_error));
      }
    }
  });
  std::ranges::sort(report.changed);
  std::ranges::sort(report.overridden);
  return report;
}

std::vector<std::pair<std::string_view, std::string>> Diagnostics::lines() const {
  return {
      {"Version", build.version},
      {"Build profile", build.profile},
      {"Platform", text::concat(build.os, "/", build.arch)},
      {"Compiler", build.compiler},
      {"Dashboard in this build", std::string(yes_no(build.dashboard))},
      {"Database", database_path},
      {"Database location set by", database_source},
      {"Configuration directory", config_dir},
      {"Schema version", std::to_string(schema_version)},
      {"Journal mode", journal_mode},
      {"Foreign keys", std::string(yes_no(foreign_keys))},
      {"Full-text search", std::string(yes_no(full_text_search))},
      {"Integrity check", integrity},
      {"Records", std::to_string(records)},
      {"Remembered front end", front_end},
      {"Settings changed from default", list_or_none(changed)},
      {"Settings overridden", list_or_none(overridden)},
      {"Problems", list_or_none(problems)},
  };
}

IssueReport Diagnostics::issue_report() const {
  IssueReport report;
  report.title = "Bug: ";
  report.body = text::concat(
      "### What happened\n\n\n### What you expected\n\n\n### Steps to reproduce\n\n\n### Environment\n\n- ",
      meta::app_name, " ", build.version, "\n- Operating system: ", build.os, "\n- Architecture: ", build.arch,
      "\n- Build: ", build.profile, ", dashboard ", yes_no(build.dashboard), "\n- Schema version: ", schema_version,
      ", integrity check: ", integrity, "\n- Settings changed from default: ", list_or_none(changed),
      "\n- Settings overridden: ", list_or_none(overridden), "\n");
  report.url = meta::new_issue_url(report.title, report.body);
  return report;
}

guide::Guide usage_guide() {
  return guide::parse(meta::usage_text());
}

Link external_link(std::string_view label, std::string_view url) {
  Link link;
  link.label = std::string(label);
  link.url = std::string(url);
  link.allowed = meta::is_repository_url(url);
  if (!link.allowed) {
    link.reason = text::concat("Only links under ", meta::repository_url, " can be opened from the application.");
  }
  return link;
}

Link source_link() {
  return external_link("Source Code", meta::source_url());
}

Link advisory_link() {
  return external_link("Report a vulnerability privately", meta::advisory_url());
}

const std::vector<Capability>& capabilities() {
  static const std::vector<Capability> all = {
      {"database.create", "Create the database on first start", "First-start dialog", "First-start screen", "db init"},
      {"database.location", "Choose the database location", "Settings, Data, Database location",
       "Settings, Database location", "db move"},
      {"database.check", "Check database health", "About, Diagnostics", "About, Diagnostics", "db check"},
      {"overview.show", "See what needs attention", "Overview", "Overview", "overview"},
      {"record.list", "List, search, filter and sort records", "Records", "Records", "records list"},
      {"record.columns", "Choose the columns of the record table", "Records, Columns", "Records, c", "config set"},
      {"record.show", "Show one record", "Records, row detail panel", "Records, Enter", "records show"},
      {"record.add", "Add a record", "Records, Add record", "Records, n", "records add"},
      {"record.edit", "Edit a record", "Records, row detail panel, Edit", "Records, e", "records edit"},
      {"record.enable", "Enable a record", "Records, row detail panel, Enable", "Records, x", "records enable"},
      {"record.disable", "Disable a record", "Records, row detail panel, Disable", "Records, x", "records disable"},
      {"record.remove", "Remove a record", "Records, row detail panel, Remove", "Records, d", "records remove"},
      {"record.import", "Import records from CSV or JSON", "Records, Import", "Records, i", "records import"},
      {"record.export", "Export records to CSV or JSON", "Records, Export", "Records, o", "records export"},
      {"demo.load", "Load labeled demo data", "Overview, Load demo data", "Overview, l", "demo load"},
      {"demo.remove", "Remove demo data", "Settings, Data, Remove demo data", "Settings, Remove demo data",
       "demo remove"},
      {"notification.list", "List and filter notifications", "Notifications", "Notifications", "notifications list"},
      {"notification.read", "Mark notifications as read", "Notifications, Mark as read", "Notifications, m and M",
       "notifications read"},
      {"notification.rule.list", "See the rule of every event type", "Notifications, Rules", "Notifications, Rules",
       "notifications rules list"},
      {"notification.rule.set", "Change a notification rule", "Notifications, Rules", "Notifications, Rules, e",
       "notifications rules set"},
      {"notification.rule.reset", "Reset a notification rule", "Notifications, Rules, Reset", "Notifications, Rules, u",
       "notifications rules reset"},
      {"notifier.install", "Start the notifier at sign-in", "Settings, Notifications, Start at sign-in",
       "Settings, Notifier at sign-in", "notifier install"},
      {"notifier.uninstall", "Stop starting the notifier at sign-in", "Settings, Notifications, Start at sign-in",
       "Settings, Notifier at sign-in", "notifier uninstall"},
      {"notifier.status", "See whether the notifier starts at sign-in", "Settings, Notifications",
       "Settings, Notifier at sign-in", "notifier status"},
      {"notifier.run", "Run the background notifier", not_applicable, not_applicable, "notifier run"},
      {"setting.list", "See every setting and what overrides it", "Settings", "Settings", "config list"},
      {"setting.set", "Change a setting", "Settings", "Settings, Enter", "config set"},
      {"setting.reset", "Reset one setting", "Settings, Reset", "Settings, u", "config reset"},
      {"theme.list", "List color themes", "Settings, Appearance, Color theme", "Settings, Color theme",
       "config theme list"},
      {"theme.set", "Choose a color theme", "Settings, Appearance, Color theme", "Settings, Color theme",
       "config theme set"},
      {"theme.show", "See the theme in use", "Settings, Appearance", "Settings, Color theme", "config theme show"},
      {"launcher.show", "See the remembered front end", "Settings, Launcher", "Settings, Open with",
       "config launcher show"},
      {"launcher.set", "Choose the front end to open by default", "Settings, Launcher", "Settings, Open with",
       "config launcher set"},
      {"launcher.reset", "Ask again which front end to open", "Settings, Launcher", "Settings, Open with",
       "config launcher reset"},
      {"launcher.switch", "Reopen in the other front end", "Menu, Open terminal UI", "Menu, Open dashboard", "--ui"},
      {"guide.read", "Read and search the usage guide offline", "About, Usage guide", "About, Usage guide", "guide"},
      {"about.diagnostics", "Show diagnostics", "About, Diagnostics", "About, Diagnostics", "diagnostics"},
      {"about.licenses", "Show licenses and notices", "About, Licenses and notices", "About, Licenses", "licenses"},
      {"about.links", "Show the source and bug report links", "About, Overview", "About, Overview", "about"},
  };
  return all;
}

std::string capability_table() {
  std::string table = "| Capability | Dashboard | Terminal UI | CLI |\n|---|---|---|---|\n";
  for (const Capability& capability : capabilities()) {
    const std::string cli =
        capability.cli == not_applicable ? std::string(capability.cli) : text::concat("`", capability.cli, "`");
    table.append(text::concat("| ", capability.name, " | ", capability.dashboard, " | ", capability.terminal, " | ",
                              cli, " |\n"));
  }
  return table;
}

}
