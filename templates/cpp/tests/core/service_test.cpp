#include <doctest/doctest.h>

#include <algorithm>
#include <filesystem>
#include <set>
#include <string>
#include <utility>
#include <vector>

#include "core/error.hpp"
#include "core/files.hpp"
#include "core/meta.hpp"
#include "core/service.hpp"
#include "core/settings.hpp"
#include "core/text.hpp"
#include "core/userconfig.hpp"
#include "support/testkit.hpp"

using app::AppError;
using app::records::Input;
using app::service::App;
using app::service::MoveMode;
using app::service::Options;
namespace files = app::files;
namespace fs = std::filesystem;
namespace meta = app::meta;
namespace service = app::service;
namespace settings = app::settings;
namespace text = app::text;
namespace userconfig = app::userconfig;

namespace {

Options options(const testkit::TempDir& directory, std::vector<std::pair<std::string, std::string>> pairs = {}) {
  pairs.insert(pairs.begin(), {"HOME", files::shown(directory.path() / "home")});
  Options out;
  out.os = app::platform::Os::Linux;
  out.env = app::platform::Env::from_pairs(std::move(pairs));
  out.config_dir = directory.path() / "config";
  out.clock = app::clock::Clock::fixed(testkit::fixed_time());
  out.zone = app::clock::Zone::fixed(120);
  return out;
}

Input named(std::string name) {
  Input input;
  input.name = std::move(name);
  return input;
}

}

TEST_CASE("the database location follows flag environment saved then default") {
  const testkit::TempDir directory("service-locate");
  const Options base = options(directory);
  const service::Location fallback = service::locate(base);
  CHECK(fallback.source == "default");
  CHECK(fallback.path == directory.path() / "home" / ".local" / "share" / files::from_utf8(meta::slug) /
                             files::from_utf8(text::concat(meta::slug, ".db")));
  CHECK_FALSE(fallback.exists);
  CHECK(fallback.overridden_by.empty());
  const fs::path saved_path = directory.path() / "saved.db";
  userconfig::save(*base.config_dir, userconfig::Config{files::shown(saved_path), ""});
  CHECK(service::locate(base).source == "saved");
  CHECK(service::locate(base).path == saved_path);
  const std::string variable = meta::env_name("db");
  const fs::path env_path = directory.path() / "env.db";
  Options with_env = options(directory, {{variable, files::shown(env_path)}});
  service::Location found = service::locate(with_env);
  CHECK(found.source == "environment");
  CHECK(found.path == env_path);
  CHECK(text::contains(found.overridden_by, variable));
  const fs::path flag_path = directory.path() / "flag.db";
  with_env.database_path = directory.path() / "sub" / ".." / "flag.db";
  found = service::locate(with_env);
  CHECK(found.source == "flag");
  CHECK(found.path == flag_path);
}

TEST_CASE("opening creates the database once and read only needs an existing one") {
  const testkit::TempDir directory("service-open");
  const Options base = options(directory);
  Options read_only = base;
  read_only.read_only = true;
  const std::string error = testkit::failure_of([&] { static_cast<void>(App::open(read_only)); });
  CHECK_MESSAGE(text::contains(error, "start the application once to create it"), error);
  {
    const App first = App::open(base);
    CHECK(first.created_database());
    first.records().create(named("kept"));
  }
  App second = App::open(base);
  CHECK_FALSE(second.created_database());
  CHECK(second.records().count() == 1);
  const App reader = App::open(read_only);
  CHECK(reader.records().count() == 1);
  CHECK_THROWS_AS(reader.records().create(named("refused")), AppError);
  CHECK(second.now() == testkit::fixed_time());
  second.dismiss_welcome();
  CHECK_FALSE(second.created_database());
}

TEST_CASE("the front end choice is remembered in the configuration file") {
  const testkit::TempDir directory("service-frontend");
  {
    App app = App::open(options(directory));
    CHECK(app.front_end().empty());
    app.remember_front_end("tui");
    CHECK(app.front_end() == "tui");
    CHECK_THROWS_AS(app.remember_front_end("web"), AppError);
  }
  App reopened = App::open(options(directory));
  CHECK(reopened.front_end() == "tui");
  CHECK(reopened.diagnostics(false).front_end == "tui");
  reopened.remember_front_end("");
  CHECK(reopened.diagnostics(false).front_end == "ask every time");
}

