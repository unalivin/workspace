#include <doctest/doctest.h>

#include <ftxui/component/event.hpp>
#include <ftxui/screen/screen.hpp>

#include <algorithm>
#include <filesystem>
#include <memory>
#include <string>
#include <vector>

#include "core/error.hpp"
#include "core/files.hpp"
#include "core/meta.hpp"
#include "core/records.hpp"
#include "core/service.hpp"
#include "core/settings.hpp"
#include "core/text.hpp"
#include "core/userconfig.hpp"
#include "support/testkit.hpp"
#include "tui/model.hpp"
#include "tui/screen.hpp"
#include "tui/tui.hpp"

using namespace app::tui;
namespace files = app::files;
namespace meta = app::meta;
namespace records = app::records;
namespace service = app::service;
namespace settings = app::settings;
namespace text = app::text;
using app::platform::Os;
using testkit::TempDir;

namespace {

using Opened = std::shared_ptr<std::vector<std::string>>;
using Pairs = std::vector<std::pair<std::string, std::string>>;

app::platform::Env environment(const TempDir& home, const Pairs& extra = {}) {
  Pairs pairs = {{"HOME", files::shown(home.path())}, {"TERM", "xterm-256color"}};
  pairs.insert(pairs.end(), extra.begin(), extra.end());
  return app::platform::Env::from_pairs(std::move(pairs));
}

service::App open_app(const TempDir& home) {
  service::Options options;
  options.os = Os::Linux;
  options.env = environment(home);
  options.clock = app::clock::Clock::fixed(testkit::fixed_time());
  options.zone = app::clock::Zone::utc();
  return service::App::open(std::move(options));
}

Options options(const TempDir& home, bool dashboard, const Opened& opened = nullptr) {
  Options made;
  made.env = environment(home);
  made.system_dark = true;
  app::notifier::Autostart entry;
  entry.os = Os::Linux;
  entry.home_dir = files::shown(home.path());
  entry.executable = "/opt/app/templateapp";
  made.autostart = entry;
  made.has_dashboard = dashboard;
  if (opened) {
    made.open_url = [opened](std::string_view url) { opened->emplace_back(url); };
  }
  return made;
}

void press(Model& model, const std::vector<Key>& sequence) {
  for (const Key key : sequence) {
    model.handle(key);
  }
}

void write(Model& model, std::string_view letters) {
  for (const char32_t letter : text::decode(letters)) {
    model.handle(Key::letter(letter));
  }
}

void replace(Model& model, std::string_view letters) {
  model.handle(Key::ctrl(U'u'));
  write(model, letters);
}

Key chr(char32_t letter) {
  return Key::letter(letter);
}

const Key save = Key::ctrl(U's');

struct Message {
  Notice kind = Notice{};
  std::string text;
  bool operator==(const Message&) const = default;
};

Message message(const Model& model) {
  return {model.notice_kind(), model.notice_text()};
}

doctest::String toString(const Message& value) {
  const std::string shown = std::to_string(static_cast<int>(value.kind)) + " " + value.text;
  return doctest::String(shown.c_str());
}

std::string problem(const Model& model) {
  return model.form() == nullptr ? std::string() : model.form()->problem;
}

bool has(std::string_view whole, std::string_view part) {
  return text::contains(whole, part);
}

void select_setting(Model& model, std::string_view label) {
  model.open(service::nav_settings);
  const auto& rows = model.settings_page().rows;
  const auto found = std::ranges::find(rows, label, &SettingRow::label);
  REQUIRE(found != rows.end());
  const auto wanted = static_cast<std::size_t>(found - rows.begin());
  for (std::size_t index = 0; index < rows.size(); ++index) {
    model.handle(keys::up);
  }
  for (std::size_t index = 0; index < wanted; ++index) {
    model.handle(keys::down);
  }
  REQUIRE(model.settings_page().cursor == wanted);
}

std::string setting_value(const Model& model, std::string_view label) {
  const auto& rows = model.settings_page().rows;
  const auto found = std::ranges::find(rows, label, &SettingRow::label);
  return found == rows.end() ? std::string() : found->value;
}

std::size_t demo_total() {
  return records::demo_records().size();
}

}

