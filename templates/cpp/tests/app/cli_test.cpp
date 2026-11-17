#include <doctest/doctest.h>

#include <algorithm>
#include <filesystem>
#include <string>
#include <vector>

#include <nlohmann/json.hpp>

#include "app/cli.hpp"
#include "app/command.hpp"
#include "core/files.hpp"
#include "core/meta.hpp"
#include "core/service.hpp"
#include "core/settings.hpp"
#include "core/text.hpp"
#include "support/harness.hpp"

using app::cli::exit_failure;
using app::cli::exit_ok;
using app::cli::exit_usage;
using nlohmann::json;
using testkit::Harness;
using testkit::Outcome;
namespace cli = app::cli;
namespace files = app::files;
namespace fs = std::filesystem;
namespace meta = app::meta;
namespace service = app::service;
namespace text = app::text;

namespace {

using Words = std::vector<std::string>;

bool has(std::string_view whole, std::string_view part) {
  return text::contains(whole, part);
}

}

TEST_CASE("help version and unknown commands") {
  Harness harness("cli-help");
  const std::string help = harness.ok({"--help"});
  for (const std::string& expected :
       Words{"Usage:", "Commands:", "records", "Global flags:", "--db <value>", "Environment:", meta::env_name("ui")}) {
    CHECK_MESSAGE(has(help, expected), expected, "\n", help);
  }
  CHECK(harness.ok({"help"}) == help);
  CHECK(harness.ok({"-h"}) == help);
  const std::string version = text::concat(meta::slug, " ", meta::version(), "\n");
  CHECK(harness.ok({"--version"}) == version);
  CHECK(harness.ok({"version"}) == version);
  const std::string records = harness.ok({"help", "records"});
  CHECK_MESSAGE(has(records, text::concat(meta::slug, " records <command>")), records);
  CHECK(has(records, "import"));
  const std::string list = harness.ok({"records", "list", "--help"});
  CHECK_MESSAGE((has(list, "--limit <value>") && has(list, "(default 500)") && has(list, "[flags]")), list);
  CHECK_MESSAGE(!files::exists(harness.home.path() / ".config"), "help must not create files");
  harness.fails({"recrods"}, exit_usage, "unknown command \"recrods\"");
  harness.fails({"rec"}, exit_usage, "Did you mean \"records\"?");
  harness.fails({"records", "lst"}, exit_usage, "unknown command \"records lst\"");
  harness.fails({"zzz"}, exit_usage, "Available: overview, records");
  harness.fails({"--frobnicate"}, exit_usage, "unknown flag --frobnicate");
  harness.fails({"--set", "nokey", "overview"}, exit_usage, "key=value");
  harness.fails({"--set", "no.such=1", "overview"}, exit_usage, "unknown setting \"no.such\"");
  harness.fails({"records", "list", "--frobnicate"}, exit_usage, "help records list");
  harness.fails({"records", "list", "--limit", "many"}, exit_usage, "whole number");
  const Outcome bare = harness.run({"records"});
  CHECK(bare.code == exit_usage);
  CHECK(bare.output.empty());
  CHECK_MESSAGE(has(bare.errors, "Commands:"), bare.errors);
}

