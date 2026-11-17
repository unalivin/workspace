#include "guikit/guikit.hpp"

#include <doctest/doctest.h>

#include <algorithm>
#include <cstdlib>
#include <set>

#include "core/clock.hpp"
#include "core/files.hpp"
#include "core/notifier.hpp"
#include "core/text.hpp"

namespace guikit {

Log new_log() {
  return std::make_shared<std::vector<std::string>>();
}

app::platform::Env environment(const testkit::TempDir& home) {
  return app::platform::Env::from_pairs({{"HOME", app::files::shown(home.path())}});
}

app::service::App open_app(const testkit::TempDir& home) {
  app::service::Options opening;
  opening.os = app::platform::Os::Linux;
  opening.env = environment(home);
  opening.clock = app::clock::Clock::fixed(testkit::fixed_time());
  opening.zone = app::clock::Zone::utc();
  return app::service::App::open(std::move(opening));
}

app::gui::Options options(const testkit::TempDir& home) {
  app::gui::Options made;
  made.env = environment(home);
  return made;
}

app::gui::Options wired(const testkit::TempDir& home, const Log& opened, const Log& terminals) {
  app::gui::Options made = options(home);
  app::notifier::Autostart entry;
  entry.os = app::platform::Os::Linux;
  entry.home_dir = app::files::shown(home.path());
  entry.executable = "/opt/app/templateapp";
  made.autostart = entry;
  made.open_terminal = [terminals](const std::vector<std::string>& arguments) {
    terminals->push_back(app::text::join(arguments, " "));
  };
  made.open_url = [opened](std::string_view url) { opened->emplace_back(url); };
  return made;
}

std::string said(const app::gui::Dashboard& board) {
  std::string text = board.notice ? board.notice->text : std::string();
  if (board.form) {
    text = app::text::concat(text, "\n", board.form->title, "\n", board.form->problem);
    for (const app::gui::Field& field : board.form->fields) {
      text = app::text::concat(text, "\n", field.label, "\n", field.help);
    }
  }
  if (board.confirm) {
    text = app::text::concat(text, "\n", board.confirm->title, "\n", board.confirm->statement, "\n",
                             board.confirm->accept);
  }
  return text;
}

void shows(const app::gui::Dashboard& board, std::initializer_list<std::string_view> parts) {
  const std::string text = said(board);
  for (const std::string_view part : parts) {
    const bool found = app::text::contains(text, part);
    CHECK_MESSAGE(found, "the window lacks \"", part, "\":\n", text);
  }
}

void fill(app::gui::Dashboard& board, std::initializer_list<std::pair<std::string_view, std::string_view>> values) {
  REQUIRE_MESSAGE(board.form.has_value(), "no form is open");
  for (const auto& [key, value] : values) {
    board.form->set(key, value);
  }
}

app::gui::Dashboard started(app::service::App& app, app::gui::Options options) {
  app::gui::Dashboard board(app, std::move(options));
  board.answer(false);
  return board;
}

app::records::Input input(std::string_view name, app::theme::Severity severity) {
  app::records::Input made;
  made.name = std::string(name);
  made.severity = severity;
  return made;
}

std::vector<std::string> names(const app::gui::Dashboard& board) {
  std::vector<std::string> listed;
  for (const app::records::Record& record : board.rec.list) {
    listed.push_back(record.name);
  }
  return listed;
}

bool shown_line(std::string_view text, std::string_view wanted) {
  const std::vector<std::string> lines = app::text::split(text, '\n');
  return std::ranges::find(lines, wanted) != lines.end();
}

bool near(std::array<std::uint8_t, 4> pixel, app::theme::Color tint) {
  const auto close = [](std::uint8_t wanted, std::uint8_t got) { return std::abs(int{wanted} - int{got}) <= 2; };
  return close(tint.r, pixel[0]) && close(tint.g, pixel[1]) && close(tint.b, pixel[2]);
}

std::size_t tints(const app::gui::Picture& picture) {
  std::set<std::array<std::uint8_t, 4>> found;
  for (std::size_t offset = 0; offset + 3 < picture.pixels.size(); offset += 4) {
    found.insert(
        {picture.pixels[offset], picture.pixels[offset + 1], picture.pixels[offset + 2], picture.pixels[offset + 3]});
  }
  return found.size();
}

Window::Window(app::gui::Dashboard& board, float width, float height)
    : stage_(width, height, 1.0F), faces_(app::gui::install_fonts()), view_(board, faces_) {}

void Window::step() {
  view_.prepare();
  stage_.run([this] { view_.frame(); });
}

std::string Window::paint(std::size_t frames) {
  for (std::size_t index = 0; index < frames; ++index) {
    step();
  }
  return text();
}

app::gui::Picture Window::picture() {
  static_cast<void>(paint());
  return stage_.picture();
}

std::string Window::text() const {
  std::string joined;
  for (const app::gui::Seen& item : view_.seen()) {
    if (item.kind == app::gui::SeenKind::Hint || item.kind == app::gui::SeenKind::Icon) {
      continue;
    }
    joined += item.text;
    joined += '\n';
  }
  return joined;
}

std::vector<std::string> Window::hints() const {
  std::vector<std::string> found;
  for (const app::gui::Seen& item : view_.seen()) {
    if (item.kind == app::gui::SeenKind::Hint) {
      found.push_back(item.text);
    }
  }
  return found;
}

std::vector<app::gui::Seen> Window::all(std::string_view wanted) const {
  std::vector<app::gui::Seen> found;
  for (const app::gui::Seen& item : view_.seen()) {
    const bool placed = item.max.x > item.min.x && item.max.y > item.min.y;
    if (placed && item.kind != app::gui::SeenKind::Hint && item.kind != app::gui::SeenKind::Icon &&
        item.text == wanted) {
      found.push_back(item);
    }
  }
  return found;
}

app::gui::Seen Window::leftmost(std::string_view wanted) const {
  const std::vector<app::gui::Seen> found = all(wanted);
  REQUIRE_MESSAGE(!found.empty(), "\"", wanted, "\" is not painted:\n", text());
  return *std::ranges::min_element(found, {}, [](const app::gui::Seen& item) { return item.min.x; });
}

app::gui::Seen Window::rightmost(std::string_view wanted) const {
  const std::vector<app::gui::Seen> found = all(wanted);
  REQUIRE_MESSAGE(!found.empty(), "\"", wanted, "\" is not painted:\n", text());
  return *std::ranges::max_element(found, {}, [](const app::gui::Seen& item) { return item.min.x; });
}

bool Window::click(std::string_view wanted) {
  const std::vector<app::gui::Seen> found = all(wanted);
  if (found.empty()) {
    return false;
  }
  click_at(found.front().middle());
  return true;
}

void Window::click_at(ImVec2 point) {
  stage_.move(point);
  step();
  stage_.press(true);
  step();
  stage_.press(false);
  step();
  step();
}

void Window::hover(ImVec2 point, double seconds) {
  stage_.move(point);
  step();
  stage_.advance(seconds);
  step();
  stage_.advance(1.0 / 60.0);
  step();
  step();
}

void Window::wheel(ImVec2 point, float sideways, float upright) {
  stage_.move(point);
  step();
  stage_.wheel(sideways, upright);
  step();
}

void Window::chord(ImGuiKey key, bool command) {
#if defined(__APPLE__)
  const ImGuiKey modifier = ImGuiMod_Super;
#else
  const ImGuiKey modifier = ImGuiMod_Ctrl;
#endif
  if (command) {
    stage_.key(modifier, true);
  }
  stage_.key(key, true);
  step();
  stage_.key(key, false);
  if (command) {
    stage_.key(modifier, false);
  }
  step();
}

void Window::type(std::string_view words) {
  const std::string copy(words);
  stage_.type(copy.c_str());
  step();
}

app::gui::View& Window::view() {
  return view_;
}

app::gui::Stage& Window::stage() {
  return stage_;
}

}