TEST_CASE("the first start welcomes and offers demo data") {
  const TempDir home("tui-welcome");
  service::App app = open_app(home);
  {
    Model model(app, options(home, false));
    std::string screen = model.screen();
    CHECK_MESSAGE(has(screen, "Welcome to Template App"), screen);
    CHECK_MESSAGE(has(screen, "Press l to load labeled demo data and look around."), screen);
    model.handle(chr(U'l'));
    CHECK(message(model) ==
          Message{Notice::Success,
                  text::concat("Added ", demo_total(), " demo records. Remove them in Settings, Remove demo data.")});
    screen = model.screen();
    for (const std::string_view wanted :
         {"What needs attention", "Open records", "Demo data is loaded", "3 Notifications (", "✔ Added"}) {
      CHECK_MESSAGE(has(screen, wanted), wanted, "\n", screen);
    }
    model.handle(chr(U'l'));
    CHECK(message(model) == Message{Notice::Success, "The demo data is already loaded"});
    model.handle(keys::enter);
    CHECK(model.page_id() == service::nav_records);
  }
  Model again(app, options(home, false));
  CHECK_FALSE(has(again.screen(), "Welcome to"));
}

TEST_CASE("a record is added edited disabled and removed with the keyboard") {
  const TempDir home("tui-records");
  service::App app = open_app(home);
  Model model(app, options(home, false));
  press(model, {keys::esc, chr(U'2')});
  CHECK(has(model.screen(), "No records yet. Press n to add the first record"));
  press(model, {chr(U'n'), save});
  CHECK_MESSAGE(problem(model).starts_with("name:"), problem(model));
  CHECK(has(model.screen(), "✖ Not saved: name:"));
  write(model, "Renew the lease");
  model.handle(save);
  CHECK(message(model) == Message{Notice::Success, "Added record 1: Renew the lease"});
  std::string screen = model.screen();
  CHECK_MESSAGE((has(screen, "Renew the lease") && has(screen, "Row 1 of 1")), screen);
  press(model, {chr(U'e'), keys::tab, keys::right, keys::tab, keys::right, save});
  CHECK(message(model) == Message{Notice::Success, "Updated record 1"});
  CHECK(model.records_page().list.at(0).status == records::Status::InProgress);
  CHECK(model.records_page().list.at(0).severity == app::theme::Severity::Low);
  model.handle(keys::enter);
  screen = model.screen();
  CHECK_MESSAGE((has(screen, "Record 1: Renew the lease") && has(screen, "No note.")), screen);
  press(model, {keys::esc, chr(U'x')});
  CHECK(message(model) == Message{Notice::Success, "Record 1 is disabled"});
  CHECK(has(model.screen(), "Disabled: shown"));
  model.handle(chr(U'x'));
  CHECK(message(model) == Message{Notice::Success, "Record 1 is enabled"});
  model.handle(chr(U'd'));
  CHECK(has(model.screen(), "▲ Press y to remove it, or n or Esc to cancel."));
  model.handle(chr(U'n'));
  CHECK(message(model) == Message{Notice::Info, std::string(cancelled)});
  CHECK(model.records_page().list.size() == 1);
  press(model, {chr(U'd'), chr(U'y')});
  CHECK(message(model) == Message{Notice::Success, "Removed record 1"});
  CHECK(has(model.screen(), "No records yet."));
  CHECK(has(testkit::failure_of([&] { static_cast<void>(app.records().get(1)); }), "was not found"));
  static_cast<void>(app.settings().set(settings::key_confirm, "false"));
  model.reload();
  model.handle(chr(U'n'));
  write(model, "Short lived");
  press(model, {save, keys::del});
  CHECK(message(model) == Message{Notice::Success, "Removed record 2"});
}

TEST_CASE("an edit that lost a race keeps the form open and explains") {
  const TempDir home("tui-conflict");
  service::App app = open_app(home);
  const service::App other = open_app(home);
  Model model(app, options(home, false));
  press(model, {keys::esc, chr(U'2'), chr(U'n')});
  write(model, "Shared");
  press(model, {save, chr(U'e')});
  const records::Record current = other.records().get(1);
  records::Input changed = current.input();
  changed.note = "changed elsewhere";
  static_cast<void>(other.records().update(1, current.revision, changed));
  write(model, " edited");
  model.handle(save);
  CHECK_MESSAGE(problem(model).starts_with("another window changed this record"), problem(model));
  CHECK(model.records_page().list.at(0).note == "changed elsewhere");
  model.handle(keys::esc);
  CHECK(message(model) == Message{Notice::Info, std::string(cancelled)});
  records::Input extra;
  extra.name = "From the other window";
  static_cast<void>(other.records().create(extra));
  model.tick();
  CHECK(message(model) == Message{Notice::Info, "Updated by another window"});
  CHECK(model.records_page().list.size() == 2);
  model.tick();
  CHECK(model.records_page().list.size() == 2);
}

