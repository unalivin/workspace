#include <doctest/doctest.h>

#include <algorithm>
#include <filesystem>
#include <fstream>
#include <string>
#include <string_view>
#include <vector>

#include "core/error.hpp"
#include "core/files.hpp"
#include "core/launcher.hpp"
#include "core/meta.hpp"
#include "core/records.hpp"
#include "core/service.hpp"
#include "core/settings.hpp"
#include "core/text.hpp"
#include "core/userconfig.hpp"
#include "gui/model.hpp"
#include "guikit/guikit.hpp"
#include "support/testkit.hpp"

using namespace app::gui;
using namespace guikit;
namespace files = app::files;
namespace meta = app::meta;
namespace records = app::records;
namespace service = app::service;
namespace settings = app::settings;
namespace text = app::text;
using app::theme::Severity;
using testkit::TempDir;

namespace {

const service::NavItem& bell_item() {
  return *std::ranges::find_if(service::navigation,
                               [](const service::NavItem& item) { return item.id == service::nav_notifications; });
}

void write_text(const std::filesystem::path& path, std::string_view content) {
  std::ofstream out(path, std::ios::binary | std::ios::trunc);
  out << content;
}

bool stored(const service::App& app, std::int64_t id) {
  try {
    static_cast<void>(app.records().get(id));
    return true;
  } catch (const app::AppError&) {
    return false;
  }
}

}

TEST_CASE("the first start welcomes and the overview counts") {
  const TempDir home("gui-welcome");
  service::App app = open_app(home);
  {
    Dashboard board(app, options(home));
    shows(board, {text::concat("Welcome to ", meta::app_name), "A new database was created at", "Load demo data"});
    board.answer(true);
    shows(board, {"Added 6 demo records"});
    CHECK(board.overview.open == 4);
    CHECK(board.overview.urgent == 2);
    CHECK(board.overview.unread_notifications == 2);
    CHECK(board.overview.disabled == 1);
    CHECK(board.overview.demo == 6);
    CHECK(board.nav_label(bell_item()) == "Notifications (2)");
    board.load_demo();
    shows(board, {"The demo data is already loaded"});
  }
  service::App again = open_app(home);
  const Dashboard board(again, options(home));
  CHECK_MESSAGE(!board.confirm.has_value(), "the welcome dialog appears again for an existing database");
}

TEST_CASE("navigation, the sidebar and the terminal hand over work") {
  const TempDir home("gui-navigation");
  const Log opened = new_log();
  const Log terminals = new_log();
  service::App app = open_app(home);
  {
    Dashboard board = started(app, wired(home, opened, terminals));
    for (std::size_t index = 0; index < service::navigation.size(); ++index) {
      board.shortcut({ShortcutKind::Page, index});
      CHECK(board.page_id() == service::navigation[index].id);
    }
    board.toggle_sidebar();
    CHECK(board.collapsed);
    CHECK(board.app().settings().get(settings::key_sidebar_collapsed).raw == "true");
    board.shortcut({ShortcutKind::Sidebar, 0});
    CHECK(!board.collapsed);
    board.shortcut({ShortcutKind::Guide, 0});
    CHECK(board.page_id() == service::nav_about);
    CHECK(board.about.tab == AboutTab::Guide);
    board.show_about(AboutTab::Shortcuts);
    CHECK(board.about.tab == AboutTab::Shortcuts);
    board.shortcut({ShortcutKind::Settings, 0});
    CHECK(board.page_id() == service::nav_settings);
    board.shortcut({ShortcutKind::Add, 0});
    CHECK(board.page_id() == service::nav_records);
    shows(board, {"Add record", "Name"});
    board.shortcut({ShortcutKind::Page, 0});
    CHECK_MESSAGE(board.page_id() == service::nav_records, "a shortcut changed the page behind an open form");
    board.cancel();
    board.shortcut({ShortcutKind::Search, 0});
    CHECK(board.focus_search);
    board.refresh();
    shows(board, {"Refreshed"});
    board.open_terminal();
    CHECK(*terminals == std::vector<std::string>{"--ui tui"});
    CHECK(board.finished());
    CHECK(board.next().empty());
  }
  {
    Options within = wired(home, opened, terminals);
    within.in_terminal = true;
    Dashboard inside = started(app, std::move(within));
    inside.open_terminal();
    CHECK(terminals->size() == 1);
    CHECK(inside.finished());
    CHECK(inside.next() == app::launcher::mode_tui);
  }
  {
    Dashboard none = started(app, options(home));
    none.open_terminal();
    shows(none, {"No terminal window can be opened from here"});
    CHECK(!none.finished());
  }
  Options failing = options(home);
  failing.open_terminal = [](const std::vector<std::string>&) {
    throw app::AppError::io("no terminal window could be opened");
  };
  Dashboard board = started(app, std::move(failing));
  board.open_terminal();
  shows(board, {"no terminal window could be opened"});
  CHECK(!board.finished());
  board.shortcut({ShortcutKind::Quit, 0});
  CHECK(board.finished());
}

