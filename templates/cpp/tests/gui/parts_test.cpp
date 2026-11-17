#include <doctest/doctest.h>

#include <imgui.h>

#include <algorithm>
#include <array>
#include <cmath>
#include <set>
#include <string>
#include <string_view>
#include <vector>

#include "core/error.hpp"
#include "core/meta.hpp"
#include "core/platform.hpp"
#include "core/records.hpp"
#include "core/text.hpp"
#include "core/theme.hpp"
#include "gui/appearance.hpp"
#include "gui/forms.hpp"
#include "gui/icons.hpp"
#include "gui/ledger.hpp"
#include "gui/render.hpp"
#include "gui/skin.hpp"
#include "gui/table.hpp"
#include "support/testkit.hpp"

using namespace app::gui;
namespace text = app::text;
namespace theme = app::theme;
using app::records::Breakdown;
using app::records::Status;
using app::theme::Severity;

namespace {

std::uint8_t share(const std::vector<std::uint8_t>& coverage, std::size_t size, std::size_t x, std::size_t y) {
  return coverage[y * size + x];
}

theme::Tokens tokens_of(std::string_view name, theme::Mode mode) {
  theme::Options wanted;
  wanted.name = std::string(name);
  wanted.mode = mode;
  wanted.density = "comfortable";
  wanted.text_scale = 100;
  return theme::resolve(wanted);
}

theme::Color ink(const ImGuiStyle& look, ImGuiCol slot) {
  return unpaint(look.Colors[slot]);
}

bool rejected(std::string_view data) {
  return !testkit::failure_of([&] { static_cast<void>(outlines(data)); }).empty();
}

}

TEST_CASE("fields keep values, labels and limits") {
  Form form("Example", "Save",
            {
                Field::text("name", "Name", "first", "Help.", 20),
                Field::area("note", "Note", "", "", 100),
                Field::choice("mode", "Mode", "b", "", {{"a", "Alpha"}, {"b", "Beta"}}),
                Field::yes_no("enabled", "Enabled", true, ""),
            },
            Submit{});
  CHECK(form.value("name") == "first");
  CHECK(form.fields[2].shown() == "Beta");
  CHECK(form.fields[3].shown() == "Yes");
  CHECK(form.fields[1].multiline);
  CHECK(!form.fields[0].multiline);
  form.set("mode", "a");
  form.set("missing", "ignored");
  CHECK(form.value("mode") == "a");
  CHECK(form.value("missing").empty());
  CHECK(form.fresh);
  CHECK(form.problem.empty());
  const Confirm confirm("Title", "Statement", "Go", std::nullopt);
  CHECK(confirm.cancel == "Cancel");
  CHECK(!confirm.danger);
}

TEST_CASE("every icon parses into closed outlines inside the view box") {
  for (const IconPath& icon : icon_paths) {
    const std::vector<Outline> shapes = outlines(icon.data);
    CHECK_MESSAGE(!shapes.empty(), icon.name, " has no outline");
    for (const Outline& shape : shapes) {
      CHECK_MESSAGE(shape.size() >= 3, icon.name, " has an open outline");
      for (const Point& point : shape) {
        const bool inside = point.x >= 0.0F && point.x <= icon_view && point.y >= 0.0F && point.y <= icon_view;
        CHECK_MESSAGE(inside, icon.name, " leaves the view box at ", point.x, ",", point.y);
      }
    }
    const std::vector<std::uint8_t> coverage = mask(shapes, 24);
    const auto filled = std::ranges::count_if(coverage, [](std::uint8_t value) { return value > 0; });
    CHECK_MESSAGE((filled > 24 && filled < 24 * 24), icon.name, " fills ", filled, " pixels");
  }
}

TEST_CASE("inner outlines cut holes") {
  const std::vector<std::uint8_t> frame = mask(outlines(icon_path("sidebar")), 24);
  CHECK(share(frame, 24, 3, 12) == 255);
  CHECK(share(frame, 24, 6, 12) == 0);
  CHECK(share(frame, 24, 14, 12) == 255);
  CHECK(share(frame, 24, 1, 1) == 0);
  const std::vector<std::uint8_t> ring = mask(outlines(icon_path("status-open")), 24);
  CHECK(share(ring, 24, 12, 12) == 0);
  CHECK(share(ring, 24, 12, 3) == 255);
  const std::vector<std::uint8_t> done = mask(outlines(icon_path("status-done")), 24);
  CHECK(share(done, 24, 12, 4) == 255);
}