TEST_CASE("records are searched filtered sorted and their columns chosen") {
  const TempDir home("tui-filters");
  service::App app = open_app(home);
  Model model(app, options(home, false));
  press(model, {chr(U'l'), chr(U'2')});
  const auto& demo = records::demo_records();
  const auto enabled = static_cast<std::size_t>(std::ranges::count(demo, true, &records::DemoRecord::enabled));
  CHECK(model.records_page().list.size() == enabled);
  press(model, {chr(U'/'), chr(U'q'), keys::backspace});
  CHECK_FALSE(model.finished());
  CHECK(model.hints().starts_with("Type to search"));
  write(model, "router");
  CHECK(model.records_page().list.size() == 1);
  CHECK(has(model.screen(), "Search: router▏"));
  press(model, {keys::enter, keys::esc});
  CHECK(model.records_page().list.size() == enabled);
  model.handle(chr(U'f'));
  CHECK(has(model.screen(), "Status: Open"));
  CHECK(std::ranges::all_of(model.records_page().list,
                            [](const records::Record& record) { return record.status == records::Status::Open; }));
  press(model, {keys::esc, chr(U'v')});
  CHECK(has(model.screen(), "No records match these filters. Press Esc to clear them."));
  press(model, {keys::esc, chr(U'a')});
  CHECK(model.records_page().list.size() == demo_total());
  press(model, {keys::end, chr(U'k'), keys::home, chr(U'j')});
  CHECK(model.records_page().cursor == 1);
  model.handle(chr(U's'));
  const std::string sorted = app.settings().text(settings::key_record_sort);
  CHECK_MESSAGE(sorted.starts_with("name:"), sorted);
  model.handle(chr(U'S'));
  const std::string reversed = app.settings().text(settings::key_record_sort);
  CHECK_MESSAGE((reversed.starts_with("name:") && reversed != sorted), reversed);
  std::vector<std::string> names;
  for (const records::Record& record : model.records_page().list) {
    names.push_back(record.name);
  }
  std::vector<std::string> ordered = names;
  std::ranges::sort(ordered);
  if (reversed.ends_with(":desc")) {
    std::ranges::reverse(ordered);
  }
  CHECK(names == ordered);
  model.handle(chr(U'c'));
  REQUIRE(model.form() != nullptr);
  const std::string hidden = model.form()->fields.at(0).key;
  CHECK(has(model.screen(), "Columns of the record table"));
  press(model, {keys::right, save});
  const std::vector<std::string> columns = text::split(app.settings().text(settings::key_record_columns), ',');
  CHECK(std::ranges::find(columns, hidden) == columns.end());
  CHECK_FALSE(has(model.screen(), text::concat("  ", text::upper(hidden), "  ")));
}

