#include "gui/table.hpp"

#include <algorithm>
#include <cmath>

#include "core/text.hpp"

namespace app::gui::table {

Cell Cell::plain(std::string text) {
  return {CellKind::Text, std::move(text), {}, {}};
}

Cell Cell::muted(std::string text) {
  return {CellKind::Muted, std::move(text), {}, {}};
}

Cell Cell::marked(std::string_view icon, theme::Color tint, std::string text) {
  return {CellKind::Icon, std::move(text), icon, tint};
}

Spec fixed(float width) {
  return {width, 0.0F};
}

Spec flexible(float weight, float least) {
  return {least, weight};
}

std::vector<Slot> layout(std::span<const Spec> specs, float available, float gap) {
  const float gaps = specs.empty() ? 0.0F : gap * static_cast<float>(specs.size() - 1);
  float fixed_total = 0;
  float weights = 0;
  for (const Spec& spec : specs) {
    fixed_total += spec.fixed;
    weights += spec.weight;
  }
  const float spare = std::max(available - gaps - fixed_total, 0.0F);
  std::vector<Slot> slots;
  slots.reserve(specs.size());
  float left = 0;
  for (const Spec& spec : specs) {
    const float width = spec.fixed + (weights > 0.0F ? spare * spec.weight / weights : 0.0F);
    slots.push_back({left, width});
    left += width + gap;
  }
  return slots;
}

float needed(std::span<const Spec> specs, float gap) {
  float total = gap * static_cast<float>(specs.size() + 1);
  for (const Spec& spec : specs) {
    total += spec.fixed;
  }
  return total;
}

void header(const Ui& ui, std::span<const Slot> slots, std::span<const std::string_view> titles) {
  const theme::Tokens& tokens = ui.tokens();
  const float height = ui.row_height();
  const float width = ImGui::GetContentRegionAvail().x;
  const ImVec2 corner = ImGui::GetCursorScreenPos();
  ImGui::Dummy(ImVec2(width, height));
  ImDrawList* canvas = ImGui::GetWindowDrawList();
  const float size = tokens.type_scale.caption;
  for (std::size_t index = 0; index < std::min(slots.size(), titles.size()); ++index) {
    ui.paint_fitted(canvas, titles[index],
                    ImVec2(corner.x + tokens.spacing.s + slots[index].left, corner.y + (height - size) / 2.0F),
                    slots[index].width, color(tokens.palette.text), ui.faces().medium, size);
  }
  canvas->AddLine(ImVec2(corner.x, corner.y + height - 0.5F), ImVec2(corner.x + width, corner.y + height - 0.5F),
                  color(tokens.palette.divider), 1.0F);
}

Click row(const Ui& ui, std::string_view id, std::span<const Slot> slots, const std::vector<Cell>& cells, bool selected,
          bool interactive) {
  const theme::Tokens& tokens = ui.tokens();
  const float height = ui.row_height();
  const float width = ImGui::GetContentRegionAvail().x;
  const ImVec2 corner = ImGui::GetCursorScreenPos();
  Click click;
  if (interactive) {
    const std::string name = text::concat("##", id);
    ImGui::PushStyleVar(ImGuiStyleVar_ItemSpacing, ImVec2(0.0F, 0.0F));
    click.once =
        ImGui::Selectable(name.c_str(), selected, ImGuiSelectableFlags_AllowDoubleClick, ImVec2(width, height));
    click.twice = ImGui::IsItemHovered() && ImGui::IsMouseDoubleClicked(ImGuiMouseButton_Left);
    ImGui::PopStyleVar();
  } else {
    ImGui::PushStyleVar(ImGuiStyleVar_ItemSpacing, ImVec2(0.0F, 0.0F));
    ImGui::Dummy(ImVec2(width, height));
    ImGui::PopStyleVar();
  }
  ImDrawList* canvas = ImGui::GetWindowDrawList();
  const float gap = tokens.spacing.s;
  const float body = tokens.type_scale.body;
  std::string summary;
  for (std::size_t index = 0; index < std::min(slots.size(), cells.size()); ++index) {
    const Cell& cell = cells[index];
    float left = corner.x + gap + slots[index].left;
    float room = slots[index].width;
    if (cell.kind == CellKind::Icon) {
      const float side = ui.icon_side();
      ui.paint_icon(canvas, cell.icon, ImVec2(left, corner.y + (height - side) / 2.0F), side, color(cell.tint));
      left += side + gap;
      room -= side + gap;
    }
    const theme::Color ink = cell.kind == CellKind::Muted ? tokens.palette.text_muted : tokens.palette.text;
    ui.paint_fitted(canvas, cell.text, ImVec2(left, corner.y + (height - body) / 2.0F), room, color(ink));
    if (!summary.empty()) {
      summary += " | ";
    }
    summary += cell.text;
  }
  ui.note(SeenKind::Row, summary, corner, ImVec2(corner.x + width, corner.y + height));
  return click;
}

}
