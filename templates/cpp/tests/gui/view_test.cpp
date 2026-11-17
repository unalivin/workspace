#include <doctest/doctest.h>

#include <imgui.h>

#include <algorithm>
#include <array>
#include <cmath>
#include <cstdint>
#include <string>
#include <string_view>
#include <vector>

#include "core/files.hpp"
#include "core/launcher.hpp"
#include "core/meta.hpp"
#include "core/records.hpp"
#include "core/service.hpp"
#include "core/settings.hpp"
#include "core/text.hpp"
#include "core/theme.hpp"
#include "gui/chooser.hpp"
#include "gui/icons.hpp"
#include "gui/model.hpp"
#include "gui/render.hpp"
#include "gui/shots.hpp"
#include "gui/skin.hpp"
#include "gui/ui.hpp"
#include "gui/view.hpp"
#include "guikit/guikit.hpp"
#include "support/testkit.hpp"

using namespace app::gui;
using namespace guikit;
namespace meta = app::meta;
namespace records = app::records;
namespace service = app::service;
namespace settings = app::settings;
namespace text = app::text;
namespace theme = app::theme;
using app::theme::Severity;
using testkit::TempDir;

namespace {

std::uint64_t brightness(const Picture& picture) {
  std::uint64_t total = 0;
  for (std::size_t offset = 0; offset + 3 < picture.pixels.size(); offset += 4) {
    total += std::uint64_t{picture.pixels[offset]} + picture.pixels[offset + 1] + picture.pixels[offset + 2];
  }
  return total;
}

const Picture& named(const std::vector<std::pair<std::string_view, Picture>>& pictures, std::string_view name) {
  const auto found = std::ranges::find_if(pictures, [name](const auto& entry) { return entry.first == name; });
  REQUIRE(found != pictures.end());
  return found->second;
}

std::size_t tinted(const Picture& picture, std::size_t row, std::size_t from, std::size_t to, theme::Color tint) {
  std::size_t count = 0;
  for (std::size_t x = from; x < std::min(to, picture.width); ++x) {
    if (near(picture.at(x, row), tint)) {
      count += 1;
    }
  }
  return count;
}

float center_y(const Seen& item) {
  return (item.min.y + item.max.y) / 2.0F;
}

void search_from_the_top_bar(Window& window, Dashboard& board, std::string_view words) {
  board.search.clear();
  static_cast<void>(window.paint(2));
  const Seen* field = window.view().find(SeenKind::Input, search_hint);
  REQUIRE_MESSAGE(field != nullptr, "the top bar has no search field:\n", window.text());
  window.click_at(field->middle());
  window.type(words);
  CHECK_MESSAGE(board.search == words, "typing did not reach the search field");
  window.chord(ImGuiKey_Enter, false);
  static_cast<void>(window.paint(2));
}

class Asked {
 public:
  explicit Asked(std::string_view default_mode)
      : stage_(chooser::width, chooser::height, 1.0F),
        faces_(install_fonts()),
        tokens_(chooser::tokens(false)),
        default_mode_(default_mode) {
    apply(tokens_);
    static_cast<void>(picture());
  }

  chooser::Choice frame() {
    chooser::Choice answer;
    stage_.run([&] {
      seen_.clear();
      const Ui ui(tokens_, faces_, icons_, seen_, 1.0F);
      answer = chooser::show(ui, default_mode_, remember, false);
    });
    return answer;
  }

  Picture picture() {
    for (int index = 0; index < 3; ++index) {
      static_cast<void>(frame());
    }
    return stage_.picture();
  }

  const Seen& item(std::string_view label) {
    const auto found = std::ranges::find_if(seen_, [label](const Seen& seen) {
      return seen.text == label && seen.max.x > seen.min.x && seen.kind != SeenKind::Icon;
    });
    REQUIRE_MESSAGE(found != seen_.end(), "the chooser does not show \"", label, "\":\n", lines());
    return *found;
  }