TEST_CASE("records are exported and imported through a preview") {
  const TempDir home("tui-transfer");
  service::App app = open_app(home);
  Model model(app, options(home, false));
  press(model, {chr(U'l'), chr(U'2'), chr(U'o')});
  const std::string target = files::shown(home.path() / "out.csv");
  replace(model, target);
  model.handle(save);
  const Message expected{Notice::Success, text::concat("Exported ", demo_total(), " records to ", target)};
  CHECK(message(model) == expected);
  CHECK(files::read_all(files::from_utf8(target)).starts_with("name,status,severity,note,enabled"));
  model.handle(chr(U'o'));
  replace(model, target);
  model.handle(save);
  CHECK_MESSAGE(has(problem(model), "already exists. Choose another name"), problem(model));
  press(model, {keys::tab, keys::tab, keys::right, save});
  CHECK(message(model) == expected);
  press(model, {chr(U'i'), save});
  CHECK(problem(model) == "give the path of the file to import");
  write(model, files::shown(home.path() / "missing.csv"));
  model.handle(save);
  CHECK_MESSAGE(has(problem(model), "cannot open the import file"), problem(model));
  const std::string source = files::shown(home.path() / "in.csv");
  files::write_private(files::from_utf8(source),
                       "name,status,severity,note,enabled\nImported row,open,high,,true\n,open,low,,true\n", true);
  replace(model, source);
  model.handle(save);
  std::string screen = model.screen();
  for (const std::string_view wanted :
       {"Import preview", "Dry run of", "✖ rejected row", "Press Enter to import these rows"}) {
    CHECK_MESSAGE(has(screen, wanted), wanted, "\n", screen);
  }
  press(model, {keys::down, keys::page_down, keys::up, keys::esc});
  CHECK(message(model) == Message{Notice::Info, std::string(cancelled)});
  CHECK_FALSE(app.records().by_name("Imported row").has_value());
  model.handle(chr(U'i'));
  write(model, source);
  press(model, {save, keys::enter});
  CHECK(model.notice_kind() == Notice::Success);
  CHECK_MESSAGE(model.notice_text().starts_with("Import finished: "), model.notice_text());
  CHECK(app.records().by_name("Imported row").has_value());
  files::write_private(files::from_utf8(source), "name,status,severity,note,enabled\n,open,low,,true\n", true);
  model.handle(chr(U'i'));
  write(model, source);
  model.handle(save);
  screen = model.screen();
  CHECK_MESSAGE((has(screen, "There is nothing to import.") && has(screen, "Esc close")), screen);
  model.handle(keys::enter);
  CHECK(model.preview() == nullptr);
}

TEST_CASE("notifications are read and rules are edited") {
  const TempDir home("tui-inbox");
  service::App app = open_app(home);
  Model model(app, options(home, false));
  model.resize(120, 30);
  press(model, {keys::esc, chr(U'3')});
  CHECK(has(model.screen(), "No notifications. They appear when a record needs attention or is completed."));
  press(model, {chr(U'1'), chr(U'l'), chr(U'3')});
  const std::size_t unread = model.inbox_page().list.size();
  REQUIRE(unread >= 1);
  std::string screen = model.screen();
  CHECK_MESSAGE(has(screen, text::concat("Showing all notifications · ", unread, " unread")), screen);
  CHECK_MESSAGE(has(screen, "A record needs attention"), screen);
  const app::notify::Notification first = model.inbox_page().list.at(0);
  model.handle(chr(U'm'));
  CHECK(message(model) == Message{Notice::Success, text::concat("Marked notification ", first.id, " as read")});
  model.handle(keys::enter);
  CHECK(model.page_id() == service::nav_records);
  CHECK(has(model.screen(), text::concat("Record ", first.record_id, ": ")));
  press(model, {chr(U'3'), chr(U'M')});
  CHECK(message(model) == Message{Notice::Success, text::concat("Marked ", unread - 1, " notifications as read")});
  model.handle(chr(U'u'));
  CHECK(has(model.screen(), "No unread notifications. Press u to show all of them."));
  press(model, {chr(U'u'), keys::right});
  screen = model.screen();
  CHECK_MESSAGE((has(screen, "MIN SEVERITY") && has(screen, "▌ Rules")), screen);
  CHECK(model.hints().starts_with("e edit rule"));
  const std::string event = model.inbox_page().rule_list.at(0).event_type;
  press(model, {chr(U'e'), keys::tab, keys::tab, keys::tab, keys::tab});
  replace(model, "soon");
  model.handle(save);
  CHECK(problem(model) == "de-duplication needs a whole number of minutes, for example 60");
  replace(model, "30");
  press(model, {keys::back_tab, keys::back_tab});
  replace(model, "25:00");
  model.handle(save);
  CHECK_FALSE(problem(model).empty());
  replace(model, "");
  model.handle(save);
  CHECK(message(model) == Message{Notice::Success, text::concat("Saved the rule for ", event)});
  CHECK(model.inbox_page().rule_list.at(0).dedup_minutes == 30);
  CHECK(model.inbox_page().rule_list.at(0).customized);
  CHECK(has(model.screen(), "changed"));
  model.handle(chr(U'u'));
  CHECK(message(model) == Message{Notice::Success, text::concat("Rule for ", event, " is back to its defaults")});
  CHECK_FALSE(model.inbox_page().rule_list.at(0).customized);
  model.handle(keys::left);
  CHECK(has(model.screen(), "▌ Inbox"));
}

