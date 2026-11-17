#include "gui/chooser.hpp"

#include <array>
#include <cmath>
#include <utility>

#include "core/launcher.hpp"
#include "core/meta.hpp"
#include "core/text.hpp"

namespace app::gui::chooser {

namespace {

constexpr float logo_side = 48.0F;

}

std::string title() {
  return text::concat("How do you want to open ", meta::app_name, "?");
}

theme::Tokens tokens(bool dark) {
  theme::Options options;
  options.name = "slate";
  options.mode = dark ? theme::Mode::Dark : theme::Mode::Light;
  options.text_scale = 100;
  return theme::resolve(options);
}

Choice show(const Ui& ui, std::string_view default_mode, bool& remember, bool focus) {
  Choice choice;
  const theme::Tokens& look = ui.tokens();
  const ImGuiViewport* viewport = ImGui::GetMainViewport();
  ImGui::SetNextWindowPos(viewport->WorkPos);
  ImGui::SetNextWindowSize(viewport->WorkSize);
  const float pad = std::round(look.spacing.l);
  ImGui::PushStyleVar(ImGuiStyleVar_WindowPadding, ImVec2(pad, pad));
  ImGui::PushStyleVar(ImGuiStyleVar_WindowRounding, 0.0F);
  ImGui::PushStyleVar(ImGuiStyleVar_WindowBorderSize, 0.0F);
  ImGui::Begin("##chooser", nullptr,
               ImGuiWindowFlags_NoDecoration | ImGuiWindowFlags_NoMove | ImGuiWindowFlags_NoSavedSettings);
  ImGui::PopStyleVar(3);
  const float side = ui.u(logo_side);
  const ImVec2 corner = ImGui::GetCursorScreenPos();
  ImGui::Dummy(ImVec2(side, side));
  ui.icons().logo(ImGui::GetWindowDrawList(), corner, side);
  ui.note(SeenKind::Icon, "logo", corner, ImVec2(corner.x + side, corner.y + side));
  ImGui::SameLine();
  ImGui::BeginGroup();
  const std::string heading = title();
  const float room = ImGui::GetContentRegionAvail().x;
  const float lines = ImGui::CalcTextSize(heading.c_str(), nullptr, false, room).y;
  ImGui::Dummy(ImVec2(0.0F, std::max((side - lines * look.type_scale.subheading / look.type_scale.body) / 2.0F -
                                         ImGui::GetStyle().ItemSpacing.y,
                                     0.0F)));
  ui.subheading(heading);
  ImGui::EndGroup();
  ui.space(look.spacing.s);
  const float full = ImGui::GetContentRegionAvail().x;
  const bool wants_terminal = default_mode == launcher::mode_tui;
  const std::array<std::pair<std::string_view, std::string_view>, 2> modes = {
      {{launcher::mode_gui, dashboard}, {launcher::mode_tui, terminal}}};
  for (const auto& [mode, label] : modes) {
    const bool preferred = wants_terminal == (mode == launcher::mode_tui);
    if (preferred && focus) {
      ImGui::SetKeyboardFocusHere();
    }
    ImGui::PushStyleVar(
        ImGuiStyleVar_FramePadding,
        ImVec2(ImGui::GetStyle().FramePadding.x, std::max((ui.row_height() - ImGui::GetFontSize()) / 2.0F, 0.0F)));
    if (ui.button(label, {}, preferred ? ButtonKind::Primary : ButtonKind::Plain, true, full)) {
      choice = {ChoiceKind::Mode, mode};
    }
    ImGui::PopStyleVar();
  }
  ui.checkbox(remember_label, remember);
  ImGui::SameLine();
  const float quit = ui.button_width("Quit");
  ImGui::SetCursorPosX(ImGui::GetCursorPosX() + std::max(ImGui::GetContentRegionAvail().x - quit, 0.0F));
  if (ui.button("Quit")) {
    choice = {ChoiceKind::Quit, {}};
  }
  ui.muted(later);
  ImGui::End();
  return choice;
}

}