  chooser::Choice click(std::string_view label) {
    const ImVec2 point = item(label).middle();
    chooser::Choice answer;
    const auto keep = [&answer](const chooser::Choice& choice) {
      if (choice.kind != chooser::ChoiceKind::Waiting) {
        answer = choice;
      }
    };
    stage_.move(point);
    keep(frame());
    stage_.press(true);
    keep(frame());
    stage_.press(false);
    keep(frame());
    keep(frame());
    return answer;
  }

  bool marked(std::string_view label) {
    const Picture shot = picture();
    const Seen& button = item(label);
    const auto row = static_cast<std::size_t>(std::lround(center_y(button)));
    const auto from = static_cast<std::size_t>(button.min.x);
    const auto to = static_cast<std::size_t>(button.max.x);
    return tinted(shot, row, from, to, tokens_.palette.accent) > 4;
  }

  [[nodiscard]] std::string lines() const {
    std::string joined;
    for (const Seen& seen : seen_) {
      if (seen.kind != SeenKind::Icon && seen.kind != SeenKind::Hint) {
        joined += seen.text;
        joined += '\n';
      }
    }
    return joined;
  }

  [[nodiscard]] std::vector<std::string> drawn() const {
    return icons_.drawn();
  }

  bool remember = false;

 private:
  Stage stage_;
  Faces faces_;
  theme::Tokens tokens_;
  Icons icons_;
  std::vector<Seen> seen_;
  std::string_view default_mode_;
};

}

TEST_CASE("every main screen renders to a picture") {
  const TempDir home("gui-screens");
  service::App app = open_app(home);
  const std::vector<std::pair<std::string_view, Picture>> pictures = shots::screens(app, options(home));
  REQUIRE(pictures.size() == shots::names.size());
  for (std::size_t index = 0; index < pictures.size(); ++index) {
    const auto& [name, picture] = pictures[index];
    CHECK(name == shots::names[index]);
    CHECK_MESSAGE((picture.width >= 400 && picture.height >= 200), name, " is ", picture.width, "x", picture.height);
    CHECK_MESSAGE(tints(picture) > 24, name, " shows only ", tints(picture), " colors");
  }
  CHECK(brightness(named(pictures, "settings-dark")) * 2 < brightness(named(pictures, "settings")));
}

TEST_CASE("about opens links only on request and shows the guide and diagnostics") {
  const TempDir home("gui-about");
  const Log opened = new_log();
  const Log terminals = new_log();
  service::App app = open_app(home);
  {
    Dashboard board = started(app, wired(home, opened, terminals));
    board.show(service::nav_about);
    Window window(board, 1280.0F, 1200.0F);
    const std::string about = window.paint();
    for (const std::string& wanted : {text::concat("Publisher: ", meta::publisher), std::string("License: MIT")}) {
      CHECK_MESSAGE(text::contains(about, wanted), "the About page lacks \"", wanted, "\":\n", about);
    }
    board.follow(service::source_link());
    CHECK(*opened == std::vector<std::string>{meta::source_url()});
    shows(board, {"Opened Source Code in your browser"});
    board.report_bug();
    shows(board, {"Report a bug", "prefilled with exactly this text and nothing else", "Operating system:"});
    REQUIRE(board.confirm.has_value());
    const bool leaked = text::contains(board.confirm->statement, app::files::shown(home.path()));
    CHECK_MESSAGE(!leaked, "the preview contains a local path");
    CHECK_MESSAGE(opened->size() == 1, "the browser opened before the preview was confirmed");
    board.answer(false);
    board.report_bug();
    board.answer(true);
    REQUIRE(opened->size() == 2);
    CHECK((*opened)[1].starts_with(text::concat(meta::source_url(), "/issues/new?")));
    board.follow(service::advisory_link());
    REQUIRE(opened->size() == 3);
    CHECK((*opened)[2].ends_with("/security/advisories/new"));
    board.show_about(AboutTab::Guide);
    CHECK(board.about.hits.size() == board.about.usage.sections.size());
    board.about.search = "quiet hours";
    board.filter_guide();
    CHECK(!board.about.hits.empty());
    CHECK(board.about.hits.size() < board.about.usage.sections.size());
    const std::size_t first = board.about.hits.front();
    CHECK(text::contains(text::lower(board.about.usage.sections[first].body), "quiet hours"));
    board.about.search = "zzzzqq";
    board.filter_guide();
    CHECK(board.about.hits.empty());
    board.show_about(AboutTab::Diagnostics);
    const std::string report = board.diagnostics_text();
    CHECK_MESSAGE(text::contains(report, "Schema version"), report);
    CHECK_MESSAGE(text::contains(report, "Integrity check"), report);
    board.check_database();
    shows(board, {"The database is healthy"});
    board.copy_diagnostics();
    CHECK(text::contains(board.clipboard.value_or(""), "Schema version"));
    static_cast<void>(window.paint(1));
    CHECK_MESSAGE(text::contains(window.stage().clipboard(), "Schema version"),
                  "the report did not reach the system clipboard");
  }
  Dashboard blocked = started(app, options(home));
  blocked.follow(service::source_link());
  shows(blocked, {text::concat("Copy this link: ", meta::source_url())});
}