TEST_CASE("arcs follow the circle") {
  const std::vector<Outline> shapes = outlines("M12 2a10 10 0 1 1 0 20a10 10 0 1 1 0-20z");
  REQUIRE(shapes.size() == 1);
  for (const Point& point : shapes[0]) {
    const float distance = std::hypot(point.x - 12.0F, point.y - 12.0F);
    CHECK_MESSAGE(std::abs(distance - 10.0F) < 0.01F, point.x, ",", point.y, " is ", distance, " from the center");
  }
  CHECK(std::ranges::any_of(shapes[0], [](const Point& point) { return point.x > 21.9F; }));
  CHECK(std::ranges::any_of(shapes[0], [](const Point& point) { return point.x < 2.1F; }));
}

TEST_CASE("numbers may touch and commands repeat") {
  const std::vector<Outline> touching = outlines("M1-2.5.5-3l3,4 1 1z");
  REQUIRE(touching.size() == 1);
  const Outline wanted = {{1.0F, -2.5F}, {0.5F, -3.0F}, {3.5F, 1.0F}, {4.5F, 2.0F}};
  CHECK(touching[0] == wanted);
  const std::vector<Outline> shapes = outlines("M2 2 10 2 10 10z m1 1h2v2z");
  REQUIRE(shapes.size() == 2);
  const Outline first = {{2.0F, 2.0F}, {10.0F, 2.0F}, {10.0F, 10.0F}};
  const Outline second = {{3.0F, 3.0F}, {5.0F, 3.0F}, {5.0F, 5.0F}};
  CHECK(shapes[0] == first);
  CHECK(shapes[1] == second);
}

TEST_CASE("broken paths are rejected and unknown names fall back") {
  for (const std::string_view broken : {"M1", "C1 2 3 4 5 6", "M1 2 L3", "5 5", "M1 2 # 3"}) {
    CHECK_MESSAGE(rejected(broken), broken);
  }
  CHECK(icon_path("no-such-icon") == icon_path(fallback_icon));
  CHECK(!known_icon("no-such-icon"));
  CHECK(known_icon(fallback_icon));
  CHECK(outlines("").empty());
}

