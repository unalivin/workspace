#include "tui/screen.hpp"

#include <ftxui/component/component.hpp>
#include <ftxui/component/loop.hpp>
#include <ftxui/component/screen_interactive.hpp>
#include <ftxui/dom/node.hpp>
#include <ftxui/screen/string.hpp>
#include <ftxui/screen/terminal.hpp>

#include <array>
#include <chrono>
#include <memory>
#include <string>
#include <thread>
#include <utility>

#include "core/error.hpp"
#include "core/text.hpp"
#include "tui/tui.hpp"

namespace app::tui {

namespace {

struct Named {
  const ftxui::Event* event = nullptr;
  Key key = Key{};
};

std::optional<ftxui::Color> color(const theme::TermColor& value, theme::Profile profile) {
  if (const auto* ansi = std::get_if<theme::Ansi>(&value)) {
    if (ansi->code < 16) {
      return ftxui::Color(static_cast<ftxui::Color::Palette16>(ansi->code));
    }
    return ftxui::Color(static_cast<ftxui::Color::Palette256>(ansi->code));
  }
  if (const auto* rgb = std::get_if<theme::Color>(&value)) {
    if (profile == theme::Profile::Ansi256) {
      return ftxui::Color(static_cast<ftxui::Color::Palette256>(theme::ansi256(*rgb)));
    }
    return ftxui::Color::RGB(rgb->r, rgb->g, rgb->b);
  }
  return std::nullopt;
}

ftxui::Terminal::Color support(theme::Profile profile) {
  switch (profile) {
    case theme::Profile::Mono:
      return ftxui::Terminal::Color::Palette1;
    case theme::Profile::Ansi16:
      return ftxui::Terminal::Color::Palette16;
    case theme::Profile::Ansi256:
      return ftxui::Terminal::Color::Palette256;
    case theme::Profile::TrueColor:
      return ftxui::Terminal::Color::TrueColor;
  }
  return ftxui::Terminal::Color::Palette1;
}

class Surface : public ftxui::Node {
 public:
  explicit Surface(Model& model) : model_(&model) {}

  void ComputeRequirement() override {
    requirement_.min_x = 0;
    requirement_.min_y = 0;
    requirement_.flex_grow_x = 1;
    requirement_.flex_grow_y = 1;
    requirement_.flex_shrink_x = 1;
    requirement_.flex_shrink_y = 1;
  }

  void Render(ftxui::Screen& screen) override {
    draw(screen, *model_);
  }