TEST_CASE("records are added listed changed and removed") {
  Harness harness("cli-records");
  const Outcome added = harness.run({"records", "add", "--name", "Renew the lease", "--severity", "high"});
  CHECK_MESSAGE(added.code == exit_ok, added.errors);
  CHECK(added.output == "Added record 1: Renew the lease\n");
  CHECK_MESSAGE(has(added.errors, "Created a new database at"), added.errors);
  harness.ok({"records", "add", "--name=Archive", "--status", "done", "--disabled", "--note", "kept\nfor later"});
  const std::string listed = harness.ok({"records", "list"});
  CHECK_MESSAGE(listed.starts_with("ID  NAME"), listed);
  CHECK_MESSAGE((has(listed, "Renew the lease") && !has(listed, "Archive")), listed);
  const std::string all = harness.ok({"records", "list", "--all", "--sort", "name", "--desc"});
  CHECK_MESSAGE(all.find("Renew the lease") < all.find("Archive"), all);
  const json rows = json::parse(harness.ok({"records", "list", "--all", "--format", "json"}));
  REQUIRE(rows.size() == 2);
  CHECK(rows[0]["name"] == "Archive");
  CHECK(rows[0]["enabled"] == false);
  CHECK(rows[1]["created_at"] == "2026-03-02T09:30:00Z");
  const std::string csv = harness.ok({"records", "list", "--format", "csv"});
  CHECK_MESSAGE(csv.starts_with("name,status,severity,note,enabled"), csv);
  const std::string shown = harness.ok({"records", "show", "2"});
  CHECK_MESSAGE((has(shown, "kept / for later") && has(shown, "Enabled   no")), shown);
  const json one = json::parse(harness.ok({"records", "show", "1", "--format", "json"}));
  CHECK(one["severity"] == "high");
  CHECK(one["revision"] == 1);
  CHECK(harness.ok({"records", "edit", "1", "--status", "in_progress"}) == "Updated record 1 (revision 2)\n");
  CHECK(harness.ok({"records", "disable", "1"}) == "Record 1 is disabled\n");
  CHECK(harness.ok({"records", "enable", "1"}) == "Record 1 is enabled\n");
  CHECK(has(harness.ok({"records", "list", "--search", "lease", "--status", "in_progress"}), "Renew the lease"));
  CHECK(has(harness.ok({"records", "list", "--search", "nothing"}), "No records match"));
  harness.fails({"records", "add"}, exit_usage, "a name is required");
  harness.fails({"records", "add", "--name", ""}, exit_usage, "name");
  harness.fails({"records", "add", "--name", "x", "--severity", "urgent"}, exit_usage, "severity");
  harness.fails({"records", "add", "--name", "Renew the lease"}, exit_usage, "already exists");
  harness.fails({"records", "edit", "1"}, exit_usage, "nothing to change");
  harness.fails({"records", "show"}, exit_usage, "records list");
  harness.fails({"records", "show", "abc"}, exit_usage, "is not a record identifier");
  harness.fails({"records", "show", "99"}, exit_failure, "was not found");
  harness.fails({"records", "list", "--format", "xml"}, exit_usage, "use table, json or csv");
  harness.fails({"records", "list", "--sort", "color"}, exit_usage, "sort");
  harness.fails({"records", "list", "extra"}, exit_usage, "unexpected argument \"extra\"");
  harness.fails({"records", "remove", "1"}, exit_usage, "--yes");
  CHECK(has(harness.ok({"records", "remove", "1", "--dry-run"}), "Nothing was changed"));
  CHECK(harness.ok({"records", "remove", "--yes", "1"}) == "Removed record 1\n");
  harness.fails({"records", "show", "1"}, exit_failure, "was not found");
}

TEST_CASE("removal asks on an interactive terminal unless confirmation is off") {
  Harness harness("cli-confirm");
  harness.ok({"records", "add", "--name", "First"});
  harness.ok({"records", "add", "--name", "Second"});
  harness.terminal = true;
  harness.standard_input = "no\n";
  const std::string errors = harness.fails({"records", "remove", "1"}, exit_failure, "cancelled, nothing was changed");
  CHECK_MESSAGE(has(errors, "Type yes to continue"), errors);
  harness.standard_input = "YES\n";
  CHECK(harness.ok({"records", "remove", "1"}) == "Removed record 1\n");
  harness.set("CI", "true");
  harness.fails({"records", "remove", "2"}, exit_usage, "--yes");
  harness.terminal = false;
  CHECK(harness.ok({"--set", "safety.confirm_destructive=false", "records", "remove", "2"}) == "Removed record 2\n");
}