TEST_CASE("the unit grows in round steps until the largest cell fits") {
  CHECK(ledger::unit(0, 7) == 1);
  CHECK(ledger::unit(7, 7) == 1);
  CHECK(ledger::unit(8, 7) == 2);
  CHECK(ledger::unit(15, 7) == 5);
  CHECK(ledger::unit(36, 7) == 10);
  CHECK(ledger::unit(700, 7) == 100);
  CHECK(ledger::unit(701, 7) == 200);
  CHECK(ledger::unit(70'000, 7) == 10'000);
  CHECK(ledger::unit(70'001, 7) == 10'001);
  CHECK(ledger::unit(12, 0) == 20);
  CHECK(ledger::unit(-3, 7) == 1);
  for (const std::int64_t largest : {1, 9, 99, 12'345, 987'654}) {
    for (const std::int64_t capacity : {1, 3, 7, 12}) {
      const std::int64_t chosen = ledger::unit(largest, capacity);
      CHECK_MESSAGE(ledger::squares(largest, chosen) <= capacity, largest, " in ", capacity, ": unit ", chosen);
      CHECK(ledger::squares(largest, chosen) >= 1);
    }
  }
}

TEST_CASE("squares round up so a small count never disappears") {
  CHECK(ledger::squares(0, 5) == 0);
  CHECK(ledger::squares(1, 5) == 1);
  CHECK(ledger::squares(5, 5) == 1);
  CHECK(ledger::squares(6, 5) == 2);
  CHECK(ledger::squares(3, 0) == 3);
}

TEST_CASE("the caption names the unit") {
  CHECK(ledger::caption(1) == "One square is one enabled record.");
  CHECK(ledger::caption(0) == "One square is one enabled record.");
  CHECK(ledger::caption(20) == "One square is 20 enabled records, rounded up.");
}

TEST_CASE("counts add up per cell and the largest cell sets the scale") {
  const std::vector<Breakdown> rows = {
      {Status::Open, Severity::High, 2, 0},
      {Status::Open, Severity::High, 3, 0},
      {Status::Done, Severity::Low, 4, 0},
      {Status::InProgress, Severity::Critical, 1, 0},
  };
  CHECK(ledger::count(rows, Status::Open, Severity::High) == 5);
  CHECK(ledger::count(rows, Status::Done, Severity::Low) == 4);
  CHECK(ledger::count(rows, Status::Done, Severity::High) == 0);
  CHECK(ledger::largest(rows) == 5);
  CHECK(ledger::largest({}) == 0);
}

TEST_CASE("flexible columns share what the fixed ones leave") {
  const std::vector<table::Spec> specs = {table::fixed(60.0F), table::flexible(2.0F, 100.0F),
                                          table::flexible(1.0F, 100.0F), table::fixed(80.0F)};
  const std::vector<table::Slot> slots = table::layout(specs, 700.0F, 10.0F);
  const std::vector<table::Slot> wanted = {{0.0F, 60.0F}, {70.0F, 320.0F}, {400.0F, 210.0F}, {620.0F, 80.0F}};
  CHECK(slots == wanted);
  CHECK(slots[3].left + slots[3].width == 700.0F);
}

TEST_CASE("hidden columns leave no gap and narrow windows keep the least width") {
  const std::vector<table::Spec> three = {table::fixed(60.0F), table::flexible(1.0F, 100.0F), table::fixed(80.0F)};
  const std::vector<table::Spec> two = {table::fixed(60.0F), table::flexible(1.0F, 100.0F)};
  const std::vector<table::Slot> all = table::layout(three, 500.0F, 10.0F);
  const std::vector<table::Slot> fewer = table::layout(two, 500.0F, 10.0F);
  CHECK(all[1].width == 340.0F);
  const table::Slot wide{70.0F, 430.0F};
  CHECK(fewer[1] == wide);
  const std::vector<table::Slot> narrow = table::layout(three, 120.0F, 10.0F);
  CHECK(narrow[1].width == 100.0F);
  CHECK(narrow[2].left == 180.0F);
  CHECK(table::layout({}, 100.0F, 10.0F).empty());
  CHECK(table::needed(three, 10.0F) == 280.0F);
  const std::vector<table::Spec> rigid = {table::fixed(40.0F), table::fixed(50.0F)};
  const std::vector<table::Slot> packed = {{0.0F, 40.0F}, {50.0F, 50.0F}};
  CHECK(table::layout(rigid, 300.0F, 10.0F) == packed);
}

TEST_CASE("every theme replaces the default look and keeps text readable") {
  ImGuiStyle stock_dark;
  ImGui::StyleColorsDark(&stock_dark);
  ImGuiStyle stock_light;
  ImGui::StyleColorsLight(&stock_light);
  const std::array<theme::Color, 2> stock = {ink(stock_dark, ImGuiCol_WindowBg), ink(stock_light, ImGuiCol_WindowBg)};
  std::set<std::array<std::uint8_t, 3>> fills;
  for (const std::string& name : theme::names()) {
    for (const theme::Mode mode : {theme::Mode::Light, theme::Mode::Dark}) {
      const theme::Tokens tokens = tokens_of(name, mode);
      const ImGuiStyle look = style(tokens);
      const theme::Color window = ink(look, ImGuiCol_WindowBg);
      const std::string where = text::concat(name, tokens.dark ? " dark" : " light");
      CHECK_MESSAGE(window == tokens.palette.background, where);
      const bool kept = std::ranges::find(stock, window) != stock.end();
      CHECK_MESSAGE((!kept || name == "contrast"), where, " keeps a stock fill");
      fills.insert({window.r, window.g, window.b});
      const std::array<std::pair<ImGuiCol, ImGuiCol>, 7> pairs = {{
          {ImGuiCol_Text, ImGuiCol_WindowBg},
          {ImGuiCol_Text, ImGuiCol_PopupBg},
          {ImGuiCol_Text, ImGuiCol_FrameBg},
          {ImGuiCol_Text, ImGuiCol_Button},
          {ImGuiCol_Text, ImGuiCol_ButtonHovered},
          {ImGuiCol_Text, ImGuiCol_ButtonActive},
          {ImGuiCol_Text, ImGuiCol_Header},
      }};
      for (const auto& [front, behind] : pairs) {
        const double ratio = theme::contrast(ink(look, front), ink(look, behind));
        CHECK_MESSAGE(ratio >= theme::min_text_contrast, where, ": slot ", front, " on ", behind, " is ", ratio);
      }
      const double muted = theme::contrast(ink(look, ImGuiCol_TextDisabled), window);
      CHECK_MESSAGE(muted >= theme::min_text_contrast, where, ": muted text ", muted);
      const double ring = theme::contrast(ink(look, ImGuiCol_NavCursor), ink(look, ImGuiCol_PopupBg));
      CHECK_MESSAGE(ring >= theme::min_component_contrast, where, ": focus ring ", ring);
    }
  }
  CHECK_MESSAGE(fills.size() >= 7, "only ", fills.size(), " distinct backgrounds");
}

TEST_CASE("text scale and density reach the style") {
  const theme::Tokens base = tokens_of("slate", theme::Mode::Light);
  theme::Options wanted;
  wanted.name = "slate";
  wanted.mode = theme::Mode::Light;
  wanted.density = "compact";
  wanted.text_scale = 150;
  wanted.reduced_motion = true;
  const theme::Tokens changed = theme::resolve(wanted);
  const ImGuiStyle plain = style(base);
  const ImGuiStyle large = style(changed);
  CHECK(large.FontSizeBase > plain.FontSizeBase);
  CHECK(plain.FontSizeBase == base.type_scale.body);
  CHECK(changed.type_scale.heading > changed.type_scale.subheading);
  CHECK(base.type_scale.heading < base.type_scale.figure);
  CHECK(!changed.motion);
  CHECK(base.motion);
  wanted.density = "comfortable";
  wanted.text_scale = 100;
  const ImGuiStyle roomy = style(theme::resolve(wanted));
  wanted.density = "compact";
  const ImGuiStyle tight = style(theme::resolve(wanted));
  CHECK(tight.FramePadding.y < roomy.FramePadding.y);
  const theme::Tokens doubled = scaled(base, 2.0F);
  CHECK(doubled.type_scale.body == base.type_scale.body * 2.0F);
  CHECK(doubled.spacing.m == base.spacing.m * 2.0F);
  CHECK(doubled.palette.accent == base.palette.accent);
}

TEST_CASE("the embedded faces are installed and the notices reproduce their license") {
  CHECK(regular_bytes().size() > 100'000);
  CHECK(medium_bytes().size() > 100'000);
  CHECK(regular_bytes() != medium_bytes());
  const std::string_view license = font_license();
  CHECK(text::contains(license, "SIL OPEN FONT LICENSE Version 1.1"));
  CHECK(text::contains(license, "The Noto Project Authors"));
  const std::string_view notices = app::meta::notices_text();
  CHECK_MESSAGE(text::contains(notices, "## Fonts embedded by the application"), "the notices do not name the fonts");
  CHECK_MESSAGE(text::contains(notices, text::trim(license)), "the notices do not reproduce the font license");
  const Stage stage(200.0F, 100.0F, 1.0F);
  const Faces faces = install_fonts();
  REQUIRE(faces.regular != nullptr);
  REQUIRE(faces.medium != nullptr);
  REQUIRE(faces.mono != nullptr);
  CHECK(faces.regular != faces.medium);
  CHECK(faces.mono != faces.regular);
  CHECK(ImGui::GetIO().FontDefault == faces.regular);
}

TEST_CASE("severity and status icons exist and differ") {
  std::set<std::string_view> names;
  for (const Severity level : theme::all_severities) {
    const std::string_view name = severity_icon(level);
    CHECK_MESSAGE(known_icon(name), name);
    names.insert(name);
  }
  for (const Status status : app::records::all_statuses) {
    const std::string_view name = status_icon(status);
    CHECK_MESSAGE(known_icon(name), name);
    names.insert(name);
  }
  CHECK(names.size() == theme::all_severities.size() + app::records::all_statuses.size());
}

TEST_CASE("a filled rectangle lands on the expected pixels") {
  Stage stage(40.0F, 30.0F, 1.0F);
  const Picture picture = stage.settle(2, [] {
    ImGui::GetBackgroundDrawList()->AddRectFilled(ImVec2(10.0F, 5.0F), ImVec2(20.0F, 15.0F),
                                                  IM_COL32(200, 40, 10, 255));
  });
  CHECK(picture.width == 40);
  CHECK(picture.height == 30);
  const std::array<std::uint8_t, 4> red = {200, 40, 10, 255};
  CHECK(picture.at(10, 5) == red);
  CHECK(picture.at(19, 14) == red);
  CHECK(picture.at(15, 10) == red);
  CHECK(picture.at(9, 10) != red);
  CHECK(picture.at(20, 10) != red);
  CHECK(picture.at(15, 4) != red);
  CHECK(picture.at(15, 15) != red);
  CHECK(picture.at(0, 0) == picture.at(39, 29));
}

TEST_CASE("a translucent fill mixes with the background once and shared edges are painted once") {
  Stage stage(40.0F, 30.0F, 1.0F);
  const Picture picture = stage.settle(2, [] {
    ImDrawList* canvas = ImGui::GetBackgroundDrawList();
    canvas->Flags &= ~ImDrawListFlags_AntiAliasedFill;
    canvas->AddRectFilled(ImVec2(0.0F, 0.0F), ImVec2(40.0F, 30.0F), IM_COL32(0, 0, 0, 255));
    canvas->AddRectFilled(ImVec2(4.0F, 4.0F), ImVec2(20.0F, 20.0F), IM_COL32(255, 255, 255, 128));
    canvas->AddRectFilled(ImVec2(20.0F, 4.0F), ImVec2(36.0F, 20.0F), IM_COL32(255, 255, 255, 128));
  });
  const std::array<std::uint8_t, 4> middle = picture.at(10, 10);
  CHECK(std::abs(int{middle[0]} - 128) <= 1);
  CHECK(middle[0] == middle[1]);
  CHECK(middle[1] == middle[2]);
  for (std::size_t x = 4; x < 36; ++x) {
    CHECK_MESSAGE(picture.at(x, 10) == middle, "column ", x, " differs");
    CHECK_MESSAGE(picture.at(x, 6) == middle, "the diagonal of the rectangle shows at column ", x);
  }
  const std::array<std::uint8_t, 4> black = {0, 0, 0, 255};
  CHECK(picture.at(3, 10) == black);
  CHECK(picture.at(36, 10) == black);
}

TEST_CASE("the scale multiplies the picture and text leaves ink") {
  Stage stage(60.0F, 20.0F, 2.0F);
  const Faces faces = install_fonts();
  const Picture picture = stage.settle(2, [&] {
    ImDrawList* canvas = ImGui::GetBackgroundDrawList();
    canvas->AddRectFilled(ImVec2(0.0F, 0.0F), ImVec2(60.0F, 20.0F), IM_COL32(255, 255, 255, 255));
    canvas->AddText(faces.regular, 14.0F, ImVec2(4.0F, 2.0F), IM_COL32(0, 0, 0, 255), "Ink");
  });
  CHECK(picture.width == 120);
  CHECK(picture.height == 40);
  std::size_t dark = 0;
  for (std::size_t offset = 0; offset + 3 < picture.pixels.size(); offset += 4) {
    if (picture.pixels[offset] < 96) {
      dark += 1;
    }
  }
  CHECK_MESSAGE(dark > 40, "only ", dark, " dark pixels");
  const std::array<std::uint8_t, 4> white = {255, 255, 255, 255};
  CHECK(picture.at(118, 38) == white);
}

TEST_CASE("the system appearance is read from the theme name and the color scheme") {
  CHECK(scheme_is_dark("'prefer-dark'\n"));
  CHECK(scheme_is_dark("Dark"));
  CHECK(!scheme_is_dark("'default'"));
  CHECK(!scheme_is_dark("'prefer-light'"));
  CHECK(!scheme_is_dark(""));
  CHECK(theme_is_dark("Adwaita:dark"));
  CHECK(theme_is_dark(" Adwaita-dark "));
  CHECK(!theme_is_dark("Adwaita"));
  CHECK(!theme_is_dark("darkness:light"));
}