TEST_CASE("moving the database copies or switches and never overwrites") {
  const testkit::TempDir directory("service-move");
  const fs::path copy_target = directory.path() / "moved" / "copy.db";
  const fs::path fresh_target = directory.path() / "fresh.db";
  fs::path original;
  {
    App app = App::open(options(directory));
    app.records().create(named("travels"));
    original = app.location().path;
    const service::MovePlan plan = app.plan_move(files::shown(copy_target), MoveMode::Copy);
    CHECK_MESSAGE(plan.statement.starts_with("Copy the current database to"), plan.statement);
    CHECK(plan.statement.ends_with(text::concat("The file at ", files::shown(original), " stays untouched.")));
    CHECK_FALSE(files::exists(copy_target));
    app.move_database(plan);
    CHECK(app.location().path == copy_target);
    CHECK(app.location().source == "saved");
    CHECK(app.records().count() == 1);
    CHECK(files::exists(original));
    if (app::platform::current_os() != app::platform::Os::Windows) {
      CHECK(files::permissions(copy_target) == 0600U);
    }
    const std::string stale = testkit::failure_of([&] { app.move_database(plan); });
    CHECK_MESSAGE(text::contains(stale, "plan the change again"), stale);
    const std::string refused =
        testkit::failure_of([&] { static_cast<void>(app.plan_move(files::shown(original), MoveMode::Copy)); });
    CHECK_MESSAGE(text::contains(refused, "would be overwritten"), refused);
    const service::MovePlan back = app.plan_move(files::shown(original), MoveMode::Switch);
    CHECK_MESSAGE(back.statement.starts_with("Use the existing database"), back.statement);
    const service::MovePlan fresh =
        app.plan_move(text::concat("  ", files::shown(fresh_target), " "), MoveMode::Switch);
    CHECK_MESSAGE(fresh.statement.starts_with("Create a new empty database"), fresh.statement);
    app.move_database(fresh);
    CHECK(app.records().count() == 0);
    CHECK_THROWS_AS(static_cast<void>(app.plan_move("", MoveMode::Copy)), AppError);
    CHECK(text::contains(
        testkit::failure_of([&] { static_cast<void>(app.plan_move(files::shown(fresh_target), MoveMode::Switch)); }),
        "already at this location"));
    CHECK(text::contains(testkit::failure_of([&] {
                           static_cast<void>(app.plan_move(files::shown(directory.path()), MoveMode::Switch));
                         }),
                         "is a directory"));
  }
  const App reopened = App::open(options(directory));
  CHECK(reopened.location().path == fresh_target);
  const App pinned = App::open(options(directory, {{meta::env_name("db"), files::shown(original)}}));
  const std::string error =
      testkit::failure_of([&] { static_cast<void>(pinned.plan_move(files::shown(copy_target), MoveMode::Switch)); });
  CHECK_MESSAGE(text::contains(error, "remove the override first"), error);
}

TEST_CASE("changes made by another connection are noticed once") {
  const testkit::TempDir directory("service-poll");
  App app = App::open(options(directory));
  const App other = App::open(options(directory));
  static_cast<void>(app.poll());
  CHECK_FALSE(app.poll());
  other.records().create(named("from elsewhere"));
  CHECK(app.poll());
  CHECK_FALSE(app.poll());
  CHECK(app.records().count() == 1);
}

TEST_CASE("appearance and time format follow the settings") {
  const testkit::TempDir directory("service-appearance");
  const App app = App::open(options(directory));
  CHECK(app.appearance(true).name == "slate");
  CHECK(app.appearance(true).dark);
  app.settings().set(settings::key_theme, "green");
  app.settings().set(settings::key_mode, "light");
  app.settings().set(settings::key_text_scale, "150");
  app.settings().set(settings::key_reduced_motion, "true");
  const app::theme::Tokens tokens = app.appearance(true);
  CHECK(tokens.name == "green");
  CHECK_FALSE(tokens.dark);
  CHECK_FALSE(tokens.motion);
  CHECK(tokens.type_scale.body > 20.9F);
  CHECK(app.format_time(testkit::fixed_time()) == "2026-03-02 11:30");
  app.settings().set(settings::key_time_zone, "utc");
  CHECK(app.format_time(testkit::fixed_time()) == "2026-03-02 09:30 UTC");
  CHECK(app.display_zone().is_utc());
}

