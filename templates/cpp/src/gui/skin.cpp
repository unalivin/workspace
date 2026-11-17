#include "gui/skin.hpp"

#include <algorithm>
#include <cmath>
#include <cstddef>

namespace app::embedded {

extern const unsigned char font_regular_data[];
extern const std::size_t font_regular_size;
extern const unsigned char font_medium_data[];
extern const std::size_t font_medium_size;
extern const unsigned char font_license_data[];
extern const std::size_t font_license_size;

}

namespace app::gui {

namespace {

std::string_view view(const unsigned char* data, std::size_t size) {
  return {reinterpret_cast<const char*>(data), size};
}

ImFont* face(ImFontAtlas* atlas, std::string_view bytes, std::string_view name) {
  ImFontConfig config;
  config.FontDataOwnedByAtlas = false;
  const std::size_t length = std::min(name.size(), sizeof(config.Name) - 1);
  name.copy(config.Name, length);
  config.Name[length] = '\0';
  void* data = const_cast<void*>(static_cast<const void*>(bytes.data()));
  return atlas->AddFontFromMemoryTTF(data, static_cast<int>(bytes.size()), 0.0F, &config);
}

float span(float value) {
  return std::round(value);
}

}

std::string_view regular_bytes() {
  return view(embedded::font_regular_data, embedded::font_regular_size);
}

std::string_view medium_bytes() {
  return view(embedded::font_medium_data, embedded::font_medium_size);
}

std::string_view font_license() {
  return view(embedded::font_license_data, embedded::font_license_size);
}

Faces install_fonts() {
  ImFontAtlas* atlas = ImGui::GetIO().Fonts;
  Faces faces;
  faces.regular = face(atlas, regular_bytes(), regular_face);
  faces.medium = face(atlas, medium_bytes(), medium_face);
  faces.mono = atlas->AddFontDefaultVector();
  ImGui::GetIO().FontDefault = faces.regular;
  return faces;
}

ImU32 color(theme::Color value) {
  return IM_COL32(value.r, value.g, value.b, 255);
}

ImU32 alpha(theme::Color value, std::uint8_t share) {
  return IM_COL32(value.r, value.g, value.b, share);
}

ImVec4 paint(theme::Color value) {
  return {static_cast<float>(value.r) / 255.0F, static_cast<float>(value.g) / 255.0F,
          static_cast<float>(value.b) / 255.0F, 1.0F};
}

theme::Color unpaint(const ImVec4& value) {
  const auto part = [](float share) {
    return static_cast<std::uint8_t>(std::lround(std::clamp(share, 0.0F, 1.0F) * 255.0F));
  };
  return {part(value.x), part(value.y), part(value.z)};
}

std::string_view severity_icon(theme::Severity level) {
  switch (level) {
    case theme::Severity::Critical:
      return "octagon-alert";
    case theme::Severity::High:
      return "triangle-alert";
    case theme::Severity::Medium:
      return "circle-alert";
    case theme::Severity::Low:
      return "circle-minus";
    case theme::Severity::Info:
      return "info";
  }
  return "info";
}

std::string_view status_icon(records::Status status) {
  switch (status) {
    case records::Status::Open:
      return "status-open";
    case records::Status::InProgress:
      return "status-progress";
    case records::Status::Done:
      return "status-done";
  }
  return "status-open";
}

theme::Tokens scaled(const theme::Tokens& tokens, float scale) {
  theme::Tokens out = tokens;
  const float factor = std::clamp(scale, 0.5F, 4.0F);
  out.spacing.xs *= factor;
  out.spacing.s *= factor;
  out.spacing.m *= factor;
  out.spacing.l *= factor;
  out.spacing.xl *= factor;
  out.spacing.row_height *= factor;
  out.type_scale.caption *= factor;
  out.type_scale.body *= factor;
  out.type_scale.subheading *= factor;
  out.type_scale.heading *= factor;
  out.type_scale.figure *= factor;
  out.radius *= factor;
  return out;
}

ImGuiStyle style(const theme::Tokens& tokens) {
  const theme::Palette& palette = tokens.palette;
  const theme::Spacing& gaps = tokens.spacing;
  ImGuiStyle look;
  look.FontSizeBase = tokens.type_scale.body;
  look.Alpha = 1.0F;
  look.DisabledAlpha = 0.5F;
  look.WindowPadding = ImVec2(span(gaps.l), span(gaps.l));
  look.WindowRounding = tokens.radius;
  look.WindowBorderSize = 1.0F;
  look.WindowMinSize = ImVec2(32.0F, 32.0F);
  look.ChildRounding = tokens.radius;
  look.ChildBorderSize = 1.0F;
  look.PopupRounding = tokens.radius;
  look.PopupBorderSize = 1.0F;
  const float pad = std::max(span((gaps.row_height - tokens.type_scale.body * 1.36F) / 2.0F), 2.0F);
  look.FramePadding = ImVec2(span(gaps.m), pad);
  look.FrameRounding = tokens.radius;
  look.FrameBorderSize = 1.0F;
  look.ItemSpacing = ImVec2(span(gaps.s), span(gaps.xs + 1.0F));
  look.ItemInnerSpacing = ImVec2(span(gaps.s), span(gaps.xs));
  look.CellPadding = ImVec2(span(gaps.s), span(gaps.xs));
  look.IndentSpacing = span(gaps.l);
  look.ScrollbarSize = span(gaps.s + 2.0F);
  look.ScrollbarRounding = tokens.radius;
  look.GrabMinSize = span(gaps.m);
  look.GrabRounding = tokens.radius;
  look.TabRounding = tokens.radius;
  look.SeparatorTextBorderSize = 1.0F;
  look.ButtonTextAlign = ImVec2(0.5F, 0.5F);
  look.SelectableTextAlign = ImVec2(0.0F, 0.5F);
  look.AntiAliasedLines = true;
  look.AntiAliasedFill = true;
  look.HoverDelayNormal = 0.4F;
  look.HoverStationaryDelay = 0.15F;

  const theme::Color hover = theme::mix(palette.surface_alt, palette.text, 0.08);
  const theme::Color active = theme::mix(palette.surface_alt, palette.text, 0.16);
  const theme::Color wash = theme::mix(palette.background, palette.text, 0.08);
  ImVec4* colors = look.Colors;
  const ImVec4 clear(0.0F, 0.0F, 0.0F, 0.0F);
  colors[ImGuiCol_Text] = paint(palette.text);
  colors[ImGuiCol_TextDisabled] = paint(palette.text_muted);
  colors[ImGuiCol_WindowBg] = paint(palette.background);
  colors[ImGuiCol_ChildBg] = clear;
  colors[ImGuiCol_PopupBg] = paint(palette.surface);
  colors[ImGuiCol_Border] = paint(palette.border);
  colors[ImGuiCol_BorderShadow] = clear;
  colors[ImGuiCol_FrameBg] = paint(palette.surface);
  colors[ImGuiCol_FrameBgHovered] = paint(palette.surface_alt);
  colors[ImGuiCol_FrameBgActive] = paint(palette.surface_alt);
  colors[ImGuiCol_TitleBg] = paint(palette.surface);
  colors[ImGuiCol_TitleBgActive] = paint(palette.surface);
  colors[ImGuiCol_TitleBgCollapsed] = paint(palette.surface);
  colors[ImGuiCol_MenuBarBg] = paint(palette.surface);
  colors[ImGuiCol_ScrollbarBg] = clear;
  colors[ImGuiCol_ScrollbarGrab] = paint(palette.border);
  colors[ImGuiCol_ScrollbarGrabHovered] = paint(palette.text_muted);
  colors[ImGuiCol_ScrollbarGrabActive] = paint(palette.text_muted);
  colors[ImGuiCol_CheckMark] = paint(palette.text);
  colors[ImGuiCol_CheckboxSelectedBg] = paint(palette.surface_alt);
  colors[ImGuiCol_SliderGrab] = paint(palette.accent);
  colors[ImGuiCol_SliderGrabActive] = paint(palette.accent);
  colors[ImGuiCol_Button] = paint(palette.surface_alt);
  colors[ImGuiCol_ButtonHovered] = paint(hover);
  colors[ImGuiCol_ButtonActive] = paint(active);
  colors[ImGuiCol_Header] = paint(tokens.selection);
  colors[ImGuiCol_HeaderHovered] = paint(wash);
  colors[ImGuiCol_HeaderActive] = paint(tokens.selection);
  colors[ImGuiCol_Separator] = paint(palette.divider);
  colors[ImGuiCol_SeparatorHovered] = paint(palette.border);
  colors[ImGuiCol_SeparatorActive] = paint(palette.border);
  colors[ImGuiCol_ResizeGrip] = clear;
  colors[ImGuiCol_ResizeGripHovered] = clear;
  colors[ImGuiCol_ResizeGripActive] = clear;
  colors[ImGuiCol_InputTextCursor] = paint(palette.text);
  colors[ImGuiCol_Tab] = paint(palette.background);
  colors[ImGuiCol_TabHovered] = paint(wash);
  colors[ImGuiCol_TabSelected] = paint(tokens.selection);
  colors[ImGuiCol_TabSelectedOverline] = paint(palette.accent);
  colors[ImGuiCol_TabDimmed] = paint(palette.background);
  colors[ImGuiCol_TabDimmedSelected] = paint(tokens.selection);
  colors[ImGuiCol_TabDimmedSelectedOverline] = paint(palette.accent);
  colors[ImGuiCol_PlotLines] = paint(palette.accent);
  colors[ImGuiCol_PlotLinesHovered] = paint(palette.accent);
  colors[ImGuiCol_PlotHistogram] = paint(palette.accent);
  colors[ImGuiCol_PlotHistogramHovered] = paint(palette.accent);
  colors[ImGuiCol_TableHeaderBg] = paint(palette.surface);
  colors[ImGuiCol_TableBorderStrong] = paint(palette.border);
  colors[ImGuiCol_TableBorderLight] = paint(palette.divider);
  colors[ImGuiCol_TableRowBg] = clear;
  colors[ImGuiCol_TableRowBgAlt] = clear;
  colors[ImGuiCol_TextLink] = paint(palette.accent);
  colors[ImGuiCol_TextSelectedBg] = paint(tokens.selection);
  colors[ImGuiCol_TreeLines] = paint(palette.divider);
  colors[ImGuiCol_DragDropTarget] = paint(tokens.focus);
  colors[ImGuiCol_DragDropTargetBg] = clear;
  colors[ImGuiCol_UnsavedMarker] = paint(palette.text);
  colors[ImGuiCol_NavCursor] = paint(tokens.focus);
  colors[ImGuiCol_NavWindowingHighlight] = paint(tokens.focus);
  colors[ImGuiCol_NavWindowingDimBg] = ImVec4(0.0F, 0.0F, 0.0F, tokens.dark ? 0.5F : 0.3F);
  colors[ImGuiCol_ModalWindowDimBg] = ImVec4(0.0F, 0.0F, 0.0F, tokens.dark ? 0.5F : 0.3F);
  return look;
}

void apply(const theme::Tokens& tokens) {
  ImGui::GetStyle() = style(tokens);
}

}