TEST_CASE("a record is added, edited, disabled, filtered and removed") {
  const TempDir home("gui-records");
  service::App app = open_app(home);
  Dashboard board = started(app, options(home));
  board.show(service::nav_records);
  CHECK(board.empty_records_text().starts_with("No records yet. Choose Add record"));
  board.add_record();
  shows(board, {"Add record", "Name", "Required, unique"});
  board.submit();
  shows(board, {"the name is empty"});
  fill(board, {{"name", "Core switch"}, {"severity", "high"}, {"note", "rack 4"}});
  board.submit();
  CHECK_MESSAGE(!board.form.has_value(), "the form stayed open after a successful save: ", said(board));
  shows(board, {"Added record 1: Core switch"});
  REQUIRE(board.selected_record() != nullptr);
  records::Record chosen = *board.selected_record();
  std::string details = board.record_details(chosen);
  CHECK_MESSAGE(text::contains(details, "Record 1"), details);
  CHECK_MESSAGE(text::contains(details, "Severity: High"), details);
  CHECK(chosen.note == "rack 4");
  board.add_record();
  fill(board, {{"name", "Core switch"}});
  board.submit();
  shows(board, {"already exists"});
  REQUIRE(board.form.has_value());
  CHECK(board.form->value("name") == "Core switch");
  fill(board, {{"name", "Edge router"}});
  board.submit();
  shows(board, {"Added record 2: Edge router"});
  CHECK(board.rec.list.size() == 2);
  board.edit_record();
  shows(board, {"Edit record 2"});
  fill(board, {{"status", records::name(records::Status::InProgress)}});
  board.submit();
  shows(board, {"Updated record 2"});
  REQUIRE(board.selected_record() != nullptr);
  chosen = *board.selected_record();
  details = board.record_details(chosen);
  CHECK_MESSAGE(text::contains(details, "Status: In progress"), details);
  CHECK_MESSAGE(text::contains(details, "Revision: 2"), details);
  board.toggle_record();
  shows(board, {"Record 2 is disabled"});
  CHECK_MESSAGE(board.rec.query.include_disabled, "a disabled record vanished from the list");
  REQUIRE(board.selected_record() != nullptr);
  CHECK(text::contains(board.record_details(*board.selected_record()), "State: Disabled"));
  board.toggle_record();
  shows(board, {"Record 2 is enabled"});
  board.rec.query.search = "rack";
  board.load_records();
  CHECK(board.rec.list.size() == 1);
  CHECK_MESSAGE(board.selected_record() == nullptr, "the detail panel shows a record that is filtered out");
  board.rec.query.search = "zzz";
  board.load_records();
  CHECK(board.empty_records_text().starts_with("No records match these filters"));
  board.clear_filters();
  board.rec.query.status = records::Status::InProgress;
  board.load_records();
  CHECK(board.rec.list.size() == 1);
  board.rec.query.status.reset();
  board.rec.query.severity = Severity::High;
  board.load_records();
  CHECK(names(board) == std::vector<std::string>{"Core switch"});
  board.clear_filters();
  board.sort_by("name", false);
  const std::vector<std::string> sorted = {"Core switch", "Edge router"};
  CHECK(names(board) == sorted);
  board.rec.selected = 2;
  board.copy_record();
  CHECK(text::contains(board.clipboard.value_or(""), "Edge router"));
  shows(board, {"Copied record 2 to the clipboard"});
  board.remove_record();
  shows(board, {"Remove record", "This removes record 2 (Edge router) permanently."});
  board.answer(false);
  CHECK_MESSAGE(stored(board.app(), 2), "cancelling removed the record");
  board.shortcut({ShortcutKind::Remove, 0});
  board.answer(true);
  shows(board, {"Removed record 2"});
  CHECK(!stored(board.app(), 2));
  CHECK(board.rec.list.size() == 1);
  CHECK_MESSAGE(board.selected_record() == nullptr, "the detail panel of a removed record is still open");
}