 private:
  Model* model_;
};

}

std::optional<Key> convert(const ftxui::Event& event) {
  if (event.is_mouse()) {
    return std::nullopt;
  }
  if (event.is_character()) {
    const std::u32string letters = text::decode(event.character());
    if (letters.empty()) {
      return std::nullopt;
    }
    return Key::letter(letters.front());
  }
  const std::array<Named, 26> named = {{
      {&ftxui::Event::Return, keys::enter},
      {&ftxui::Event::Escape, keys::esc},
      {&ftxui::Event::Tab, keys::tab},
      {&ftxui::Event::TabReverse, keys::back_tab},
      {&ftxui::Event::ArrowUp, keys::up},
      {&ftxui::Event::ArrowDown, keys::down},
      {&ftxui::Event::ArrowLeft, keys::left},
      {&ftxui::Event::ArrowRight, keys::right},
      {&ftxui::Event::Home, keys::home},
      {&ftxui::Event::End, keys::end},
      {&ftxui::Event::PageUp, keys::page_up},
      {&ftxui::Event::PageDown, keys::page_down},
      {&ftxui::Event::Backspace, keys::backspace},
      {&ftxui::Event::Delete, keys::del},
      {&ftxui::Event::F1, Key::function(1)},
      {&ftxui::Event::F2, Key::function(2)},
      {&ftxui::Event::F3, Key::function(3)},
      {&ftxui::Event::F4, Key::function(4)},
      {&ftxui::Event::F5, Key::function(5)},
      {&ftxui::Event::F6, Key::function(6)},
      {&ftxui::Event::F7, Key::function(7)},
      {&ftxui::Event::F8, Key::function(8)},
      {&ftxui::Event::F9, Key::function(9)},
      {&ftxui::Event::F10, Key::function(10)},
      {&ftxui::Event::F11, Key::function(11)},
      {&ftxui::Event::F12, Key::function(12)},
  }};
  for (const Named& entry : named) {
    if (event == *entry.event) {
      return entry.key;
    }
  }
  const std::string& input = event.input();
  if (input == "\r") {
    return keys::enter;
  }
  if (input == "\x08") {
    return keys::backspace;
  }
  if (input.size() == 1 && input.front() >= 1 && input.front() <= 26) {
    return Key::ctrl(U'a' + static_cast<char32_t>(input.front() - 1));
  }
  return std::nullopt;
}

bool dark_background(const platform::Env& env) {
  const std::string reported = env.get("COLORFGBG").value_or("");
  const std::size_t separator = reported.rfind(';');
  const std::string_view last =
      text::trim_whitespace(std::string_view(reported).substr(separator == std::string::npos ? 0 : separator + 1));
  const std::optional<std::int64_t> background = text::is_digits(last) ? text::parse_integer(last) : std::nullopt;
  if (!background || *background > 255) {
    return true;
  }
  return *background < 7 || *background == 8;
}

CellStyle convert_style(const theme::Style& style, theme::Profile profile) {
  CellStyle out;
  if (const auto foreground = color(style.foreground, profile)) {
    out.foreground = *foreground;
  }
  if (const auto background = color(style.background, profile)) {
    out.background = *background;
  }
  out.bold = style.bold;
  out.underline = style.underline;
  out.reverse = style.reverse;
  return out;
}

void paint(ftxui::Screen& screen, const Lines& lines, const Model& model) {
  const theme::Profile profile = model.profile();
  const int columns = screen.dimx();
  const int rows = screen.dimy();
  int y = 0;
  for (const Line& line : lines) {
    if (y >= rows) {
      break;
    }
    int x = 0;
    for (const Span& span : line.spans) {
      const CellStyle look = convert_style(model.style(span.look), profile);
      for (const std::string& glyph : ftxui::Utf8ToGlyphs(span.text)) {
        if (x >= columns) {
          break;
        }
        auto& cell = screen.PixelAt(x, y);
        cell.character = glyph;
        cell.foreground_color = look.foreground;
        cell.background_color = look.background;
        cell.bold = look.bold;
        cell.underlined = look.underline;
        cell.inverted = look.reverse;
        ++x;
      }
    }
    ++y;
  }
}

void draw(ftxui::Screen& screen, Model& model) {
  model.resize(static_cast<std::size_t>(std::max(screen.dimx(), 0)),
               static_cast<std::size_t>(std::max(screen.dimy(), 0)));
  paint(screen, model.view(), model);
}

std::string run(service::App& app, Options options) {
  Model model(app, std::move(options));
  try {
    ftxui::Terminal::SetColorSupport(support(model.profile()));
    auto screen = ftxui::ScreenInteractive::FullscreenAlternateScreen();
    screen.TrackMouse(false);
    screen.ForceHandleCtrlC(false);
    ftxui::Component view = ftxui::Renderer([&model]() -> ftxui::Element { return std::make_shared<Surface>(model); });
    ftxui::Component root = ftxui::CatchEvent(view, [&model, &screen](const ftxui::Event& event) {
      const std::optional<Key> key = convert(event);
      if (!key) {
        return false;
      }
      model.handle(*key);
      ftxui::Terminal::SetColorSupport(support(model.profile()));
      if (model.finished()) {
        screen.Exit();
      }
      return true;
    });
    ftxui::Loop loop(&screen, root);
    auto deadline = std::chrono::steady_clock::now() + model.interval();
    while (!loop.HasQuitted() && !model.finished()) {
      loop.RunOnce();
      if (std::chrono::steady_clock::now() >= deadline) {
        model.tick();
        deadline = std::chrono::steady_clock::now() + model.interval();
        screen.PostEvent(ftxui::Event::Custom);
      }
      std::this_thread::sleep_for(std::chrono::milliseconds(10));
    }
  } catch (const AppError&) {
    throw;
  } catch (const std::exception& error) {
    throw AppError::io(text::concat("the terminal UI stopped: ", error.what()));
  }
  return std::string(model.switch_to());
}

}