TEST_CASE("reduced motion skips the notice fade") {
  const TempDir home("gui-motion");
  service::App app = open_app(home);
  Dashboard board = started(app, options(home));
  Window window(board, shots::width, shots::height);
  static_cast<void>(window.paint(2));
  board.refresh();
  static_cast<void>(window.paint(1));
  const double shown = ImGui::GetTime();
  CHECK(window.view().notice_wash(shown + 0.1) > 0);
  CHECK(window.view().notice_wash(shown + 5.0) == 0);
  board.save_setting(settings::key_reduced_motion, "true");
  CHECK(!board.tokens.motion);
  board.refresh();
  static_cast<void>(window.paint(1));
  CHECK(window.view().notice_wash(ImGui::GetTime() + 0.1) == 0);
}

TEST_CASE("keys reach the dashboard through the window") {
  const TempDir home("gui-keys");
  service::App app = open_app(home);
  Dashboard board = started(app, options(home));
  board.load_demo();
  Window window(board, shots::width, shots::height);
  static_cast<void>(window.paint(2));
  CHECK(!board.restyled);
  window.chord(ImGuiKey_2, true);
  CHECK(board.page_id() == service::nav_records);
  window.chord(ImGuiKey_F1, false);
  CHECK(board.page_id() == service::nav_about);
  CHECK(board.about.tab == AboutTab::Guide);
  window.chord(ImGuiKey_B, true);
  CHECK(board.collapsed);
  window.chord(ImGuiKey_N, true);
  CHECK(board.form.has_value());
  static_cast<void>(window.paint(2));
  window.chord(ImGuiKey_Escape, false);
  static_cast<void>(window.paint(2));
  CHECK_MESSAGE(!board.form.has_value(), "Escape did not close the form");
  board.show(service::nav_records);
  REQUIRE(!board.rec.list.empty());
  board.rec.selected = board.rec.list.front().id;
  static_cast<void>(window.paint(2));
  window.chord(ImGuiKey_Delete, false);
  shows(board, {"Remove record"});
  window.chord(ImGuiKey_Q, true);
  CHECK(board.finished());
  CHECK(board.page_id() == service::nav_records);
}

TEST_CASE("the record table leaves out columns the detail panel shows before it scrolls") {
  const TempDir home("gui-fit");
  service::App app = open_app(home);
  Dashboard board = started(app, options(home));
  board.load_demo();
  board.show(service::nav_records);
  REQUIRE(!board.rec.list.empty());
  Window window(board, 1180.0F, 760.0F);
  static_cast<void>(window.paint(2));
  CHECK(window.all("Updated").size() == 1);
  CHECK(window.all("ID").size() == 1);
  board.rec.selected = board.rec.list.front().id;
  static_cast<void>(window.paint(2));
  CHECK_MESSAGE(window.view().table_width() <= window.view().table_room(),
                "the table scrolls: ", window.view().table_width(), " in ", window.view().table_room());
  CHECK(window.all("Updated").empty());
  CHECK(window.all("ID").size() == 1);
  CHECK(window.all("Name").size() == 1);
  CHECK(window.all("Severity").size() == 1);
  CHECK(text::contains(window.text(), "Updated: "));
  board.rec.selected.reset();
  static_cast<void>(window.paint(2));
  CHECK(window.all("Updated").size() == 1);
}

