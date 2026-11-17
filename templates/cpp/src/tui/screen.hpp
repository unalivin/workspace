#pragma once

#include <ftxui/component/event.hpp>
#include <ftxui/screen/color.hpp>
#include <ftxui/screen/screen.hpp>

#include <optional>

#include "core/theme.hpp"
#include "tui/model.hpp"
#include "tui/text.hpp"
#include "tui/widgets.hpp"

namespace app::tui {

struct CellStyle {
  ftxui::Color foreground = ftxui::Color::Default;
  ftxui::Color background = ftxui::Color::Default;
  bool bold = false;
  bool underline = false;
  bool reverse = false;
};

[[nodiscard]] std::optional<Key> convert(const ftxui::Event& event);
[[nodiscard]] CellStyle convert_style(const theme::Style& style, theme::Profile profile);
void paint(ftxui::Screen& screen, const Lines& lines, const Model& model);
void draw(ftxui::Screen& screen, Model& model);

}
