#pragma once

#include <imgui.h>

#include <span>
#include <string>
#include <string_view>
#include <vector>

#include "gui/ui.hpp"

namespace app::gui::table {

struct Spec {
  float fixed = 0;
  float weight = 0;
  bool operator==(const Spec&) const = default;
};

struct Slot {
  float left = 0;
  float width = 0;
  bool operator==(const Slot&) const = default;
};

enum class CellKind { Text, Muted, Icon };

struct Cell {
  CellKind kind = CellKind::Text;
  std::string text;
  std::string_view icon;
  theme::Color tint;

  [[nodiscard]] static Cell plain(std::string text);
  [[nodiscard]] static Cell muted(std::string text);
  [[nodiscard]] static Cell marked(std::string_view icon, theme::Color tint, std::string text);
};

struct Click {
  bool once = false;
  bool twice = false;
};

[[nodiscard]] Spec fixed(float width);
[[nodiscard]] Spec flexible(float weight, float least);
[[nodiscard]] std::vector<Slot> layout(std::span<const Spec> specs, float available, float gap);
[[nodiscard]] float needed(std::span<const Spec> specs, float gap);
void header(const Ui& ui, std::span<const Slot> slots, std::span<const std::string_view> titles);
Click row(const Ui& ui, std::string_view id, std::span<const Slot> slots, const std::vector<Cell>& cells, bool selected,
          bool interactive = true);

}