TEST_CASE("records are imported and exported") {
  Harness harness("cli-transfer");
  const std::string source = harness.path("in.csv");
  files::write_private(
      files::from_utf8(source),
      "name,status,severity,note,enabled\nAlpha,open,high,first,true\nBeta,done,low,,false\n,open,low,,true\n", true);
  const Outcome dry = harness.run({"records", "import", source, "--dry-run"});
  CHECK_MESSAGE(dry.code == exit_ok, dry.errors);
  CHECK_MESSAGE(dry.output.starts_with("Dry run: would have created 2"), dry.output);
  CHECK_MESSAGE(has(dry.errors, "rejected row 4"), dry.errors);
  CHECK(has(harness.ok({"records", "list", "--all"}), "No records yet"));
  CHECK(harness.ok({"records", "import", source}).starts_with("Created 2, updated 0, unchanged 0, rejected 1 row"));
  CHECK(harness.ok({"records", "import", source}).starts_with("Created 0, updated 0, unchanged 2"));
  const std::string exported = harness.ok({"records", "export"});
  CHECK_MESSAGE((has(exported, "Alpha,open,high,first,true") && has(exported, "Beta,done,low,,false")), exported);
  CHECK(json::parse(harness.ok({"records", "export", "--format", "json"})).size() == 2);
  const std::string target = harness.path("out.json");
  CHECK(harness.ok({"records", "export", "--output", target}) == text::concat("Exported 2 records to ", target, "\n"));
  CHECK(text::trim_whitespace(files::read_all(files::from_utf8(target))).starts_with('['));
  harness.fails({"records", "export", "--output", target}, exit_usage, "--force");
  harness.ok({"records", "export", "--output", target, "--force", "--format", "csv"});
  CHECK(files::read_all(files::from_utf8(target)).starts_with("name,"));
  harness.fails({"records", "export", "--format", "xml"}, exit_usage, "use csv or json");
  harness.standard_input =
      "[{\"name\":\"Gamma\",\"status\":\"open\",\"severity\":\"info\",\"note\":\"\",\"enabled\":true}]";
  CHECK(harness.ok({"records", "import", "-", "--format", "json"}).starts_with("Created 1"));
  harness.fails({"records", "import", "-"}, exit_usage, "--format csv or --format json");
  harness.fails({"records", "import"}, exit_usage, "give one file to import");
  harness.fails({"records", "import", harness.path("missing.csv")}, exit_failure, "cannot open the import file");
  const std::string broken = harness.path("broken.csv");
  files::write_private(files::from_utf8(broken), "name,status,severity,note,enabled\n,open,low,,true\n", true);
  harness.fails({"records", "import", broken}, exit_failure, "no row could be imported");
}

TEST_CASE("demo data feeds the overview and the notifications") {
  Harness harness("cli-demo");
  CHECK(has(harness.ok({"overview"}), "No records yet"));
  const std::string added = harness.ok({"demo", "load"});
  CHECK_MESSAGE((added.starts_with("Added ") && has(added, "Demo:")), added);
  CHECK(harness.ok({"demo", "load"}) == "Demo data is already loaded.\n");
  const std::string overview = harness.ok({"overview"});
  for (const std::string_view expected :
       {"MEASURE", "Open records", "Unread notifications", "SEVERITY", "Critical", "Demo data is loaded"}) {
    CHECK_MESSAGE(has(overview, expected), expected, "\n", overview);
  }
  const json summary = json::parse(harness.ok({"overview", "--format", "json"}));
  const std::int64_t demo = summary["demo_records"].get<std::int64_t>();
  CHECK(demo > 0);
  CHECK_FALSE(summary["breakdown"].empty());
  harness.fails({"overview", "--format", "csv"}, exit_usage, "use table or json");
  const std::int64_t unread = summary["unread_notifications"].get<std::int64_t>();
  CHECK(unread > 0);
  const std::string listed = harness.ok({"notifications", "list"});
  CHECK_MESSAGE((listed.starts_with("ID  WHEN") && has(listed, "unread") && has(listed, "record ")), listed);
  const json rows = json::parse(harness.ok({"notifications", "list", "--unread", "--format", "json"}));
  CHECK(static_cast<std::int64_t>(rows.size()) == unread);
  const std::string first = std::to_string(rows[0]["id"].get<std::int64_t>());
  CHECK(harness.ok({"notifications", "read", first}) == text::concat("Marked notification ", first, " as read\n"));
  CHECK(harness.ok({"notifications", "read", "all"}) ==
        text::concat("Marked ", unread - 1, " notifications as read\n"));
  CHECK(has(harness.ok({"notifications", "list", "--unread"}), "No notifications"));
  CHECK(has(harness.ok({"notifications", "list", "--severity", "critical", "--event", "record.urgent"}), "read"));
  harness.fails({"notifications", "read"}, exit_usage, "the word all");
  harness.fails({"notifications", "read", "x"}, exit_usage, "is not a notification identifier");
  harness.fails({"notifications", "read", "9999"}, exit_failure, "was not found");
  harness.fails({"notifications", "list", "--severity", "loud"}, exit_usage, "severity");
  harness.fails({"notifications", "list", "--event", "nope"}, exit_usage, "unknown event type");
  harness.fails({"demo", "remove"}, exit_usage, "--yes");
  CHECK(has(harness.ok({"demo", "remove", "--dry-run"}), text::concat("This removes ", demo, " demo records")));
  CHECK(harness.ok({"demo", "remove", "--yes"}) == text::concat("Removed ", demo, " demo records\n"));
  CHECK(harness.ok({"demo", "remove", "--yes"}) == "There is no demo data to remove.\n");
}