TEST_CASE("files are exported privately and imported back") {
  const testkit::TempDir directory("service-transfer");
  const App app = App::open(options(directory));
  app.records().create(named("exported"));
  const fs::path target = directory.path() / "out" / "records.csv";
  CHECK(app.export_file(target, "", false) == 1);
  CHECK(text::contains(testkit::failure_of([&] { app.export_file(target, "", false); }), "already exists"));
  CHECK(app.export_file(target, "", true) == 1);
  CHECK_THROWS_AS(app.export_file({}, "", false), AppError);
  CHECK(text::contains(testkit::failure_of([&] { app.export_file(directory.path() / "records.txt", "", false); }),
                       "--format"));
  const fs::path json = directory.path() / "records.data";
  CHECK(app.export_file(json, "json", false) == 1);
  const app::transfer::ImportResult result = app.import_file(json, "json", false);
  CHECK(result.created == 0);
  CHECK(result.skipped == 1);
  CHECK(text::contains(
      testkit::failure_of([&] { static_cast<void>(app.import_file(directory.path() / "missing.csv", "", false)); }),
      "cannot open the import file"));
  CHECK(app.export_text(app::transfer::Format::Csv).count == 1);
}

TEST_CASE("diagnostics report the build the database and changed settings") {
  const testkit::TempDir directory("service-diagnostics");
  const App app = App::open(options(directory, {{settings::env_name(settings::key_density), "compact"}}));
  app.settings().set(settings::key_theme, "amber");
  const service::Diagnostics report = app.diagnostics(true);
  CHECK(report.schema_version == 1);
  CHECK(report.integrity == "ok");
  CHECK(report.records == 0);
  CHECK(report.foreign_keys);
  CHECK(report.full_text_search);
  CHECK(report.build.dashboard);
  CHECK(report.changed == std::vector<std::string>{std::string(settings::key_theme)});
  CHECK(report.overridden == std::vector<std::string>{std::string(settings::key_density)});
  CHECK_MESSAGE(report.problems.empty(), text::join(report.problems, " | "));
  const auto lines = report.lines();
  CHECK(lines.front().first == "Version");
  CHECK(lines.front().second == meta::version());
  CHECK(std::ranges::any_of(lines, [](const auto& line) { return line.first == "Problems" && line.second == "none"; }));
  const service::IssueReport issue = report.issue_report();
  CHECK(meta::is_repository_url(issue.url));
  CHECK(text::contains(issue.body, "Settings changed from default: appearance.theme"));
  CHECK_FALSE(text::contains(issue.body, report.database_path));
  const App broken = App::open(options(directory, {{settings::env_name(settings::key_theme), "neon"}}));
  const service::Diagnostics noisy = broken.diagnostics(false);
  REQUIRE(noisy.problems.size() == 1);
  CHECK(noisy.problems.front().starts_with("appearance.theme: environment variable"));
}

TEST_CASE("navigation shortcuts links and capabilities are complete") {
  CHECK(service::navigation.size() == 5);
  for (const service::Shortcut& shortcut : service::shortcuts) {
    CHECK_FALSE(shortcut.keys.empty());
    CHECK_FALSE(shortcut.terminal.empty());
    CHECK_FALSE(shortcut.action.empty());
  }
  CHECK(service::source_link().allowed);
  CHECK(service::advisory_link().allowed);
  const service::Link outside = service::external_link("Elsewhere", "https://example.test/");
  CHECK_FALSE(outside.allowed);
  CHECK(text::contains(outside.reason, meta::repository_url));
  std::set<std::string_view> ids;
  for (const service::Capability& capability : service::capabilities()) {
    ids.insert(capability.id);
    CHECK_MESSAGE((!capability.cli.empty() && capability.cli != service::not_applicable), capability.id);
    CHECK_MESSAGE((!capability.dashboard.empty() && !capability.terminal.empty()), capability.id);
  }
  CHECK(ids.size() == service::capabilities().size());
  CHECK(service::capabilities().size() == 39);
  CHECK(text::lines(service::capability_table()).size() == service::capabilities().size() + 2);
  CHECK_FALSE(service::usage_guide().sections.empty());
}