TEST_CASE("a record table wider than its space scrolls sideways") {
  const TempDir home("gui-scroll");
  service::App app = open_app(home);
  Dashboard board = started(app, options(home));
  board.load_demo();
  board.show(service::nav_records);
  REQUIRE(!board.rec.list.empty());
  board.rec.selected = board.rec.list.front().id;
  Window window(board, 900.0F, 600.0F);
  const Picture before = window.picture();
  CHECK_MESSAGE(window.view().table_width() > window.view().table_room(),
                "the table fits: ", window.view().table_width(), " in ", window.view().table_room());
  const Seen header = window.leftmost("Name");
  const float header_before = header.min.x;
  window.wheel(ImVec2(420.0F, 300.0F), -8.0F, 0.0F);
  const Picture after = window.picture();
  const auto row = [](const Picture& picture) {
    std::vector<std::array<std::uint8_t, 4>> pixels;
    for (std::size_t x = 230; x < 520; ++x) {
      pixels.push_back(picture.at(x, 300));
    }
    return pixels;
  };
  CHECK_MESSAGE(row(before) != row(after), "the table did not move");
  CHECK(window.leftmost("Name").min.x < header_before);
  CHECK(board.rec.selected == board.rec.list.front().id);
}

TEST_CASE("every dashboard capability is reachable") {
  const TempDir home("gui-capabilities");
  const Log opened = new_log();
  const Log terminals = new_log();
  service::App app = open_app(home);
  Dashboard board(app, wired(home, opened, terminals));
  Window window(board, 1280.0F, 2600.0F);
  std::vector<std::string> seen;
  const std::string welcome = window.paint();
  CHECK_MESSAGE(text::contains(welcome, text::concat("Welcome to ", meta::app_name)),
                "the first start shows no dialog:\n", welcome);
  seen.emplace_back("First-start dialog");
  board.answer(false);
  board.show(service::nav_overview);
  const std::string empty = window.paint();
  CHECK_MESSAGE(!text::contains(empty, "Welcome to"), "the dialog stays open:\n", empty);
  seen.push_back(empty);
  board.load_demo();
  for (const service::NavItem& item : service::navigation) {
    board.show(item.id);
    seen.push_back(window.paint());
  }
  board.show(service::nav_records);
  REQUIRE(!board.rec.list.empty());
  const std::int64_t first_id = board.rec.list.front().id;
  const std::string first_name = board.rec.list.front().name;
  for (const std::string_view wanted : {"Disable", "Enable"}) {
    board.rec.selected = first_id;
    const std::string detail = window.paint();
    const std::vector<std::string> lines = text::split(detail, '\n');
    CHECK_MESSAGE(std::ranges::count(lines, first_name) >= 2, "no detail panel for ", first_name, ":\n", detail);
    CHECK_MESSAGE(shown_line(detail, wanted), "the detail panel does not offer ", wanted, ":\n", detail);
    seen.emplace_back("row detail panel");
    seen.push_back(detail);
    board.toggle_record();
  }
  board.show(service::nav_notifications);
  board.inbox.rules = true;
  seen.push_back(window.paint());
  board.inbox.rules = false;
  for (const AboutTab tab :
       {AboutTab::Overview, AboutTab::Guide, AboutTab::Shortcuts, AboutTab::Diagnostics, AboutTab::Licenses}) {
    board.show_about(tab);
    seen.push_back(window.paint());
  }
  board.show(service::nav_overview);
  static_cast<void>(window.paint(2));
  CHECK_MESSAGE(window.click("Menu"), "the menu bar has no Menu:\n", window.text());
  CHECK_MESSAGE(window.view().find(SeenKind::Choice, "Menu > Open terminal UI") != nullptr, window.text());
  seen.push_back(window.text());
  const std::string everything = text::join(seen, "\n");
  std::vector<std::string> missing;
  for (const service::Capability& capability : service::capabilities()) {
    if (capability.dashboard == service::not_applicable) {
      continue;
    }
    for (const std::string& piece : text::split(capability.dashboard, ',')) {
      const std::string_view part = text::trim(piece);
      if (!text::contains(everything, part)) {
        missing.push_back(text::concat(capability.id, ": ", part));
      }
    }
  }
  CHECK_MESSAGE(missing.empty(), "not found in the dashboard: ", text::join(missing, "; "), "\n", everything);
}