TEST_CASE("an edit that lost a race keeps the form and explains") {
  const TempDir home("gui-conflict");
  service::App app = open_app(home);
  const service::App other = open_app(home);
  Dashboard board = started(app, options(home));
  const records::Record created = board.app().records().create(input("shared", Severity::Low));
  board.refresh();
  board.show(service::nav_records);
  board.rec.selected = created.id;
  board.edit_record();
  fill(board, {{"name", "shared mine"}});
  records::Input changed = created.input();
  changed.note = "from the other window";
  static_cast<void>(other.records().update(created.id, created.revision, changed));
  board.tick();
  REQUIRE(board.form.has_value());
  CHECK_MESSAGE(board.form->value("name") == "shared mine", "an external change discarded unsaved input");
  board.submit();
  shows(board, {"another window changed this record"});
  const records::Record kept = board.app().records().get(created.id);
  CHECK(kept.name == "shared");
  CHECK(kept.note == "from the other window");
  board.cancel();
  board.edit_record();
  REQUIRE(board.form.has_value());
  CHECK(board.form->value("note") == "from the other window");
  fill(board, {{"name", "shared mine"}});
  board.submit();
  shows(board, {"Updated record 1"});
}

TEST_CASE("changes from another window keep the selection and unsaved input") {
  const TempDir home("gui-external");
  service::App app = open_app(home);
  const service::App other = open_app(home);
  Dashboard board = started(app, options(home));
  for (const std::string_view name : {"alpha", "beta", "gamma"}) {
    static_cast<void>(other.records().create(input(name, Severity::Critical)));
  }
  board.show(service::nav_records);
  board.tick();
  shows(board, {"Updated by another window"});
  CHECK(board.rec.list.size() == 3);
  CHECK(board.nav_label(bell_item()) == "Notifications (3)");
  board.sort_by("name", false);
  const auto beta = std::ranges::find_if(board.rec.list, [](const records::Record& row) { return row.name == "beta"; });
  REQUIRE(beta != board.rec.list.end());
  board.rec.selected = beta->id;
  board.show(service::nav_settings);
  const std::string scale_key(settings::key_text_scale);
  const std::string theme_key(settings::key_theme);
  board.set.drafts[scale_key] = "12";
  board.set.dirty.insert(scale_key);
  static_cast<void>(other.records().create(input("aaa first", Severity::Info)));
  static_cast<void>(other.settings().set(settings::key_theme, "green"));
  board.tick();
  REQUIRE(board.selected_record() != nullptr);
  CHECK(board.selected_record()->name == "beta");
  CHECK_MESSAGE(board.set.drafts[scale_key] == "12", "an external change discarded unsaved input");
  CHECK(board.tokens.name == "green");
  CHECK(board.set.drafts[theme_key] == "green");
  shows(board, {"Updated by another window"});
}

TEST_CASE("records are imported through a preview and exported") {
  const TempDir home("gui-transfer");
  service::App app = open_app(home);
  Dashboard board = started(app, options(home));
  board.show(service::nav_records);
  const std::filesystem::path source = home.path() / "in.csv";
  write_text(source, "name,severity,enabled\nalpha,high,true\nbeta,nope,true\ngamma,low,false\n");
  board.import_records();
  shows(board, {"Import records", "Nothing is written until you confirm the preview"});
  board.submit();
  shows(board, {"choose the file to import"});
  fill(board, {{"path", files::shown(home.path() / "missing.csv")}});
  board.submit();
  shows(board, {"cannot open the import file"});
  fill(board, {{"path", files::shown(source)}});
  board.submit();
  shows(board, {"Import preview", "would have created 2, updated 0, unchanged 0, rejected 1", "Rejected row 3"});
  CHECK_MESSAGE(board.app().records().count() == 0, "the preview wrote records");
  board.answer(true);
  shows(board, {"Import finished: created 2"});
  CHECK(board.rec.list.size() == 2);
  const std::filesystem::path target = home.path() / "out.json";
  const std::string target_text = files::shown(target);
  board.export_records();
  shows(board, {"Export records", "readable only by your user account"});
  fill(board, {{"path", target_text}});
  board.submit();
  shows(board, {text::concat("Exported 2 records to ", target_text)});
  board.export_records();
  fill(board, {{"path", target_text}});
  board.submit();
  shows(board, {"already exists", "Overwrite it"});
  fill(board, {{"overwrite", "true"}, {"format", "csv"}});
  board.submit();
  CHECK_MESSAGE(!board.form.has_value(), said(board));
  CHECK(files::read_all(target).starts_with("name,status"));
}

