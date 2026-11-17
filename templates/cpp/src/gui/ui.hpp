#pragma once

#include <imgui.h>

#include <cstddef>
#include <optional>
#include <span>
#include <string>
#include <string_view>
#include <vector>

#include "core/theme.hpp"
#include "gui/forms.hpp"
#include "gui/icons.hpp"
#include "gui/skin.hpp"

namespace app::gui {

enum class SeenKind { Text, Button, Toggle, Choice, Input, Tab, Row, Icon, Hint };

struct Seen {
  SeenKind kind = SeenKind::Text;
  std::string text;
  ImVec2 min;
  ImVec2 max;

  [[nodiscard]] ImVec2 middle() const;
};

enum class ButtonKind { Plain, Primary, Danger, Bare };

struct Edited {
  bool changed = false;
  bool entered = false;
  bool left = false;
};

class Ui {
 public:
  Ui(const theme::Tokens& tokens, Faces faces, Icons& icons, std::vector<Seen>& seen, float scale);

  [[nodiscard]] const theme::Tokens& tokens() const;
  [[nodiscard]] const Faces& faces() const;
  [[nodiscard]] Icons& icons() const;
  [[nodiscard]] float u(float value) const;
  [[nodiscard]] float icon_side() const;
  [[nodiscard]] float row_height() const;
  [[nodiscard]] ImVec2 measure(std::string_view text, ImFont* font = nullptr, float size = 0) const;
  [[nodiscard]] std::string fit(std::string_view text, float width, ImFont* font = nullptr, float size = 0) const;
  [[nodiscard]] float button_width(std::string_view label, std::string_view icon = {}) const;

  void wrap_at(float local_x);
  void note(SeenKind kind, std::string_view text) const;
  void note(SeenKind kind, std::string_view text, ImVec2 min, ImVec2 max) const;
  void space(float height) const;
  void text(std::string_view text) const;
  void colored(std::string_view text, theme::Color tint) const;
  void muted(std::string_view text) const;
  void small(std::string_view text) const;
  void strong(std::string_view text) const;
  void heading(std::string_view text) const;
  void subheading(std::string_view text) const;
  void wrapped(std::string_view text) const;
  void wrapped_muted(std::string_view text) const;
  void mono(std::string_view text, bool wrap) const;
  void figure(std::string_view text, theme::Color tint) const;
  void icon(std::string_view name, theme::Color tint) const;
  void paint_icon(ImDrawList* canvas, std::string_view name, ImVec2 corner, float side, ImU32 tint) const;
  void paint_text(ImDrawList* canvas, std::string_view text, ImVec2 corner, ImU32 tint, ImFont* font = nullptr,
                  float size = 0) const;
  void paint_fitted(ImDrawList* canvas, std::string_view text, ImVec2 corner, float room, ImU32 tint,
                    ImFont* font = nullptr, float size = 0) const;
  void tooltip(std::string_view text) const;
  void separator() const;

  bool button(std::string_view label, std::string_view icon = {}, ButtonKind kind = ButtonKind::Plain,
              bool enabled = true, float width = 0, std::optional<theme::Color> icon_tint = std::nullopt) const;
  bool icon_button(std::string_view id, std::string_view icon, std::string_view hint, theme::Color tint) const;
  bool checkbox(std::string_view label, bool& value) const;
  bool combo(std::string_view id, std::string_view title, const Choices& options, std::string& value,
             float width) const;
  Edited input(std::string_view id, std::string_view hint, std::string& value, std::size_t limit, float width,
               bool focus = false) const;
  Edited area(std::string_view id, std::string& value, std::size_t limit, float width, float height) const;
  bool tab(std::string_view label, bool selected) const;
  bool entry(std::string_view id, std::string_view label, bool selected, float indent) const;

 private:
  void line(std::string_view text, ImU32 tint, ImFont* font, float size, bool wrap) const;

  const theme::Tokens* tokens_;
  Faces faces_;
  Icons* icons_;
  std::vector<Seen>* seen_;
  float scale_;
  float wrap_ = 0;
};

}
