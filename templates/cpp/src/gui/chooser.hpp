#pragma once

#include <string>
#include <string_view>

#include "core/theme.hpp"
#include "gui/ui.hpp"

namespace app::gui::chooser {

inline constexpr std::string_view dashboard = "Dashboard (graphical window)";
inline constexpr std::string_view terminal = "Terminal (text interface)";
inline constexpr std::string_view remember_label = "Remember my choice";
inline constexpr std::string_view later = "Change this later in Settings, Open with.";
inline constexpr float width = 460.0F;
inline constexpr float height = 290.0F;

enum class ChoiceKind { Waiting, Mode, Quit };

struct Choice {
  ChoiceKind kind = ChoiceKind::Waiting;
  std::string_view mode;
  bool operator==(const Choice&) const = default;
};

[[nodiscard]] std::string title();
[[nodiscard]] theme::Tokens tokens(bool dark);
Choice show(const Ui& ui, std::string_view default_mode, bool& remember, bool focus);

}
