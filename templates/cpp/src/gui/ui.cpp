#include "gui/ui.hpp"

#include <imgui_stdlib.h>

#include <algorithm>
#include <array>
#include <cmath>

#include "core/text.hpp"

namespace app::gui {

namespace {

const char* first(std::string_view text) {
  return text.data();
}

const char* last(std::string_view text) {
  return text.data() + text.size();
}

std::string hidden(std::string_view id) {
  return text::concat("##", id);
}

}

ImVec2 Seen::middle() const {
  return {(min.x + max.x) / 2.0F, (min.y + max.y) / 2.0F};
}

Ui::Ui(const theme::Tokens& tokens, Faces faces, Icons& icons, std::vector<Seen>& seen, float scale)
    : tokens_(&tokens), faces_(faces), icons_(&icons), seen_(&seen), scale_(scale) {}

const theme::Tokens& Ui::tokens() const {
  return *tokens_;
}

const Faces& Ui::faces() const {
  return faces_;
}

Icons& Ui::icons() const {
  return *icons_;
}

float Ui::u(float value) const {
  return value * scale_;
}

float Ui::icon_side() const {
  return std::round(tokens_->type_scale.body * icon_scale);
}

float Ui::row_height() const {
  return std::round(tokens_->spacing.row_height);
}

ImVec2 Ui::measure(std::string_view text, ImFont* font, float size) const {
  if (text.empty()) {
    return {0.0F, size > 0 ? size : ImGui::GetFontSize()};
  }
  if (font == nullptr && size <= 0) {
    return ImGui::CalcTextSize(first(text), last(text), false);
  }
  ImGui::PushFont(font, size);
  const ImVec2 measured = ImGui::CalcTextSize(first(text), last(text), false);
  ImGui::PopFont();
  return measured;
}

std::string Ui::fit(std::string_view text, float width, ImFont* font, float size) const {
  std::string clean = text::replace_all(text::replace_all(text, "\n", " "), "\t", " ");
  if (measure(clean, font, size).x <= width) {
    return clean;
  }
  const std::u32string letters = text::decode(clean);
  std::size_t low = 0;
  std::size_t high = letters.size();
  while (low < high) {
    const std::size_t middle = (low + high + 1) / 2;
    const std::string candidate = text::concat(text::encode(std::u32string_view(letters).substr(0, middle)), "…");
    if (measure(candidate, font, size).x <= width) {
      low = middle;
    } else {
      high = middle - 1;
    }
  }
  if (low == 0) {
    return measure("…", font, size).x <= width ? "…" : std::string();
  }
  std::u32string_view kept = std::u32string_view(letters).substr(0, low);
  while (!kept.empty() && kept.back() == U' ') {
    kept.remove_suffix(1);
  }
  return text::concat(text::encode(kept), "…");
}

float Ui::button_width(std::string_view label, std::string_view icon) const {
  const ImGuiStyle& look = ImGui::GetStyle();
  float width = look.FramePadding.x * 2.0F;
  if (!icon.empty()) {
    width += icon_side();
  }
  if (!label.empty()) {
    width += measure(label).x;
    if (!icon.empty()) {
      width += look.ItemInnerSpacing.x;
    }
  }
  return std::ceil(width);
}

void Ui::wrap_at(float local_x) {
  wrap_ = local_x;
}

void Ui::note(SeenKind kind, std::string_view text) const {
  note(kind, text, ImGui::GetItemRectMin(), ImGui::GetItemRectMax());
}

void Ui::note(SeenKind kind, std::string_view text, ImVec2 min, ImVec2 max) const {
  seen_->push_back({kind, std::string(text), min, max});
}

void Ui::space(float height) const {
  ImGui::Dummy(ImVec2(0.0F, std::round(height)));
}

void Ui::line(std::string_view text, ImU32 tint, ImFont* font, float size, bool wrap) const {
  ImGui::PushFont(font, size);
  ImGui::PushStyleColor(ImGuiCol_Text, tint);
  if (wrap) {
    ImGui::PushTextWrapPos(wrap_);
  }
  ImGui::TextUnformatted(first(text), last(text));
  if (wrap) {
    ImGui::PopTextWrapPos();
  }
  ImGui::PopStyleColor();
  ImGui::PopFont();
  note(SeenKind::Text, text);
}

void Ui::text(std::string_view text) const {
  line(text, color(tokens_->palette.text), nullptr, 0, false);
}

void Ui::colored(std::string_view text, theme::Color tint) const {
  line(text, color(tint), nullptr, 0, true);
}

void Ui::muted(std::string_view text) const {
  line(text, color(tokens_->palette.text_muted), nullptr, 0, true);
}

void Ui::small(std::string_view text) const {
  line(text, color(tokens_->palette.text_muted), nullptr, tokens_->type_scale.caption, true);
}

void Ui::strong(std::string_view text) const {
  line(text, color(tokens_->palette.text), faces_.medium, 0, true);
}

void Ui::heading(std::string_view text) const {
  line(text, color(tokens_->palette.text), faces_.medium, tokens_->type_scale.heading, false);
}

void Ui::subheading(std::string_view text) const {
  line(text, color(tokens_->palette.text), faces_.medium, tokens_->type_scale.subheading, true);
}

void Ui::wrapped(std::string_view text) const {
  line(text, color(tokens_->palette.text), nullptr, 0, true);
}

void Ui::wrapped_muted(std::string_view text) const {
  line(text, color(tokens_->palette.text_muted), nullptr, 0, true);
}

void Ui::mono(std::string_view text, bool wrap) const {
  line(text, color(tokens_->palette.text), faces_.mono, 0, wrap);
}

void Ui::figure(std::string_view text, theme::Color tint) const {
  line(text, color(tint), faces_.medium, tokens_->type_scale.figure, false);
}

void Ui::paint_icon(ImDrawList* canvas, std::string_view name, ImVec2 corner, float side, ImU32 tint) const {
  icons_->draw(canvas, name, corner, side, tint);
  note(SeenKind::Icon, name, corner, ImVec2(corner.x + side, corner.y + side));
}

void Ui::paint_text(ImDrawList* canvas, std::string_view text, ImVec2 corner, ImU32 tint, ImFont* font,
                    float size) const {
  ImGui::PushFont(font, size);
  canvas->AddText(ImVec2(std::round(corner.x), std::round(corner.y)), tint, first(text), last(text));
  const ImVec2 measured = ImGui::CalcTextSize(first(text), last(text), false);
  ImGui::PopFont();
  note(SeenKind::Text, text, corner, ImVec2(corner.x + measured.x, corner.y + measured.y));
}

void Ui::paint_fitted(ImDrawList* canvas, std::string_view text, ImVec2 corner, float room, ImU32 tint, ImFont* font,
                      float size) const {
  const std::string shown = fit(text, std::max(room, 0.0F), font, size);
  ImGui::PushFont(font, size);
  canvas->AddText(ImVec2(std::round(corner.x), std::round(corner.y)), tint, first(shown), last(shown));
  const ImVec2 measured = ImGui::CalcTextSize(first(shown), last(shown), false);
  ImGui::PopFont();
  note(SeenKind::Text, text, corner, ImVec2(corner.x + measured.x, corner.y + measured.y));
}

void Ui::icon(std::string_view name, theme::Color tint) const {
  const float side = icon_side();
  const float height = std::max(ImGui::GetTextLineHeight(), side);
  const ImVec2 corner = ImGui::GetCursorScreenPos();
  ImGui::Dummy(ImVec2(side, height));
  paint_icon(ImGui::GetWindowDrawList(), name, ImVec2(corner.x, corner.y + (height - side) / 2.0F), side, color(tint));
}

void Ui::tooltip(std::string_view text) const {
  note(SeenKind::Hint, text);
  if (ImGui::IsItemHovered(ImGuiHoveredFlags_ForTooltip) && ImGui::BeginTooltip()) {
    ImGui::PushTextWrapPos(u(360.0F));
    ImGui::TextUnformatted(first(text), last(text));
    ImGui::PopTextWrapPos();
    ImGui::EndTooltip();
  }
}

void Ui::separator() const {
  ImGui::Separator();
}

bool Ui::button(std::string_view label, std::string_view icon, ButtonKind kind, bool enabled, float width,
                std::optional<theme::Color> icon_tint) const {
  const theme::Palette& palette = tokens_->palette;
  theme::Color ink = palette.text;
  int pushed = 0;
  if (kind == ButtonKind::Primary || kind == ButtonKind::Danger) {
    const theme::Color fill = kind == ButtonKind::Primary ? palette.accent : tokens_->semantic.error;
    ink = kind == ButtonKind::Primary ? tokens_->on_accent : theme::best_on(fill);
    ImGui::PushStyleColor(ImGuiCol_Button, color(fill));
    ImGui::PushStyleColor(ImGuiCol_ButtonHovered, color(theme::mix(fill, ink, 0.12)));
    ImGui::PushStyleColor(ImGuiCol_ButtonActive, color(theme::mix(fill, ink, 0.22)));
    pushed = 3;
  } else if (kind == ButtonKind::Bare) {
    ImGui::PushStyleColor(ImGuiCol_Button, IM_COL32(0, 0, 0, 0));
    pushed = 1;
  }
  ImGui::PushStyleVar(ImGuiStyleVar_FrameBorderSize, 0.0F);
  ImGui::BeginDisabled(!enabled);
  const ImGuiStyle& look = ImGui::GetStyle();
  const float natural = button_width(label, icon);
  const ImVec2 size(std::max(width, natural), ImGui::GetFrameHeight());
  const ImVec2 corner = ImGui::GetCursorScreenPos();
  const std::string id = hidden(label.empty() ? icon : label);
  const bool pressed = ImGui::Button(id.c_str(), size);
  ImDrawList* canvas = ImGui::GetWindowDrawList();
  const ImU32 tint = ImGui::GetColorU32(paint(ink));
  float left = corner.x + (width > natural && icon.empty() ? (size.x - natural) / 2.0F : 0.0F) + look.FramePadding.x;
  if (!icon.empty()) {
    const float side = icon_side();
    icons_->draw(canvas, icon, ImVec2(left, corner.y + (size.y - side) / 2.0F), side,
                 icon_tint ? ImGui::GetColorU32(paint(*icon_tint)) : tint);
    left += side + look.ItemInnerSpacing.x;
  }
  if (!label.empty()) {
    canvas->AddText(ImVec2(std::round(left), std::round(corner.y + (size.y - ImGui::GetFontSize()) / 2.0F)), tint,
                    first(label), last(label));
  }
  ImGui::EndDisabled();
  ImGui::PopStyleVar();
  ImGui::PopStyleColor(pushed);
  note(SeenKind::Button, label.empty() ? icon : label, corner, ImVec2(corner.x + size.x, corner.y + size.y));
  return pressed && enabled;
}

bool Ui::icon_button(std::string_view id, std::string_view icon, std::string_view hint, theme::Color tint) const {
  ImGui::PushStyleColor(ImGuiCol_Button, IM_COL32(0, 0, 0, 0));
  ImGui::PushStyleVar(ImGuiStyleVar_FrameBorderSize, 0.0F);
  const float side = icon_side();
  const float edge = ImGui::GetFrameHeight();
  const ImVec2 corner = ImGui::GetCursorScreenPos();
  const std::string name = hidden(id);
  const bool pressed = ImGui::Button(name.c_str(), ImVec2(edge, edge));
  icons_->draw(ImGui::GetWindowDrawList(), icon,
               ImVec2(corner.x + (edge - side) / 2.0F, corner.y + (edge - side) / 2.0F), side, color(tint));
  ImGui::PopStyleVar();
  ImGui::PopStyleColor();
  note(SeenKind::Button, hint, corner, ImVec2(corner.x + edge, corner.y + edge));
  tooltip(hint);
  return pressed;
}

bool Ui::checkbox(std::string_view label, bool& value) const {
  const theme::Palette& palette = tokens_->palette;
  const float box = std::round(tokens_->type_scale.body * 1.25F);
  const float height = ImGui::GetFrameHeight();
  const float gap = ImGui::GetStyle().ItemInnerSpacing.x;
  const ImVec2 measured = measure(label);
  const ImVec2 size(box + (label.empty() ? 0.0F : gap + measured.x), height);
  const ImVec2 corner = ImGui::GetCursorScreenPos();
  const std::string name = text::concat("##check-", label);
  const bool pressed = ImGui::InvisibleButton(name.c_str(), size);
  if (pressed) {
    value = !value;
  }
  const bool hovered = ImGui::IsItemHovered();
  ImDrawList* canvas = ImGui::GetWindowDrawList();
  const ImVec2 from(std::round(corner.x), std::round(corner.y + (height - box) / 2.0F));
  const ImVec2 to(from.x + box, from.y + box);
  const float radius = std::min(tokens_->radius, box / 4.0F);
  if (value) {
    canvas->AddRectFilled(from, to, color(palette.accent), radius);
    const float inset = std::round(box * 0.12F);
    icons_->draw(canvas, "check", ImVec2(from.x + inset, from.y + inset), box - inset * 2.0F,
                 color(tokens_->on_accent));
  } else {
    canvas->AddRectFilled(from, to, color(hovered ? palette.surface_alt : palette.surface), radius);
    canvas->AddRect(from, to, color(palette.border), radius);
  }
  if (!label.empty()) {
    canvas->AddText(ImVec2(std::round(to.x + gap), std::round(corner.y + (height - measured.y) / 2.0F)),
                    color(palette.text), first(label), last(label));
  }
  note(SeenKind::Toggle, label, corner, ImVec2(corner.x + size.x, corner.y + size.y));
  return pressed;
}

bool Ui::combo(std::string_view id, std::string_view title, const Choices& options, std::string& value,
               float width) const {
  bool changed = false;
  const ImGuiStyle& look = ImGui::GetStyle();
  const std::string name = hidden(id);
  const float mark = std::round(tokens_->type_scale.body * 0.3F);
  const std::string shown = fit(title, width - look.FramePadding.x * 2.0F - mark * 2.0F - look.ItemInnerSpacing.x);
  ImGui::SetNextItemWidth(width);
  const ImVec2 corner = ImGui::GetCursorScreenPos();
  const bool open = ImGui::BeginCombo(name.c_str(), shown.c_str(), ImGuiComboFlags_NoArrowButton);
  const ImVec2 middle(corner.x + width - look.FramePadding.x - mark, corner.y + ImGui::GetFrameHeight() / 2.0F);
  const std::array<ImVec2, 3> chevron = {ImVec2(middle.x - mark, middle.y - mark / 2.0F),
                                         ImVec2(middle.x, middle.y + mark / 2.0F),
                                         ImVec2(middle.x + mark, middle.y - mark / 2.0F)};
  ImGui::GetWindowDrawList()->AddPolyline(chevron.data(), static_cast<int>(chevron.size()),
                                          color(tokens_->palette.text_muted), ImDrawFlags_None,
                                          std::max(u(1.5F), 1.0F));
  if (open) {
    for (const auto& [option, label] : options) {
      const bool current = option == value;
      const std::string entry_name = text::concat(label, "##", option);
      if (ImGui::Selectable(entry_name.c_str(), current)) {
        changed = changed || !current;
        value = option;
      }
      note(SeenKind::Choice, label);
      if (current) {
        ImGui::SetItemDefaultFocus();
      }
    }
    ImGui::EndCombo();
  }
  note(SeenKind::Choice, title, corner, ImVec2(corner.x + width, corner.y + ImGui::GetFrameHeight()));
  return changed;
}

Edited Ui::input(std::string_view id, std::string_view hint, std::string& value, std::size_t limit, float width,
                 bool focus) const {
  Edited edited;
  const std::string name = hidden(id);
  const std::string placeholder(hint);
  ImGui::SetNextItemWidth(width);
  if (focus) {
    ImGui::SetKeyboardFocusHere();
  }
  ImGui::PushStyleColor(ImGuiCol_TextDisabled, color(tokens_->palette.text_muted));
  edited.entered =
      ImGui::InputTextWithHint(name.c_str(), placeholder.c_str(), &value, ImGuiInputTextFlags_EnterReturnsTrue);
  ImGui::PopStyleColor();
  edited.changed = ImGui::IsItemEdited();
  edited.left = ImGui::IsItemDeactivated();
  if (limit > 0 && text::length(value) > limit) {
    value = text::truncated(value, limit);
  }
  note(SeenKind::Input, hint.empty() ? id : hint);
  return edited;
}

Edited Ui::area(std::string_view id, std::string& value, std::size_t limit, float width, float height) const {
  Edited edited;
  const std::string name = hidden(id);
  edited.changed = ImGui::InputTextMultiline(name.c_str(), &value, ImVec2(width, height));
  edited.left = ImGui::IsItemDeactivated();
  if (limit > 0 && text::length(value) > limit) {
    value = text::truncated(value, limit);
  }
  note(SeenKind::Input, id);
  return edited;
}

bool Ui::tab(std::string_view label, bool selected) const {
  const ImGuiStyle& look = ImGui::GetStyle();
  const ImVec2 size(measure(label, selected ? faces_.medium : nullptr).x + look.FramePadding.x * 2.0F,
                    ImGui::GetFrameHeight());
  const ImVec2 corner = ImGui::GetCursorScreenPos();
  const std::string name = hidden(label);
  ImGui::PushStyleColor(ImGuiCol_Button, IM_COL32(0, 0, 0, 0));
  ImGui::PushStyleVar(ImGuiStyleVar_FrameBorderSize, 0.0F);
  const bool pressed = ImGui::Button(name.c_str(), size);
  ImGui::PopStyleVar();
  ImGui::PopStyleColor();
  ImDrawList* canvas = ImGui::GetWindowDrawList();
  ImGui::PushFont(selected ? faces_.medium : nullptr, 0.0F);
  const theme::Color ink = selected ? tokens_->palette.text : tokens_->palette.text_muted;
  canvas->AddText(
      ImVec2(std::round(corner.x + look.FramePadding.x), std::round(corner.y + (size.y - ImGui::GetFontSize()) / 2.0F)),
      color(ink), first(label), last(label));
  ImGui::PopFont();
  if (selected) {
    const float thick = std::max(u(2.0F), 2.0F);
    canvas->AddRectFilled(ImVec2(corner.x + look.FramePadding.x, corner.y + size.y - thick),
                          ImVec2(corner.x + size.x - look.FramePadding.x, corner.y + size.y),
                          color(tokens_->palette.accent), thick / 2.0F);
  }
  note(SeenKind::Tab, label, corner, ImVec2(corner.x + size.x, corner.y + size.y));
  return pressed && !selected;
}

bool Ui::entry(std::string_view id, std::string_view label, bool selected, float indent) const {
  const ImVec2 corner = ImGui::GetCursorScreenPos();
  const float width = ImGui::GetContentRegionAvail().x;
  const std::string name = hidden(id);
  const bool pressed = ImGui::Selectable(name.c_str(), selected, ImGuiSelectableFlags_None, ImVec2(0.0F, row_height()));
  const float room = width - indent - tokens_->spacing.s * 2.0F;
  const std::string shown = fit(label, room, selected ? faces_.medium : nullptr);
  ImGui::PushFont(selected ? faces_.medium : nullptr, 0.0F);
  ImGui::GetWindowDrawList()->AddText(ImVec2(std::round(corner.x + indent + tokens_->spacing.s),
                                             std::round(corner.y + (row_height() - ImGui::GetFontSize()) / 2.0F)),
                                      color(tokens_->palette.text), first(shown), last(shown));
  ImGui::PopFont();
  note(SeenKind::Row, label, corner, ImVec2(corner.x + width, corner.y + row_height()));
  return pressed;
}

}