TEST_CASE("notification rules are listed changed and reset") {
  Harness harness("cli-rules");
  std::string listed = harness.ok({"notifications", "rules", "list"});
  CHECK_MESSAGE((has(listed, "record.urgent") && has(listed, "default") && has(listed, "none")), listed);
  const std::string changed =
      harness.ok({"notifications", "rules", "set", "record.urgent", "--enabled", "no", "--min-severity", "HIGH",
                  "--quiet-start", "22:00", "--quiet-end", "07:00", "--dedup-minutes", "5"});
  CHECK(changed ==
        "Rule for record.urgent: enabled no, minimum severity High, quiet hours 22:00 to 07:00, "
        "de-duplication 5 minutes\n");
  listed = harness.ok({"notifications", "rules", "list"});
  CHECK_MESSAGE((has(listed, "22:00 to 07:00") && has(listed, "changed")), listed);
  const Words base = {"notifications", "rules", "set", "record.urgent"};
  const auto with = [&base](Words extra) {
    Words all = base;
    all.insert(all.end(), extra.begin(), extra.end());
    return all;
  };
  harness.fails(base, exit_usage, "nothing to change");
  harness.fails(with({"--enabled", "maybe"}), exit_usage, "true or false");
  harness.fails(with({"--quiet-start", "25:00"}), exit_usage, "time of day");
  harness.fails(with({"--dedup-minutes", "x"}), exit_usage, "whole number");
  harness.fails(with({"--min-severity", "loud"}), exit_usage, "info, low");
  harness.fails({"notifications", "rules", "set", "nope", "--enabled", "true"}, exit_usage, "unknown event type");
  harness.fails({"notifications", "rules", "set"}, exit_usage, "give one event type: record.urgent");
  CHECK(harness.ok({"notifications", "rules", "reset", "record.urgent"}) ==
        "Rule for record.urgent is back to its defaults\n");
  CHECK_FALSE(has(harness.ok({"notifications", "rules", "list"}), "changed"));
}

TEST_CASE("the notifier delivers new items and manages the sign-in entry") {
  Harness harness("cli-notifier");
  CHECK(harness.ok({"notifier", "run", "--once"}) == "Checked once: 0 new notifications shown\n");
  harness.ok({"records", "add", "--name", "Server down", "--severity", "critical"});
  CHECK(harness.ok({"notifier", "run", "--once"}) == "Checked once: 1 new notifications shown\n");
  CHECK(harness.delivered == Words{text::concat(meta::app_name, " | 1 new notification")});
  harness.ok({"config", "set", "notifier.show_titles", "true"});
  harness.ok({"records", "add", "--name", "Disk full", "--severity", "critical"});
  const Outcome loop = harness.run({"notifier", "run"});
  CHECK_MESSAGE(loop.code == exit_ok, loop.errors);
  CHECK(loop.output.empty());
  CHECK_MESSAGE((has(loop.errors, "checking every 60 seconds") && has(loop.errors, "makes no network requests")),
                loop.errors);
  REQUIRE(harness.delivered.size() == 2);
  CHECK(harness.delivered[1].ends_with("A record needs attention"));
  const std::string status = harness.ok({"notifier", "status"});
  CHECK_MESSAGE((status.starts_with("Start at sign-in: off") && has(status, "not verified")), status);
  const std::string planned = harness.ok({"notifier", "install", "--dry-run"});
  CHECK_MESSAGE((planned.starts_with("Dry run: would write") && has(planned, "/opt/app/templateapp notifier run")),
                planned);
  CHECK(has(harness.ok({"notifier", "uninstall"}), "is not set to start at sign-in"));
  CHECK(harness.ok({"notifier", "install"}).starts_with("Wrote "));
  const fs::path entry =
      harness.home.path() / ".config" / "autostart" / files::from_utf8(text::concat(meta::slug, "-notifier.desktop"));
  CHECK(has(files::read_all(entry), "Exec=\"/opt/app/templateapp\" notifier run"));
  CHECK(harness.ok({"notifier", "status"}).starts_with("Start at sign-in: on"));
  CHECK(harness.ok({"notifier", "uninstall", "--dry-run"}).starts_with("Dry run: would remove"));
  CHECK(files::exists(entry));
  CHECK(harness.ok({"notifier", "uninstall"}).starts_with("Removed "));
  CHECK_FALSE(files::exists(entry));
}