TEST_CASE("the overview draws a tally beside the table with the same numbers") {
  const TempDir home("gui-overview-tally");
  service::App app = open_app(home);
  Dashboard board = started(app, options(home));
  Window window(board, shots::width, shots::height);
  board.show(service::nav_overview);
  const std::string empty = window.paint();
  CHECK_MESSAGE(text::contains(empty, "No records yet."), "an empty database is not explained:\n", empty);
  CHECK_MESSAGE(!text::contains(empty, "One square is"), "an empty database shows a tally:\n", empty);
  board.load_demo();
  const Picture picture = window.picture();
  const std::string shown = window.text();
  for (const std::string_view wanted :
       {"Enabled records by severity and status", "The same as a table", "One square is one enabled record."}) {
    CHECK_MESSAGE(shown_line(shown, wanted), "the overview lacks \"", wanted, "\":\n", shown);
  }

  const std::array<Seen, 4> titles = {window.leftmost("Open records"), window.leftmost("Urgent records"),
                                      window.leftmost("Unread notifications"), window.leftmost("Disabled records")};
  const float line = center_y(titles[0]);
  for (const Seen& title : titles) {
    CHECK_MESSAGE(std::abs(center_y(title) - line) < 2.0F, "the figures are not on one line");
  }
  const float first = titles[1].min.x - titles[0].min.x;
  const float second = titles[2].min.x - titles[1].min.x;
  const float third = titles[3].min.x - titles[2].min.x;
  CHECK_MESSAGE(std::abs(second - third) < 2.0F, "the later figures differ in width: ", second, " and ", third);
  CHECK_MESSAGE((first > second * 1.3F && first < second * 1.5F), "the first figure is not the widest: ", first,
                " and ", second);

  const theme::Tokens tokens = board.tokens;
  const float square = tokens.spacing.m;
  const float table = window.leftmost("Status").min.x;
  const theme::Color faded = theme::mix(tokens.semantic.severity(Severity::Medium), tokens.palette.surface, 0.6);
  struct Row {
    Severity level;
    float strong;
    float pale;
  };
  const std::array<Row, 5> rows = {{
      {Severity::Critical, 1.0F, 0.0F},
      {Severity::High, 1.0F, 0.0F},
      {Severity::Medium, 1.0F, 1.0F},
      {Severity::Low, 1.0F, 0.0F},
      {Severity::Info, 0.0F, 0.0F},
  }};
  float above = line;
  for (const Row& row : rows) {
    const std::string_view name = theme::label(row.level);
    const Seen label = window.leftmost(name);
    CHECK_MESSAGE(label.max.x < table, name, " is not a row of the tally");
    CHECK_MESSAGE(center_y(label) > above, name, " is out of order");
    above = center_y(label);
    const auto y = static_cast<std::size_t>(std::lround(center_y(label)));
    const auto from = static_cast<std::size_t>(label.max.x + 4.0F);
    const auto to = static_cast<std::size_t>(table - square * 2.0F);
    const std::array<std::pair<theme::Color, float>, 2> wanted = {{
        {tokens.semantic.severity(row.level), row.strong},
        {faded, row.pale},
    }};
    for (const auto& [tint, share] : wanted) {
      const auto drawn = static_cast<float>(tinted(picture, y, from, to, tint));
      const float least = share * (square - 2.0F);
      const float most = share * (square + 1.0F);
      CHECK_MESSAGE((drawn >= least && drawn <= most), name, ": ", drawn, " pixels, wanted ", least, " to ", most);
    }
  }
  CHECK_MESSAGE(window.all("Medium").size() == 3, "the table does not repeat the tally:\n", shown);
  CHECK_MESSAGE(window.all("Info").size() == 1, "a disabled record is counted:\n", shown);
}