TEST_CASE("settings are changed reset and the actions ask first") {
  const TempDir home("tui-settings");
  service::App app = open_app(home);
  Model model(app, options(home, false));
  model.resize(120, 70);
  press(model, {keys::esc, chr(U',')});
  const std::string screen = model.screen();
  for (const std::string_view wanted :
       {"Appearance", "Database location", "Remove demo data", "Notifier at sign-in", "Open with"}) {
    CHECK_MESSAGE(has(screen, wanted), wanted, "\n", screen);
  }
  select_setting(model, "Density");
  press(model, {keys::enter, keys::right, save});
  CHECK(message(model) == Message{Notice::Success, "Saved Density"});
  CHECK(app.settings().text(settings::key_density) != "comfortable");
  model.handle(chr(U'u'));
  CHECK(message(model) == Message{Notice::Success, "Density is back to its default"});
  CHECK(app.settings().text(settings::key_density) == "comfortable");
  select_setting(model, "Notifier interval");
  model.handle(keys::enter);
  replace(model, "5");
  model.handle(save);
  CHECK_FALSE(problem(model).empty());
  replace(model, "120");
  model.handle(save);
  CHECK(app.settings().number(settings::key_notifier_interval) == 120);
  CHECK(setting_value(model, "Notifier interval") == "120");
  select_setting(model, "Remove demo data");
  model.handle(keys::enter);
  CHECK(message(model) == Message{Notice::Info, "There is no demo data to remove"});
  press(model, {chr(U'1'), chr(U'l')});
  select_setting(model, "Remove demo data");
  model.handle(keys::enter);
  CHECK(has(model.screen(), text::concat("This removes ", demo_total(), " demo records.")));
  model.handle(chr(U'y'));
  CHECK(message(model) == Message{Notice::Success, text::concat("Removed ", demo_total(), " demo records")});
  select_setting(model, "Open with");
  model.handle(keys::enter);
  CHECK(message(model) == Message{Notice::Success, "Open with: Dashboard window"});
  CHECK(app.front_end() == app::userconfig::front_end_gui);
  model.handle(keys::enter);
  CHECK(setting_value(model, "Open with") == "Terminal UI");
  model.handle(chr(U'u'));
  CHECK(message(model) == Message{Notice::Success, "Open with: Ask every time"});
  select_setting(model, "Notifier at sign-in");
  CHECK(setting_value(model, "Notifier at sign-in") == "Off");
  model.handle(keys::enter);
  REQUIRE(model.confirmation() != nullptr);
  CHECK(has(model.confirmation()->statement, "notifier run\" starts when you sign in"));
  model.handle(chr(U'y'));
  CHECK(message(model) == Message{Notice::Success, "The notifier starts the next time you sign in"});
  CHECK(setting_value(model, "Notifier at sign-in") == "On");
  press(model, {keys::enter, chr(U'y')});
  CHECK(message(model) == Message{Notice::Success, "The notifier no longer starts at sign-in"});
  select_setting(model, "Database location");
  model.handle(keys::enter);
  model.handle(save);
  CHECK_FALSE(problem(model).empty());
  const std::filesystem::path target = home.path() / "moved" / "data.db";
  write(model, files::shown(target));
  model.handle(save);
  CHECK(model.confirmation() != nullptr);
  model.handle(chr(U'y'));
  CHECK(message(model) == Message{Notice::Success, text::concat("Now using ", files::shown(target))});
  CHECK(app.location().path == target);
  CHECK(files::exists(target));
}