TEST_CASE("settings themes and the launcher preference are managed") {
  Harness harness("cli-config");
  const std::string listed = harness.ok({"config", "list"});
  CHECK_MESSAGE((listed.starts_with("KEY") && has(listed, "appearance.theme") && has(listed, "default")), listed);
  CHECK(harness.ok({"config", "get", "appearance.density"}) == "comfortable\n");
  CHECK(harness.ok({"config", "set", "appearance.density", "compact"}) == "appearance.density = compact\n");
  CHECK(harness.ok({"config", "get", "appearance.density"}) == "compact\n");
  const Outcome flagged =
      harness.run({"--set", "appearance.density=comfortable", "config", "get", "appearance.density"});
  CHECK(flagged.code == exit_ok);
  CHECK(flagged.output == "comfortable\n");
  CHECK_MESSAGE((has(flagged.errors, "overridden by command-line flag --set appearance.density") &&
                 has(flagged.errors, "\"compact\"")),
                flagged.errors);
  const std::string variable = app::settings::env_name("appearance.density");
  harness.set(variable, "nonsense");
  const Outcome noisy = harness.run({"config", "list"});
  CHECK(noisy.code == exit_ok);
  CHECK_MESSAGE((has(noisy.errors, "note: appearance.density:") && has(noisy.errors, "is ignored")), noisy.errors);
  harness.set(variable, "comfortable");
  const Outcome saved = harness.run({"config", "set", "appearance.density", "compact"});
  CHECK(saved.output == "appearance.density = compact\n");
  CHECK_MESSAGE(has(saved.errors, "saved, but the environment variable"), saved.errors);
  const json rows = json::parse(harness.ok({"config", "list", "--format", "json"}));
  const auto density = std::ranges::find_if(rows, [](const json& row) { return row["key"] == "appearance.density"; });
  REQUIRE(density != rows.end());
  CHECK((*density)["value"] == "comfortable");
  CHECK((*density)["source"] == "environment");
  CHECK(has((*density)["overridden_by"].get<std::string>(), variable));
  CHECK_FALSE(rows[0].contains("overridden_by"));
  CHECK_FALSE(rows[0].contains("problem"));
  harness.unset(variable);
  CHECK(harness.ok({"config", "reset", "appearance.density"}) ==
        "appearance.density is back to its default: comfortable\n");
  harness.fails({"config", "set", "appearance.density", "huge"}, exit_usage, "compact");
  harness.fails({"config", "get", "no.such"}, exit_usage, "no.such");
  harness.fails({"config", "set", "appearance.density"}, exit_usage, "give a key and a value");
  harness.fails({"config", "get"}, exit_usage, "give one setting key");
  const std::string themes = harness.ok({"config", "theme", "list"});
  CHECK_MESSAGE((has(themes, "slate") && has(themes, "in use") && has(themes, "LIGHT ACCENT")), themes);
  CHECK(harness.ok({"config", "theme", "set", "green"}).starts_with("Color theme: green ("));
  harness.fails({"config", "theme", "set", "neon"}, exit_usage, "slate");
  harness.fails({"config", "theme", "set"}, exit_usage, "give one theme name: slate");
  const std::string shown = harness.ok({"config", "theme", "show"});
  for (const std::string_view expected :
       {"Theme", "green", "system (the terminal cannot ask", "Colors listed    light", "Terminal colors", "contrast"}) {
    CHECK_MESSAGE(has(shown, expected), expected, "\n", shown);
  }
  CHECK(has(harness.ok({"config", "launcher", "show"}), "not set, the application asks at start"));
  CHECK(harness.ok({"config", "launcher", "set", "tui"}) == "Open with: tui (terminal UI)\n");
  CHECK(harness.ok({"config", "launcher", "show"}) == "Open with: tui (terminal UI)\n");
  harness.fails({"config", "launcher", "set", "web"}, exit_usage, "give gui or tui");
  harness.set(meta::env_name("ui"), "gui");
  const Outcome overridden = harness.run({"config", "launcher", "show"});
  CHECK_MESSAGE(
      has(overridden.errors, text::concat("overridden by environment variable ", meta::env_name("ui"), "=gui")),
      overridden.errors);
  CHECK(has(harness.ok({"config", "launcher", "reset"}), "not set"));
}