TEST_CASE("the top bar searches and its chip, bell and help lead somewhere") {
  const TempDir home("gui-top-bar");
  service::App app = open_app(home);
  Dashboard board = started(app, options(home));
  Window window(board, shots::width, 2000.0F);
  const std::string empty = window.paint();
  CHECK_MESSAGE(!shown_line(empty, "Demo data"), "an empty database shows the demo chip:\n", empty);
  CHECK(board.bell_label() == "Notifications: nothing unread");
  window.click_at(window.rightmost("Help").middle());
  CHECK(board.page_id() == service::nav_about);
  CHECK(board.about.tab == AboutTab::Guide);
  board.show(service::nav_overview);
  const std::string overview = window.paint();
  const std::vector<std::string> lines = text::split(overview, '\n');
  const auto load =
      std::ranges::find_if(lines, [](const std::string& line) { return line.starts_with("Load demo data"); });
  REQUIRE_MESSAGE(load != lines.end(), "no button loads demo data:\n", overview);
  CHECK(window.click(*load));
  CHECK(board.overview.demo == 6);
  const std::string loaded = window.paint();
  CHECK_MESSAGE(shown_line(loaded, "Demo data"), "the demo chip is missing while demo data is loaded:\n", loaded);
  CHECK(board.bell_label() == "Notifications: 2 unread");
  const Seen help = window.rightmost("Help");
  const Seen chip = window.leftmost("Demo data");
  const std::vector<Seen> counts = window.all("2");
  const auto bell = std::ranges::find_if(counts, [&](const Seen& item) {
    return std::abs(center_y(item) - center_y(help)) < 12.0F && item.min.x > chip.max.x && item.max.x < help.min.x;
  });
  REQUIRE_MESSAGE(bell != counts.end(), "the bell shows no unread count between the chip and Help:\n", loaded);
  const std::vector<std::string> hints = window.hints();
  CHECK_MESSAGE(std::ranges::find(hints, board.bell_label()) != hints.end(), "the bell has no hint");
  window.click_at(bell->middle());
  CHECK(board.page_id() == service::nav_notifications);
  CHECK(window.click("Demo data"));
  CHECK(board.page_id() == service::nav_settings);
  shows(board, {demo_note});
  CHECK(shown_line(window.paint(), "Remove demo data"));
  search_from_the_top_bar(window, board, "   ");
  shows(board, {"Type one or more words"});
  search_from_the_top_bar(window, board, "demo");
  CHECK(board.page_id() == service::nav_records);
  CHECK(board.rec.query.search == "demo");
  CHECK(board.rec.list.size() == 6);
  shows(board, {"6 records and"});
  search_from_the_top_bar(window, board, "quiet hours");
  CHECK(board.page_id() == service::nav_about);
  CHECK(board.about.tab == AboutTab::Guide);
  CHECK(board.about.search == "quiet hours");
  search_from_the_top_bar(window, board, "zzzqqq");
  shows(board, {"Nothing matches \"zzzqqq\""});
}

