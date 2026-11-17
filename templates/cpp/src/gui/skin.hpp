#pragma once

#include <imgui.h>

#include <cstdint>
#include <string_view>

#include "core/records.hpp"
#include "core/theme.hpp"

namespace app::gui {

inline constexpr float icon_scale = 1.3F;
inline constexpr std::string_view regular_face = "NotoSans-Regular";
inline constexpr std::string_view medium_face = "NotoSans-Medium";

struct Faces {
  ImFont* regular = nullptr;
  ImFont* medium = nullptr;
  ImFont* mono = nullptr;
};

[[nodiscard]] std::string_view regular_bytes();
[[nodiscard]] std::string_view medium_bytes();
[[nodiscard]] std::string_view font_license();
[[nodiscard]] Faces install_fonts();
[[nodiscard]] ImU32 color(theme::Color value);
[[nodiscard]] ImU32 alpha(theme::Color value, std::uint8_t share);
[[nodiscard]] ImVec4 paint(theme::Color value);
[[nodiscard]] theme::Color unpaint(const ImVec4& value);
[[nodiscard]] std::string_view severity_icon(theme::Severity level);
[[nodiscard]] std::string_view status_icon(records::Status status);
[[nodiscard]] theme::Tokens scaled(const theme::Tokens& tokens, float scale);
[[nodiscard]] ImGuiStyle style(const theme::Tokens& tokens);
void apply(const theme::Tokens& tokens);

}