TEST_CASE("the database is created located moved and checked") {
  Harness harness("cli-database");
  Outcome where = harness.run({"db", "path"});
  CHECK(where.code == exit_ok);
  const std::string located(text::trim_whitespace(where.output));
  CHECK_MESSAGE(located.ends_with(text::concat(meta::slug, ".db")), where.output);
  CHECK_MESSAGE(has(where.errors, "set by: default location, not created yet"), where.errors);
  CHECK_FALSE(files::exists(files::from_utf8(located)));
  CHECK(harness.ok({"db", "init"}).starts_with("Database ready at"));
  CHECK(harness.ok({"db", "init"}).starts_with("The database already exists at"));
  CHECK(harness.ok({"db", "check"}) == "Database is healthy: integrity check ok, foreign keys ok, schema version 1\n");
  harness.ok({"records", "add", "--name", "Travels"});
  const std::string target = harness.path("moved/copy.db");
  harness.fails({"db", "move", target}, exit_usage, "--yes");
  const std::string planned = harness.ok({"db", "move", target, "--dry-run"});
  CHECK_MESSAGE(
      (planned.starts_with("Dry run: Copy the current database to") && planned.ends_with("Nothing was changed.\n")),
      planned);
  CHECK(harness.ok({"db", "move", target, "--yes"}).starts_with("Now using "));
  where = harness.run({"db", "path"});
  CHECK_MESSAGE((has(where.output, "copy.db") && has(where.errors, "set by: saved location, exists")), where.output,
                where.errors);
  CHECK(has(harness.ok({"records", "list"}), "Travels"));
  harness.fails({"db", "move", target, "--yes"}, exit_usage, "already at this location");
  harness.fails({"db", "move"}, exit_usage, "give the new database path");
  const std::string other = harness.path("other.db");
  CHECK(has(harness.ok({"db", "move", other, "--switch", "--yes"}), "other.db"));
  CHECK(has(harness.ok({"records", "list"}), "No records yet"));
  where = harness.run({"--db", target, "db", "path"});
  CHECK_MESSAGE((has(where.output, "copy.db") && has(where.errors, "command-line flag --db")), where.output,
                where.errors);
  CHECK(has(harness.ok({"--db", target, "records", "list"}), "Travels"));
  harness.fails({"--db", target, "db", "move", other, "--switch", "--yes"}, exit_usage, "remove the override first");
  harness.set(meta::env_name("db"), target);
  where = harness.run({"db", "path"});
  CHECK_MESSAGE(has(where.errors, text::concat("environment variable ", meta::env_name("db"))), where.errors);
  const std::string wrong = harness.path("notes.txt");
  const std::string words = "this is not a database, it is a text file with words";
  files::write_private(files::from_utf8(wrong), words, true);
  const Outcome refused = harness.run({"--db", wrong, "db", "check"});
  CHECK_MESSAGE(refused.code == exit_failure, refused.errors);
  CHECK(files::read_all(files::from_utf8(wrong)) == words);
}

TEST_CASE("diagnostics the guide about and licenses are available") {
  Harness harness("cli-about");
  const std::string diagnostics = harness.ok({"diagnostics"});
  for (const std::string_view expected :
       {"ITEM", "Version", "Schema version", "Integrity check", "Dashboard in this build", "Problems"}) {
    CHECK_MESSAGE(has(diagnostics, expected), expected, "\n", diagnostics);
  }
  const std::string contents = harness.ok({"guide"});
  CHECK_MESSAGE(contents.starts_with(text::concat(meta::app_name, " usage guide\n\nContents:\n")), contents);
  harness.fails({"guide", "zzzunlikelywordzzz"}, exit_failure, "no section of the usage guide mentions");
  const std::string topic = harness.ok({"guide", "keyboard"});
  CHECK_MESSAGE(has(topic, "===="), topic);
  const std::string about = harness.ok({"about"});
  for (const std::string_view expected :
       {std::string_view("Application"), std::string_view("Publisher"), meta::publisher, meta::repository_url,
        std::string_view("Report a vulnerability"), std::string_view("Nothing is opened or sent")}) {
    CHECK_MESSAGE(has(about, expected), expected, "\n", about);
  }
  const std::string issue = harness.ok({"about", "--issue"});
  CHECK_MESSAGE((has(issue, "### Environment") && has(issue, "/issues/new?")), issue);
  CHECK_MESSAGE(!has(issue, harness.path("")), "the report must not contain local paths\n", issue);
  const std::string licenses = harness.ok({"licenses"});
  CHECK_MESSAGE((has(licenses, "MIT License") && has(licenses, "Permission is hereby granted")), licenses);
  harness.fails({"licenses", "extra"}, exit_usage, "unexpected argument");
}