TEST_CASE("notifications are read and rules are edited and reset") {
  const TempDir home("gui-inbox");
  service::App app = open_app(home);
  Dashboard board(app, options(home));
  board.answer(true);
  board.show(service::nav_notifications);
  CHECK(board.inbox_summary() == "2 shown, 2 unread");
  board.mark_read();
  shows(board, {"Select a notification first"});
  REQUIRE(board.inbox.list.size() == 2);
  board.inbox.selected = board.inbox.list[0].id;
  board.mark_read();
  shows(board, {"Marked the notification as read"});
  CHECK(board.inbox_summary() == "2 shown, 1 unread");
  board.inbox.unread_only = true;
  board.load_inbox();
  CHECK(board.inbox_summary() == "1 shown, 1 unread");
  board.mark_all_read();
  shows(board, {"Marked 1 notifications as read"});
  CHECK(board.empty_inbox_text().starts_with("No unread notifications"));
  board.inbox.unread_only = false;
  board.load_inbox();
  REQUIRE(board.inbox.list.size() == 2);
  const app::notify::Notification item = board.inbox.list[1];
  board.inbox.selected = item.id;
  board.open_notification_record();
  CHECK(board.page_id() == service::nav_records);
  REQUIRE(board.selected_record() != nullptr);
  CHECK(board.selected_record()->id == item.record_id);
  board.show(service::nav_notifications);
  board.inbox.rules = true;
  board.edit_rule();
  shows(board, {"Select an event type first"});
  REQUIRE(!board.inbox.rule_list.empty());
  board.inbox.rule = board.inbox.rule_list[0].event_type;
  board.edit_rule();
  shows(board, {"Rule for ", "Minimum severity", "Quiet hours start", "De-duplication"});
  fill(board, {{"start", "22:00"}});
  board.submit();
  shows(board, {"both a start and an end"});
  fill(board, {{"end", "07:00"}, {"dedup", "soon"}});
  board.submit();
  shows(board, {"whole number of minutes"});
  fill(board, {{"dedup", "15"}, {"severity", "critical"}});
  board.submit();
  shows(board, {"Saved the rule for"});
  const app::notify::Rule rule = board.inbox.rule_list[0];
  CHECK(rule.quiet_start == "22:00");
  CHECK(rule.dedup_minutes == 15);
  CHECK(rule.min_severity == Severity::Critical);
  CHECK(rule.customized);
  board.reset_rule();
  CHECK_MESSAGE(!board.inbox.rule_list[0].customized, "the rule was not reset");
}

TEST_CASE("settings are saved, explained and reset") {
  const TempDir home("gui-settings");
  const std::string variable = settings::env_name(settings::key_density);
  const app::platform::Env env =
      app::platform::Env::from_pairs({{"HOME", files::shown(home.path())}, {variable, "compact"}});
  service::Options opening;
  opening.os = app::platform::Os::Linux;
  opening.env = env;
  opening.clock = app::clock::Clock::fixed(testkit::fixed_time());
  opening.zone = app::clock::Zone::utc();
  service::App app = service::App::open(std::move(opening));
  Options wanted = options(home);
  wanted.env = env;
  Dashboard board = started(app, std::move(wanted));
  board.show(service::nav_settings);
  const auto find = [&board](std::string_view key) {
    const auto found =
        std::ranges::find_if(board.set.list, [key](const settings::Value& value) { return value.key == key; });
    REQUIRE_MESSAGE(found != board.set.list.end(), key, " has no control");
    return *found;
  };
  for (const settings::Definition& definition : settings::definitions()) {
    static_cast<void>(find(definition.key));
  }
  for (const std::string_view group : {"Data", "Notifications", "Launcher"}) {
    const bool present = std::ranges::find(board.set.groups, group) != board.set.groups.end();
    CHECK_MESSAGE(present, "no group ", group);
  }
  const std::string note = board.setting_note(find(settings::key_density));
  CHECK_MESSAGE(text::contains(note, text::concat("overridden by environment variable ", variable)), note);
  const app::theme::Color before = board.tokens.palette.accent;
  board.save_setting(settings::key_theme, "green");
  shows(board, {"Saved Color theme"});
  CHECK(board.tokens.name == "green");
  CHECK(board.tokens.palette.accent != before);
  CHECK_MESSAGE(board.restyled, "the toolkit style was not rebuilt");
  CHECK(open_app(home).settings().get(settings::key_theme).raw == "green");
  board.reset_setting(settings::key_theme);
  CHECK(board.tokens.name == "slate");
  CHECK(!find(settings::key_theme).stored.has_value());
  board.save_setting(settings::key_mode, "dark");
  CHECK(board.tokens.dark);
  board.save_setting(settings::key_mode, "light");
  CHECK(!board.tokens.dark);
  board.save_setting(settings::key_accent, "red");
  const std::string accent_key(settings::key_accent);
  const std::string scale_key(settings::key_text_scale);
  CHECK(text::contains(board.set.problems[accent_key], "#RRGGBB"));
  board.save_setting(settings::key_accent, "#FFFF00");
  shows(board, {"was adjusted"});
  CHECK_MESSAGE(!board.set.problems.contains(accent_key), "the error stayed after a valid value");
  board.save_setting(settings::key_density, "compact");
  shows(board, {text::concat("Saved Density, but the environment variable ", variable, " still takes precedence")});
  board.save_setting(settings::key_text_scale, "9000");
  const std::string problem = board.set.problems[scale_key];
  CHECK_MESSAGE(text::contains(problem, "80"), problem);
  CHECK_MESSAGE(text::contains(problem, "200"), problem);
  const float small = board.tokens.type_scale.body;
  board.save_setting(settings::key_text_scale, "150");
  CHECK(board.tokens.type_scale.body > small);
  board.save_setting(settings::key_confirm, "false");
  CHECK(board.app().settings().get(settings::key_confirm).raw == "false");
  board.remember(app::userconfig::front_end_tui);
  CHECK(board.front_end() == app::userconfig::front_end_tui);
  board.remember("");
  CHECK(board.front_end().empty());
}