TEST_CASE("a collapsed sidebar explains its icons on hover") {
  const TempDir home("gui-hints");
  service::App app = open_app(home);
  Dashboard board = started(app, options(home));
  board.show(service::nav_records);
  Window window(board, shots::width, shots::height);
  const std::string expanded = window.paint();
  CHECK_MESSAGE(shown_line(expanded, "Overview"), expanded);
  CHECK_MESSAGE(shown_line(expanded, "Collapse"), expanded);
  const Seen label = window.leftmost("Overview");
  CHECK(window.click("Collapse"));
  CHECK(board.collapsed);
  const std::string collapsed = window.paint();
  CHECK_MESSAGE(!shown_line(collapsed, "Collapse"), collapsed);
  const Seen* button = window.view().find(SeenKind::Button, "Overview");
  REQUIRE_MESSAGE(button != nullptr, "the collapsed sidebar has no Overview button:\n", collapsed);
  CHECK_MESSAGE(button->max.x - button->min.x < 64.0F, "the collapsed button is as wide as its label");
  const std::vector<std::string> hints = window.hints();
  const auto hint = std::ranges::find_if(hints, [](const std::string& line) { return line.starts_with("Overview: "); });
  CHECK_MESSAGE(hint != hints.end(), "no hint explains the Overview icon");
  const ImVec2 icon(button->middle().x, center_y(label));
  const Picture plain = window.picture();
  window.hover(icon, 1.0);
  const Picture hovered = window.stage().picture();
  std::size_t right_of_icon = 0;
  for (std::size_t x = static_cast<std::size_t>(button->max.x) + 4; x < 400; ++x) {
    for (std::size_t y = static_cast<std::size_t>(icon.y) - 4; y < static_cast<std::size_t>(icon.y) + 40; ++y) {
      if (plain.at(x, y) != hovered.at(x, y)) {
        right_of_icon += 1;
      }
    }
  }
  CHECK_MESSAGE(right_of_icon > 200, "hovering shows no hint beside the icon: ", right_of_icon, " pixels changed");
  window.hover(ImVec2(600.0F, 400.0F), 1.0);
  window.click_at(icon);
  CHECK(board.page_id() == service::nav_overview);
}

TEST_CASE("record table columns and sort are saved and follow other windows") {
  const TempDir home("gui-columns");
  service::App app = open_app(home);
  const service::App other = open_app(home);
  Dashboard board = started(app, options(home));
  for (const std::string_view name : {"Alpha", "Bravo"}) {
    records::Input made = input(name, Severity::Low);
    made.note = text::concat("rack ", name);
    static_cast<void>(other.records().create(made));
  }
  board.show(service::nav_records);
  board.tick();
  Window window(board, shots::width, shots::height);
  const std::string all = window.paint();
  for (const std::string_view wanted : {"ID", "Name", "Columns", "2 records shown"}) {
    CHECK_MESSAGE(shown_line(all, wanted), "the records page lacks \"", wanted, "\":\n", all);
  }
  CHECK(board.rec.query.sort == "updated");
  CHECK(board.rec.query.descending);
  const std::vector<std::string> every(settings::record_columns().begin(), settings::record_columns().end());
  CHECK(board.rec.columns == every);
  CHECK(window.click("Columns"));
  shows(board, {"Columns of the record table", "Severity", "Updated"});
  REQUIRE(board.form.has_value());
  const bool pinned = std::ranges::any_of(board.form->fields, [](const Field& field) { return field.key == "name"; });
  CHECK_MESSAGE(!pinned, "the name column can be hidden");
  fill(board, {{"severity", "hide"}, {"id", "hide"}});
  board.submit();
  shows(board, {"Saved the columns of the record table"});
  CHECK(other.settings().get(settings::key_record_columns).raw == "name,status,updated,state");
  const std::vector<std::string> kept = {"name", "status", "updated", "state"};
  CHECK(board.rec.columns == kept);
  const std::string fewer = window.paint();
  CHECK_MESSAGE(shown_line(fewer, "Name"), fewer);
  CHECK_MESSAGE(!shown_line(fewer, "ID"), "a hidden column is still painted:\n", fewer);
  board.sort_by("name", false);
  CHECK(other.settings().get(settings::key_record_sort).raw == "name:asc");
  CHECK(board.rec.list.front().name == "Alpha");
  static_cast<void>(other.settings().set(settings::key_record_sort, "name:desc"));
  static_cast<void>(other.settings().reset(settings::key_record_columns));
  board.tick();
  shows(board, {"Updated by another window"});
  CHECK(board.rec.list.front().name == "Bravo");
  CHECK(board.rec.query.sort == "name");
  CHECK(board.rec.query.descending);
  CHECK(board.rec.columns == every);
  const std::string again = window.paint();
  CHECK_MESSAGE(shown_line(again, "ID"), "the column stayed hidden after a reset in another window:\n", again);
}