TEST_CASE("starting without a command follows the launch decision") {
  Harness harness("cli-launch");
  const Outcome idle = harness.run({});
  CHECK(idle.code == exit_ok);
  CHECK(idle.output.empty());
  CHECK_MESSAGE((has(idle.errors, "nothing was opened") && has(idle.errors, "Usage:")), idle.errors);
  harness.fails({"--ui", "web"}, exit_usage, "use --ui gui or --ui tui");
  harness.fails({"--ui", "gui"}, exit_usage, "this build has no dashboard");
  harness.fails({"--ui", "tui"}, exit_usage, "not an interactive terminal");
  harness.terminal = true;
  harness.tui = Words{};
  const Outcome headless = harness.run({});
  CHECK_MESSAGE(headless.code == exit_ok, headless.errors);
  CHECK_MESSAGE(has(headless.errors, "no display is available"), headless.errors);
  CHECK(harness.opened() == Words{"tui"});
  harness.set("DISPLAY", ":0");
  harness.ok({});
  const Words direct = {"tui"};
  CHECK_MESSAGE(harness.opened() == direct, "without a dashboard the terminal UI opens directly");
  harness.gui = Words{"tui"};
  harness.tui = Words{"gui", ""};
  harness.standard_input = "r\n1\n";
  const std::string prompt = harness.ok({});
  CHECK_MESSAGE((has(prompt, "How do you want to open") && has(prompt, "remember: on")), prompt);
  const Words round_trip = {"gui", "tui", "gui"};
  CHECK(harness.opened() == round_trip);
  CHECK(harness.ok({"config", "launcher", "show"}) == "Open with: gui (dashboard window)\n");
  harness.standard_input.clear();
  harness.ok({});
  CHECK_MESSAGE(harness.opened() == round_trip, "the remembered choice opens without asking");
  harness.standard_input = "q\n";
  harness.ok({"--ask"});
  CHECK(harness.opened().empty());
  harness.ok({"--ui", "tui"});
  CHECK(harness.opened().at(0) == "tui");
  harness.set(meta::env_name("ui"), "tui");
  harness.ok({});
  CHECK(harness.opened().at(0) == "tui");
  harness.unset(meta::env_name("ui"));
  harness.terminal = false;
  harness.set("TERM", "dumb");
  harness.ok({"config", "launcher", "reset"});
  harness.chooser = app::launcher::PromptResult{"tui", true, false};
  harness.fails({}, exit_failure, "no terminal window can be opened from here");
  CHECK(harness.opened() == Words{"chooser default gui"});
  CHECK(harness.ok({"config", "launcher", "show"}) == "Open with: tui (terminal UI)\n");
  harness.window = true;
  harness.ok({"--db", harness.path("w.db"), "--set", "appearance.density=compact"});
  const Words opened = harness.opened();
  REQUIRE(opened.size() == 1);
  CHECK_MESSAGE((opened[0].starts_with("window ") && has(opened[0], " --ui tui --db ")), opened[0]);
  CHECK_MESSAGE(opened[0].ends_with("--set appearance.density=compact"), opened[0]);
  harness.chooser = app::launcher::PromptResult{"", false, true};
  harness.ok({"--ask"});
  CHECK(harness.opened() == Words{"chooser default tui"});
  harness.set("CI", "1");
  const Outcome automated = harness.run({"--ask"});
  CHECK(automated.code == exit_ok);
  CHECK_MESSAGE(has(automated.errors, "automation environment"), automated.errors);
}