TEST_CASE("the settings actions ask before they change anything") {
  const TempDir home("gui-actions");
  const Log opened = new_log();
  const Log terminals = new_log();
  service::App app = open_app(home);
  Dashboard board(app, wired(home, opened, terminals));
  board.answer(true);
  board.show(service::nav_settings);
  CHECK(board.overview.demo == 6);
  board.remove_demo();
  shows(board, {"This removes 6 demo records. Your own records are not touched."});
  board.answer(true);
  shows(board, {"Removed 6 demo records"});
  CHECK(board.overview.demo == 0);
  board.remove_demo();
  shows(board, {"There is no demo data to remove"});
  const std::filesystem::path entry =
      home.path() / ".config" / "autostart" / text::concat(meta::slug, "-notifier.desktop");
  CHECK(board.set.notifier == NotifierState{NotifierKind::Off, ""});
  board.toggle_notifier();
  shows(board, {text::concat("This writes ", files::shown(entry)), "needs no administrator rights"});
  board.answer(false);
  CHECK_MESSAGE(!files::exists(entry), "cancelling turned the notifier on");
  board.toggle_notifier();
  board.answer(true);
  CHECK_MESSAGE(files::exists(entry), "the entry was not written");
  shows(board, {"The notifier starts the next time you sign in"});
  const NotifierState on{NotifierKind::On, files::shown(entry)};
  CHECK(board.set.notifier == on);
  board.toggle_notifier();
  CHECK_MESSAGE(!files::exists(entry), "the entry was not removed");
  board.change_database();
  shows(board, {"Database location", "New location"});
  board.submit();
  REQUIRE(board.form.has_value());
  CHECK(!board.form->problem.empty());
  const std::filesystem::path target = home.path() / "elsewhere" / "data.db";
  fill(board, {{"path", files::shown(target)}});
  board.submit();
  shows(board, {text::concat("Copy the current database to ", files::shown(target))});
  board.answer(true);
  shows(board, {text::concat("Now using ", files::shown(target))});
  CHECK(board.app().location().path == target);
  CHECK(board.database_path() == files::shown(target));
}

TEST_CASE("the window size is saved and closing twice is harmless") {
  const TempDir home("gui-size");
  service::App app = open_app(home);
  {
    Dashboard board = started(app, options(home));
    const WindowSize size = board.size();
    CHECK(size.width >= 720.0F);
    CHECK(size.height >= 480.0F);
    board.keep_window_size(900.4F, 700.0F);
    board.close("");
    board.close("");
    CHECK(board.finished());
  }
  const Values values = open_app(home).settings().effective();
  CHECK(values.at(std::string(settings::key_window_width)).number() == 900);
  CHECK(values.at(std::string(settings::key_window_height)).number() == 700);
  const WindowSize kept{900.0F, 700.0F};
  CHECK(window_size(values) == kept);
}