TEST_CASE("the chooser offers both front ends and marks the default") {
  Asked asked(app::launcher::mode_gui);
  const std::string shown = asked.lines();
  for (const std::string_view wanted : {std::string_view(chooser::dashboard), chooser::terminal,
                                        chooser::remember_label, std::string_view("Quit"), chooser::later}) {
    CHECK_MESSAGE(shown_line(shown, wanted), "the chooser lacks \"", wanted, "\":\n", shown);
  }
  CHECK_MESSAGE(shown_line(shown, chooser::title()), shown);
  CHECK(chooser::title() == text::concat("How do you want to open ", meta::app_name, "?"));
  CHECK(chooser::dashboard == "Dashboard (graphical window)");
  CHECK(chooser::terminal == "Terminal (text interface)");
  CHECK_MESSAGE(asked.marked(chooser::dashboard), "the default choice is not marked");
  CHECK_MESSAGE(!asked.marked(chooser::terminal), "both choices are marked");
  const chooser::Choice terminal{chooser::ChoiceKind::Mode, app::launcher::mode_tui};
  const chooser::Choice dashboard{chooser::ChoiceKind::Mode, app::launcher::mode_gui};
  CHECK(asked.click(chooser::terminal) == terminal);
  CHECK(!asked.remember);
  CHECK(asked.click(chooser::remember_label).kind == chooser::ChoiceKind::Waiting);
  CHECK_MESSAGE(asked.remember, "the checkbox did not react");
  CHECK(asked.click(chooser::dashboard) == dashboard);
  CHECK(asked.click("Quit").kind == chooser::ChoiceKind::Quit);
  const std::vector<std::string> only_check = {"check"};
  CHECK_MESSAGE(asked.drawn() == only_check, "the chooser draws more than the application icon and the check mark");
  Asked other(app::launcher::mode_tui);
  CHECK_MESSAGE(other.marked(chooser::terminal), "the terminal default is not marked");
  CHECK(!other.marked(chooser::dashboard));
  const Picture dark = shots::chooser(true);
  const Picture light = shots::chooser(false);
  CHECK_MESSAGE(dark.at(4, 4) != light.at(4, 4), "the chooser ignores the system's dark mode");
}

TEST_CASE("every icon the dashboard draws comes from the application set") {
  const TempDir home("gui-icons");
  service::App app = open_app(home);
  Dashboard board = started(app, options(home));
  board.load_demo();
  Window window(board, 1280.0F, 2600.0F);
  for (const service::NavItem& item : service::navigation) {
    board.show(item.id);
    static_cast<void>(window.paint());
  }
  board.show(service::nav_records);
  board.rec.selected = board.rec.list.front().id;
  static_cast<void>(window.paint());
  board.show(service::nav_notifications);
  board.inbox.rules = true;
  static_cast<void>(window.paint());
  for (const AboutTab tab : {AboutTab::Overview, AboutTab::Guide, AboutTab::Shortcuts, AboutTab::Diagnostics}) {
    board.show_about(tab);
    static_cast<void>(window.paint());
  }
  board.toggle_sidebar();
  static_cast<void>(window.paint());
  const std::vector<std::string> drawn = window.view().icons().drawn();
  CHECK_MESSAGE(drawn.size() >= 12, "only ", drawn.size(), " icons were drawn");
  for (const std::string& name : drawn) {
    CHECK_MESSAGE(known_icon(name), name, " is drawn but is not in the application set");
  }
  for (const service::NavItem& item : service::navigation) {
    const std::string_view name = nav_icon(item.id);
    const bool found = std::ranges::find(drawn, name) != drawn.end();
    CHECK_MESSAGE(found, "the icon ", name, " of ", item.id, " was never drawn");
  }
}