TEST_CASE("about opens links only on request and shows the guide and diagnostics") {
  const TempDir home("tui-about");
  service::App app = open_app(home);
  const Opened opened = std::make_shared<std::vector<std::string>>();
  {
    Model quiet(app, options(home, false));
    quiet.resize(120, 40);
    press(quiet, {keys::esc, chr(U'5')});
    const std::string screen = quiet.screen();
    for (const std::string& wanted :
         {text::concat("Publisher: ", meta::publisher), std::string("License: MIT"), std::string("Source Code"),
          std::string("Report a vulnerability privately"), std::string("Nothing is sent by the application.")}) {
      CHECK_MESSAGE(has(screen, wanted), wanted, "\n", screen);
    }
    quiet.handle(keys::enter);
    CHECK(quiet.notice_kind() == Notice::Failure);
    CHECK_MESSAGE(has(quiet.notice_text(), "no browser can be started from here. Copy this link:"),
                  quiet.notice_text());
  }
  Model model(app, options(home, false, opened));
  model.resize(120, 40);
  press(model, {chr(U'5'), keys::enter});
  CHECK(message(model) == Message{Notice::Success, "Opened Source Code in the browser"});
  REQUIRE(opened->size() == 1);
  CHECK(opened->at(0) == service::source_link().url);
  press(model, {keys::down, keys::enter});
  std::string screen = model.screen();
  CHECK_MESSAGE((has(screen, "Report a bug") && has(screen, "Press Enter to open the report form in the browser")),
                screen);
  CHECK(opened->size() == 1);
  model.handle(keys::enter);
  CHECK(message(model) == Message{Notice::Success, "Opened Report a bug in the browser"});
  REQUIRE(opened->size() == 2);
  CHECK_MESSAGE(has(opened->at(1), "/issues/new?title="), opened->at(1));
  model.handle(keys::right);
  CHECK(has(model.screen(), "Search: press / to search"));
  model.handle(keys::enter);
  CHECK(has(model.screen(), "Esc back to the contents"));
  press(model, {keys::right, keys::down, keys::page_down});
  CHECK(has(model.screen(), "Esc back to the contents"));
  press(model, {keys::esc, chr(U'/')});
  write(model, "zzqzz");
  CHECK_FALSE(model.finished());
  CHECK(has(model.screen(), "No section mentions these words. Press Esc to show the contents."));
  press(model, {keys::esc, keys::right});
  CHECK(has(model.screen(), "Open the menu"));
  model.handle(keys::right);
  CHECK(has(model.screen(), "Diagnostics stay on this computer."));
  press(model, {keys::right, keys::page_down, keys::page_down});
  CHECK(has(model.screen(), "▌Licenses"));
  press(model, {keys::right, keys::left, chr(U'1'), chr(U'?')});
  CHECK(model.page_id() == service::nav_about);
  CHECK(has(model.screen(), "▌Usage guide"));
}

TEST_CASE("the menu the sidebar and quitting work from the keyboard") {
  const TempDir home("tui-menu");
  service::App app = open_app(home);
  {
    Model model(app, options(home, false));
    press(model, {keys::esc, chr(U':')});
    const std::string screen = model.screen();
    CHECK_MESSAGE((has(screen, "Menu") && has(screen, "Not available: this build has no dashboard")), screen);
    model.handle(keys::enter);
    CHECK(model.notice_kind() == Notice::Warning);
    CHECK_MESSAGE(model.notice_text().starts_with("This build has no dashboard."), model.notice_text());
    press(model, {Key::function(10), keys::down, keys::down, keys::down, keys::down, keys::enter});
    CHECK(message(model) == Message{Notice::Success, "Refreshed"});
    press(model, {chr(U':'), keys::esc, keys::tab});
    CHECK(model.page_id() == service::nav_records);
    CHECK(model.hints().starts_with("n add"));
    model.handle(keys::back_tab);
    CHECK(has(model.screen(), "1 Overview"));
    model.handle(chr(U'b'));
    CHECK_FALSE(has(model.screen(), "1 Overview"));
    CHECK(app.settings().text(settings::key_sidebar_collapsed) == "true");
    model.resize(min_width - 1, min_height);
    CHECK(has(model.screen(), "needs a terminal of at least 60 columns and 16 rows."));
    model.resize(100, 30);
    press(model, {chr(U':'), keys::up, keys::enter});
    CHECK(model.finished());
    CHECK(model.switch_to().empty());
    CHECK(model.view().empty());
  }
  {
    Model model(app, options(home, true));
    CHECK_FALSE(has(model.screen(), "1 Overview"));
    press(model, {chr(U':'), keys::enter});
    CHECK(model.finished());
    CHECK(model.switch_to() == app::userconfig::front_end_gui);
  }
  Model model(app, options(home, true));
  model.handle(Key::ctrl(U'c'));
  CHECK(model.finished());
}