TEST_CASE("every capability names a real command and every command has help") {
  Harness harness("cli-capabilities");
  const Words paths = cli::command_paths();
  const auto known = [&paths](std::string_view path) { return std::ranges::find(paths, path) != paths.end(); };
  Words covered;
  for (const service::Capability& capability : service::capabilities()) {
    CHECK_MESSAGE((known(capability.cli) || capability.cli == "--ui"), capability.id, " names the unknown command ",
                  capability.cli);
    covered.emplace_back(capability.cli);
  }
  for (const std::string& path : paths) {
    Words arguments = {"help"};
    for (std::string& word : text::split(path, ' ')) {
      arguments.push_back(std::move(word));
    }
    const std::string help = harness.ok(arguments);
    CHECK_MESSAGE((has(help, text::concat(meta::slug, " ", path)) && has(help, "Global flags:")), path, "\n", help);
    const bool utility = path == "version" || path == "db path" || path == "config get";
    CHECK_MESSAGE((utility || std::ranges::find(covered, path) != covered.end()), "the command ", path,
                  " has no capability row");
  }
  const std::string reference = cli::reference();
  CHECK(text::lines(reference).size() == paths.size() + 2);
  CHECK_MESSAGE(
      has(reference, text::concat("| `", meta::slug, " records import` | `<file\\|->` `--dry-run` `--format` |")),
      reference);
  CHECK_FALSE(files::exists(harness.home.path() / ".config"));
}

TEST_CASE("flags and positional arguments can be mixed") {
  const std::vector<cli::Flag> flags = {cli::toggle("all", ""), cli::text_flag("format", "table", ""),
                                        cli::number_flag("limit", "10", "")};
  CHECK(flags[2].kind == cli::Kind::Number);
  cli::Parsed parsed = cli::parse_flags(flags, {"7", "--all", "-format=json", "x", "--", "--limit", "3"});
  CHECK(parsed.positional == Words{"7", "x", "--limit", "3"});
  CHECK(parsed.values.at("all") == "true");
  CHECK(parsed.values.at("format") == "json");
  CHECK(parsed.values.at("limit") == "10");
  CHECK(parsed.given.contains("all"));
  CHECK(parsed.given.contains("format"));
  CHECK_FALSE(parsed.given.contains("limit"));
  parsed = cli::parse_flags(flags, {"--limit", "3", "--all=false", "-"});
  CHECK(parsed.values.at("limit") == "3");
  CHECK(parsed.values.at("all") == "false");
  CHECK(parsed.positional == Words{"-"});
  CHECK(testkit::failure_of([&] { static_cast<void>(cli::parse_flags(flags, {"--format"})); }) ==
        "flag --format needs a value");
  CHECK(testkit::failure_of([&] { static_cast<void>(cli::parse_flags(flags, {"--all=perhaps"})); }) ==
        "flag --all needs true or false, found \"perhaps\"");
  CHECK(testkit::failure_of([&] { static_cast<void>(cli::parse_flags(flags, {"--nope"})); }) == "unknown flag --nope");
  CHECK(cli::tabulate("a\tbbb\tc\nlong\tb\tc\nplain\n\tx\ty") == "a     bbb  c\nlong  b    c\nplain\n  x  y");
  CHECK(cli::capitalize("created 1") == "Created 1");
  CHECK(cli::capitalize("").empty());
}

TEST_CASE("the command reference blocks are replaced between their markers") {
  const std::string document =
      "intro\n<!-- capabilities:start -->\nold\n<!-- capabilities:end -->\nmiddle\n<!-- "
      "reference:start -->\nold\n<!-- reference:end -->\nend\n";
  const std::string updated = cli::commands_document(document);
  CHECK(has(updated, service::capability_table()));
  CHECK(has(updated, cli::reference()));
  CHECK_FALSE(has(updated, "\nold\n"));
  CHECK(updated.starts_with("intro\n"));
  CHECK(updated.ends_with("<!-- reference:end -->\nend\n"));
  CHECK(cli::commands_document(updated) == updated);
  CHECK(has(testkit::failure_of([] { static_cast<void>(cli::commands_document("nothing")); }),
            "lacks the capabilities start marker"));
  CHECK(has(testkit::failure_of([] {
              static_cast<void>(cli::commands_document("<!-- capabilities:end -->\n<!-- capabilities:start -->\n"));
            }),
            "wrong order"));
}