TEST_CASE("every screen fills the terminal exactly at every size") {
  const TempDir home("tui-sizes");
  service::App app = open_app(home);
  Model model(app, options(home, false));
  const auto check = [&model](std::string_view what) {
    const std::vector<std::pair<std::size_t, std::size_t>> sizes = {
        {min_width, min_height}, {80, 24}, {110, 30}, {200, 60}};
    for (const auto& [columns, rows] : sizes) {
      model.resize(columns, rows);
      const Lines lines = model.view();
      CHECK_MESSAGE(lines.size() == rows - 1, what, " at ", columns, "x", rows);
      for (const Line& line : lines) {
        CHECK_MESSAGE(line.width() == columns, what, " at ", columns, "x", rows, ": ", line.text());
      }
    }
  };
  check("welcome");
  model.handle(chr(U'l'));
  check("notice");
  for (const char32_t page : std::u32string_view(U"12345")) {
    model.handle(chr(page));
    check("page");
  }
  press(model, {chr(U'2'), keys::enter});
  check("detail");
  model.handle(chr(U'e'));
  write(model, " 日本語のテキスト and a very long name that keeps going beyond the width of a narrow terminal window");
  check("form");
  press(model, {keys::esc, chr(U'd')});
  check("confirm");
  press(model, {keys::esc, chr(U':')});
  check("menu");
  press(model, {keys::esc, chr(U'3'), keys::right});
  check("rules");
  for (int turn = 0; turn < 4; ++turn) {
    press(model, {chr(U'5'), keys::right});
    check("about");
  }
  CHECK(short_hints("a · b · c", 40) == "a · b · c");
  const std::string shortened = short_hints(model.hints(), 58);
  CHECK_MESSAGE((width(shortened) <= 58 && shortened.ends_with("… · ? guide · q quit")), shortened);
  CHECK(short_hints(model.hints(), 10) == "… · ? guide · q quit");
}

TEST_CASE("keys and styles are converted and the screen is painted") {
  CHECK(convert(ftxui::Event::Character("n")) == chr(U'n'));
  CHECK(convert(ftxui::Event::Character("語")) == chr(U'語'));
  CHECK(convert(ftxui::Event::CtrlC) == Key::ctrl(U'c'));
  CHECK(convert(ftxui::Event::CtrlS) == Key::ctrl(U's'));
  CHECK(convert(ftxui::Event::CtrlU) == Key::ctrl(U'u'));
  CHECK(convert(ftxui::Event::TabReverse) == keys::back_tab);
  CHECK(convert(ftxui::Event::Tab) == keys::tab);
  CHECK(convert(ftxui::Event::Return) == keys::enter);
  CHECK(convert(ftxui::Event::Escape) == keys::esc);
  CHECK(convert(ftxui::Event::Backspace) == keys::backspace);
  CHECK(convert(ftxui::Event::Delete) == keys::del);
  CHECK(convert(ftxui::Event::PageDown) == keys::page_down);
  CHECK(convert(ftxui::Event::F10) == Key::function(10));
  CHECK_FALSE(convert(ftxui::Event::Insert).has_value());
  CHECK_FALSE(convert(ftxui::Event::Custom).has_value());
  const TempDir home("tui-paint");
  const std::vector<std::pair<std::string, bool>> backgrounds = {
      {"15;0", true}, {"0;15", false}, {"0;default;7", false}, {"12;8", true}, {"unknown", true}};
  for (const auto& [reported, dark] : backgrounds) {
    CHECK_MESSAGE(dark_background(environment(home, {{"COLORFGBG", reported}})) == dark, reported);
  }
  CHECK(dark_background(environment(home)));
  service::App app = open_app(home);
  Model model(app, options(home, false));
  model.handle(keys::esc);
  CHECK(model.profile() == app::theme::Profile::Ansi256);
  const CellStyle heading = convert_style(model.style(Look::Heading), model.profile());
  CHECK(heading.foreground != ftxui::Color(ftxui::Color::Default));
  CHECK(heading.bold);
  ftxui::Screen screen(90, 24);
  draw(screen, model);
  const auto line = [&screen](int y) {
    std::string out;
    for (int x = 0; x < 90; ++x) {
      out.append(screen.PixelAt(x, y).character);
    }
    return out;
  };
  CHECK_MESSAGE(line(0).starts_with("Template App · Overview"), line(0));
  CHECK(line(1) == repeat("─", 90));
  CHECK_MESSAGE(line(2).starts_with("▌1 Overview"), line(2));
  CHECK_MESSAGE(has(line(22), ": menu · ? guide · q quit"), line(22));
  CHECK(screen.PixelAt(0, 0).foreground_color == heading.foreground);
  CHECK(screen.PixelAt(0, 0).bold);
  const CellStyle plain = convert_style(app::theme::Style{}, app::theme::Profile::Mono);
  CHECK(plain.foreground == ftxui::Color(ftxui::Color::Default));
  app::theme::Style direct;
  direct.foreground = app::theme::Color{10, 20, 30};
  direct.background = app::theme::Ansi{4};
  direct.reverse = true;
  const CellStyle exact = convert_style(direct, app::theme::Profile::TrueColor);
  CHECK(exact.foreground == ftxui::Color::RGB(10, 20, 30));
  CHECK(exact.background == ftxui::Color(ftxui::Color::Blue));
  CHECK(exact.reverse);
}

TEST_CASE("every terminal capability is reachable") {
  const TempDir home("tui-capabilities");
  service::App app = open_app(home);
  Model model(app, options(home, true));
  model.resize(140, 50);
  const std::string welcome = model.screen();
  CHECK_MESSAGE(has(welcome, "Welcome to"), welcome);
  std::vector<std::string> screens;
  std::vector<std::string> hints;
  model.handle(keys::esc);
  hints.push_back(model.hints());
  model.handle(chr(U'l'));
  for (std::size_t index = 0; index < service::navigation.size(); ++index) {
    model.handle(chr(U'1' + static_cast<char32_t>(index)));
    screens.push_back(model.screen());
    hints.push_back(model.hints());
    for (int turn = 0; turn < 3; ++turn) {
      model.handle(keys::page_down);
      screens.push_back(model.screen());
    }
  }
  press(model, {chr(U'3'), keys::right});
  screens.push_back(model.screen());
  hints.push_back(model.hints());
  press(model, {keys::left, chr(U'5')});
  for (int turn = 0; turn < 6; ++turn) {
    model.handle(keys::right);
    screens.push_back(model.screen());
  }
  press(model, {chr(U'1'), chr(U':')});
  screens.push_back(model.screen());
  const std::string everything = text::join(screens, "\n");
  std::vector<std::string> known;
  for (const std::string& line : hints) {
    std::size_t start = 0;
    while (start <= line.size()) {
      const std::size_t found = line.find(" · ", start);
      known.push_back(line.substr(start, found == std::string::npos ? found : found - start));
      if (found == std::string::npos) {
        break;
      }
      start = found + std::string_view(" · ").size();
    }
  }
  const auto hinted = [&known](std::string_view key) {
    const std::string lead = text::concat(key, " ");
    return std::ranges::any_of(known, [&lead](const std::string& hint) { return hint.starts_with(lead); });
  };
  const auto parts_of = [](std::string_view whole, std::string_view separator) {
    std::vector<std::string> parts;
    std::size_t start = 0;
    while (start <= whole.size()) {
      const std::size_t found = whole.find(separator, start);
      parts.emplace_back(whole.substr(start, found == std::string_view::npos ? found : found - start));
      if (found == std::string_view::npos) {
        break;
      }
      start = found + separator.size();
    }
    return parts;
  };
  std::vector<std::string> missing;
  for (const service::Capability& capability : service::capabilities()) {
    if (capability.terminal == service::not_applicable || capability.terminal == "First-start screen") {
      continue;
    }
    for (const std::string& part : parts_of(capability.terminal, ", ")) {
      bool found = false;
      if (text::length(part) <= 2 || has(part, " and ")) {
        const std::vector<std::string> wanted = parts_of(part, " and ");
        found = std::ranges::all_of(wanted, hinted);
      } else {
        found = has(everything, part) || hinted(part);
      }
      if (!found) {
        missing.push_back(text::concat(capability.id, ": ", part));
      }
    }
  }
  CHECK_MESSAGE(missing.empty(), "not found in the terminal UI: ", text::join(missing, "; "),
                "\nhints: ", text::join(known, " | "), "\n", everything);
}
