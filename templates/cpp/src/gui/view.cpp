#include "gui/view.hpp"

#include <algorithm>
#include <cmath>

#include "core/meta.hpp"
#include "core/text.hpp"
#include "gui/ledger.hpp"
#include "gui/table.hpp"

namespace app::gui {

namespace {

constexpr float sidebar_width = 224.0F;
constexpr float nav_marker = 4.0F;
constexpr std::array<float, 4> figure_shares = {3.0F, 2.0F, 2.0F, 2.0F};
constexpr std::array<float, 2> chart_shares = {5.0F, 3.0F};
constexpr double flash_seconds = 0.6;
constexpr ImGuiWindowFlags fixed_window = ImGuiWindowFlags_NoDecoration | ImGuiWindowFlags_NoMove |
                                          ImGuiWindowFlags_NoSavedSettings | ImGuiWindowFlags_NoBringToFrontOnFocus;

}

std::string_view nav_icon(std::string_view id) {
  if (id == service::nav_overview) {
    return "layout-dashboard";
  }
  if (id == service::nav_records) {
    return "list";
  }
  if (id == service::nav_notifications) {
    return "bell";
  }
  if (id == service::nav_settings) {
    return "settings";
  }
  return "info";
}

std::string_view command_key() {
#if defined(__APPLE__)
  return "Cmd";
#else
  return "Ctrl";
#endif
}

std::vector<MenuEntry> menu_entries() {
  const std::string_view command = command_key();
  std::vector<MenuEntry> entries;
  entries.push_back({"Menu", "Open terminal UI", "", MenuAct::Terminal, 0, false});
  entries.push_back({"Menu", "Quit", text::concat(command, "+Q"), MenuAct::Quit, 0, true});
  for (std::size_t index = 0; index < service::navigation.size(); ++index) {
    entries.push_back({"View", std::string(service::navigation[index].label), text::concat(command, "+", index + 1),
                       MenuAct::Page, index, false});
  }
  entries.push_back({"View", "Collapse or expand the sidebar", text::concat(command, "+B"), MenuAct::Sidebar, 0, true});
  entries.push_back({"View", "Refresh now", text::concat(command, "+R"), MenuAct::Refresh, 0, false});
  entries.push_back({"Help", "Usage guide", "F1", MenuAct::Guide, 0, false});
  entries.push_back({"Help", "Keyboard shortcuts", "", MenuAct::Shortcuts, 0, false});
  entries.push_back({"Help", "Diagnostics", "", MenuAct::Diagnostics, 0, false});
  entries.push_back({"Help", text::concat("About ", meta::app_name), "", MenuAct::About, 0, false});
  return entries;
}

View::View(Dashboard& board, Faces faces, float scale)
    : board_(&board),
      faces_(faces),
      scale_(scale),
      tokens_(scaled(board.tokens, scale)),
      ui_(tokens_, faces_, icons_, seen_, scale_) {
  board_->restyled = true;
}

void View::rescale(float scale) {
  if (scale > 0.0F && scale != scale_) {
    scale_ = scale;
    ui_ = Ui(tokens_, faces_, icons_, seen_, scale_);
    board_->restyled = true;
  }
}

const std::vector<Seen>& View::seen() const {
  return seen_;
}

const Seen* View::find(SeenKind kind, std::string_view text) const {
  const auto found =
      std::ranges::find_if(seen_, [&](const Seen& item) { return item.kind == kind && item.text == text; });
  return found == seen_.end() ? nullptr : &*found;
}

bool View::shows(std::string_view text) const {
  return std::ranges::any_of(seen_, [&](const Seen& item) { return text::contains(item.text, text); });
}

Icons& View::icons() {
  return icons_;
}

const theme::Tokens& View::tokens() const {
  return tokens_;
}

float View::table_width() const {
  return table_width_;
}

float View::table_room() const {
  return table_room_;
}

std::uint8_t View::notice_wash(double now) const {
  const double age = now - flash_time_;
  if (tokens_.motion && age >= 0.0 && age < flash_seconds) {
    return static_cast<std::uint8_t>((1.0 - age / flash_seconds) * 80.0);
  }
  return 0;
}

bool View::panel(const char* id, ImVec2 size, ImVec2 padding, std::optional<theme::Color> fill, bool border,
                 ImGuiChildFlags child, ImGuiWindowFlags window) const {
  ImGui::PushStyleVar(ImGuiStyleVar_WindowPadding, padding);
  ImGui::PushStyleColor(ImGuiCol_ChildBg, fill ? color(*fill) : IM_COL32(0, 0, 0, 0));
  ImGui::PushStyleColor(ImGuiCol_Border, color(tokens_.palette.divider));
  ImGuiChildFlags flags = child | ImGuiChildFlags_AlwaysUseWindowPadding;
  if (border) {
    flags |= ImGuiChildFlags_Borders;
  }
  const bool open = ImGui::BeginChild(id, size, flags, window);
  ImGui::PopStyleColor(2);
  ImGui::PopStyleVar();
  return open;
}

bool View::card(const char* id, float height) const {
  const float pad = std::round(tokens_.spacing.m);
  return panel(id, ImVec2(0.0F, height), ImVec2(pad, pad), tokens_.palette.surface, true,
               height > 0.0F ? ImGuiChildFlags_None : ImGuiChildFlags_AutoResizeY);
}

void View::section(std::string_view title) const {
  ui_.space(tokens_.spacing.s);
  ui_.subheading(title);
  ui_.space(tokens_.spacing.xs);
}

void View::prepare() {
  if (board_->restyled) {
    board_->restyled = false;
    tokens_ = scaled(board_->tokens, scale_);
    apply(tokens_);
  }
}

void View::frame() {
  seen_.clear();
  act_ = nullptr;
  keys();
  if (board_->clipboard) {
    ImGui::SetClipboardText(board_->clipboard->c_str());
    board_->clipboard.reset();
  }
  menu_bar();
  const ImGuiViewport* viewport = ImGui::GetMainViewport();
  ImGui::SetNextWindowPos(viewport->WorkPos);
  ImGui::SetNextWindowSize(viewport->WorkSize);
  ImGui::PushStyleVar(ImGuiStyleVar_WindowPadding, ImVec2(0.0F, 0.0F));
  ImGui::PushStyleVar(ImGuiStyleVar_WindowRounding, 0.0F);
  ImGui::PushStyleVar(ImGuiStyleVar_WindowBorderSize, 0.0F);
  ImGui::Begin("##dashboard", nullptr, fixed_window);
  ImGui::PopStyleVar(3);
  const ImVec2 origin = ImGui::GetCursorPos();
  const ImVec2 size = ImGui::GetContentRegionAvail();
  const theme::Spacing& gaps = tokens_.spacing;
  const float top = std::round(std::max(ImGui::GetFrameHeight(), tokens_.type_scale.heading) + gaps.s * 2.0F);
  const float bottom = notice_height(size.x);
  const float middle = std::max(size.y - top - bottom, 0.0F);
  const float side = board_->collapsed ? std::round(ui_.icon_side() + gaps.m * 2.0F + gaps.s * 2.0F)
                                       : std::round(ui_.u(sidebar_width));
  ImGui::SetCursorPos(origin);
  top_bar(size.x, top);
  ImGui::SetCursorPos(ImVec2(origin.x, origin.y + top));
  sidebar(side, middle);
  ImGui::SetCursorPos(ImVec2(origin.x + side, origin.y + top));
  const float pad = std::round(gaps.l);
  if (panel("content", ImVec2(size.x - side, middle), ImVec2(pad, pad), std::nullopt, false)) {
    const std::string_view page = board_->page_id();
    if (page == service::nav_records) {
      records_page();
    } else if (page == service::nav_notifications) {
      inbox_page();
    } else if (page == service::nav_settings) {
      settings_page();
    } else if (page == service::nav_about) {
      about_page();
    } else {
      overview_page();
    }
  }
  ImGui::EndChild();
  if (bottom > 0.0F) {
    ImGui::SetCursorPos(ImVec2(origin.x, origin.y + top + middle));
    notice_bar(size.x, bottom);
  }
  dialogs();
  ImGui::End();
  if (act_) {
    const std::function<void()> act = std::move(act_);
    act_ = nullptr;
    act();
  }
}

void View::keys() {
  const bool typing = ImGui::GetIO().WantTextInput;
  const bool modal = board_->form.has_value() || board_->confirm.has_value();
  std::vector<Shortcut> pressed;
  for (std::size_t index = 0; index < 5; ++index) {
    const auto key = static_cast<ImGuiKey>(static_cast<int>(ImGuiKey_1) + static_cast<int>(index));
    if (ImGui::IsKeyChordPressed(ImGuiMod_Ctrl | key)) {
      pressed.push_back({ShortcutKind::Page, index});
    }
  }
  const std::array<std::pair<ImGuiKey, ShortcutKind>, 6> bound = {{
      {ImGuiKey_F, ShortcutKind::Search},
      {ImGuiKey_Comma, ShortcutKind::Settings},
      {ImGuiKey_N, ShortcutKind::Add},
      {ImGuiKey_R, ShortcutKind::Refresh},
      {ImGuiKey_B, ShortcutKind::Sidebar},
      {ImGuiKey_Q, ShortcutKind::Quit},
  }};
  for (const auto& [key, kind] : bound) {
    if (ImGui::IsKeyChordPressed(ImGuiMod_Ctrl | key)) {
      pressed.push_back({kind, 0});
    }
  }
  if (ImGui::IsKeyChordPressed(ImGuiKey_F1)) {
    pressed.push_back({ShortcutKind::Guide, 0});
  }
  if (!typing && !modal) {
    if (ImGui::IsKeyPressed(ImGuiKey_Delete, false)) {
      pressed.push_back({ShortcutKind::Remove, 0});
    }
    if (ImGui::IsKeyPressed(ImGuiKey_Escape, false)) {
      pressed.push_back({ShortcutKind::Escape, 0});
    }
  }
  for (const Shortcut shortcut : pressed) {
    board_->shortcut(shortcut);
  }
}

void View::choose(const MenuEntry& entry) {
  switch (entry.act) {
    case MenuAct::Terminal:
      board_->open_terminal();
      break;
    case MenuAct::Quit:
      board_->close("");
      break;
    case MenuAct::Page:
      board_->page = entry.page;
      break;
    case MenuAct::Sidebar:
      board_->toggle_sidebar();
      break;
    case MenuAct::Refresh:
      board_->refresh();
      break;
    case MenuAct::Guide:
      board_->show_about(AboutTab::Guide);
      break;
    case MenuAct::Shortcuts:
      board_->show_about(AboutTab::Shortcuts);
      break;
    case MenuAct::Diagnostics:
      board_->show_about(AboutTab::Diagnostics);
      break;
    case MenuAct::About:
      board_->show_about(AboutTab::Overview);
      break;
  }
}

void View::menu_bar() {
  const std::vector<MenuEntry> entries = menu_entries();
  for (const MenuEntry& entry : entries) {
    ui_.note(SeenKind::Choice, text::concat(entry.menu, " > ", entry.label), ImVec2(0.0F, 0.0F), ImVec2(0.0F, 0.0F));
  }
  ImGui::PushStyleVar(ImGuiStyleVar_FramePadding, ImVec2(tokens_.spacing.m, tokens_.spacing.xs));
  ImGui::PushStyleVar(ImGuiStyleVar_WindowBorderSize, 0.0F);
  const bool open = ImGui::BeginMainMenuBar();
  ImGui::PopStyleVar(2);
  if (!open) {
    return;
  }
  const ImVec2 bar = ImGui::GetWindowPos();
  const ImVec2 extent = ImGui::GetWindowSize();
  ImGui::GetWindowDrawList()->AddLine(ImVec2(bar.x, bar.y + extent.y - 0.5F),
                                      ImVec2(bar.x + extent.x, bar.y + extent.y - 0.5F), color(tokens_.palette.divider),
                                      1.0F);
  for (const std::string_view menu : {"Menu", "View", "Help"}) {
    const std::string name(menu);
    const bool unfolded = ImGui::BeginMenu(name.c_str());
    ui_.note(SeenKind::Button, menu);
    if (!unfolded) {
      continue;
    }
    for (const MenuEntry& entry : entries) {
      if (entry.menu != menu) {
        continue;
      }
      if (entry.divider) {
        ImGui::Separator();
      }
      if (ImGui::MenuItem(entry.label.c_str(), entry.keys.empty() ? nullptr : entry.keys.c_str())) {
        choose(entry);
      }
    }
    ImGui::EndMenu();
  }
  ImGui::EndMainMenuBar();
}

void View::top_bar(float width, float height) {
  const theme::Spacing& gaps = tokens_.spacing;
  if (panel("top-bar", ImVec2(width, height), ImVec2(std::round(gaps.l), std::round(gaps.s)), tokens_.palette.surface,
            false, ImGuiChildFlags_None, ImGuiWindowFlags_NoScrollbar)) {
    const ImVec2 corner = ImGui::GetCursorScreenPos();
    const float inner = ImGui::GetContentRegionAvail().y;
    const std::string_view title = service::navigation.at(board_->page).label;
    const float size = tokens_.type_scale.heading;
    ui_.paint_text(ImGui::GetWindowDrawList(), title, ImVec2(corner.x, corner.y + (inner - size) / 2.0F),
                   color(tokens_.palette.text), faces_.medium, size);
    const float title_width = ui_.measure(title, faces_.medium, size).x;
    const std::int64_t unread = board_->overview.unread_notifications;
    const std::string count = unread > 0 ? std::to_string(unread) : std::string();
    const float spacing = ImGui::GetStyle().ItemSpacing.x;
    const float help = ui_.button_width("Help", "info");
    const float bell = ui_.button_width(count, "bell");
    const float chip = board_->overview.demo > 0 ? ui_.button_width("Demo data", "info") + spacing : 0.0F;
    const float room = ImGui::GetContentRegionAvail().x - title_width - gaps.l - help - bell - chip - spacing * 2.0F;
    const float search = std::clamp(room - gaps.l, ui_.u(120.0F), ui_.u(420.0F));
    const float total = search + spacing + chip + bell + spacing + help;
    ImGui::SetCursorPosX(ImGui::GetCursorPosX() + ImGui::GetContentRegionAvail().x - total);
    ImGui::SetCursorPosY(ImGui::GetCursorPosY() + (inner - ImGui::GetFrameHeight()) / 2.0F);
    const bool focus = board_->focus_search;
    board_->focus_search = false;
    const Edited edited = ui_.input("search", search_hint, board_->search, search_limit, search, focus);
    if (edited.entered) {
      act_ = [this] { board_->search_everywhere(); };
    }
    if (board_->overview.demo > 0) {
      ImGui::SameLine();
      if (ui_.button("Demo data", "info", ButtonKind::Plain, true, 0.0F, tokens_.semantic.info)) {
        act_ = [this] {
          board_->show(service::nav_settings);
          board_->say(Tone::Info, std::string(demo_note));
        };
      }
      ui_.tooltip(demo_note);
    }
    ImGui::SameLine();
    ImGui::PushID("bell");
    if (ui_.button(count, "bell")) {
      act_ = [this] { board_->show(service::nav_notifications); };
    }
    ImGui::PopID();
    ui_.tooltip(board_->bell_label());
    ImGui::SameLine();
    if (ui_.button("Help", "info")) {
      act_ = [this] { board_->show_about(AboutTab::Guide); };
    }
    ui_.tooltip("Usage guide (F1)");
  }
  ImGui::EndChild();
}

float View::notice_height(float width) const {
  if (!board_->notice) {
    return 0.0F;
  }
  const theme::Spacing& gaps = tokens_.spacing;
  const float spacing = ImGui::GetStyle().ItemSpacing.x;
  const float room = width - gaps.l * 2.0F - ui_.icon_side() - spacing * 2.0F - ImGui::GetFrameHeight();
  const std::string& text = board_->notice->text;
  const float lines =
      ImGui::CalcTextSize(text.data(), text.data() + text.size(), false, std::max(room, ui_.u(80.0F))).y;
  return std::round(std::max(lines, ImGui::GetFrameHeight()) + gaps.s * 2.0F);
}

void View::notice_bar(float width, float height) {
  if (!board_->notice) {
    return;
  }
  const Message message = *board_->notice;
  const theme::Spacing& gaps = tokens_.spacing;
  const theme::Severity level = severity(message.tone);
  const theme::Color tint = message.tone == Tone::Success ? tokens_.semantic.success : tokens_.semantic.severity(level);
  const double now = ImGui::GetTime();
  if (board_->flash != flash_seen_) {
    flash_seen_ = board_->flash;
    flash_time_ = now;
  }
  const std::uint8_t wash = notice_wash(now);
  if (panel("notice-bar", ImVec2(width, height), ImVec2(std::round(gaps.l), std::round(gaps.s)),
            tokens_.palette.surface, false, ImGuiChildFlags_None, ImGuiWindowFlags_NoScrollbar)) {
    const ImVec2 corner = ImGui::GetWindowPos();
    if (wash > 0) {
      ImGui::GetWindowDrawList()->AddRectFilled(corner, ImVec2(corner.x + width, corner.y + height),
                                                alpha(tokens_.palette.accent, wash));
    }
    const float spacing = ImGui::GetStyle().ItemSpacing.x;
    const float edge = ImGui::GetFrameHeight();
    const float inner = ImGui::GetContentRegionAvail().y;
    const float start = ImGui::GetCursorPosY();
    const float room = ImGui::GetContentRegionAvail().x - ui_.icon_side() - spacing * 2.0F - edge;
    const float lines = ImGui::CalcTextSize(message.text.data(), message.text.data() + message.text.size(), false,
                                            std::max(room, ui_.u(80.0F)))
                            .y;
    ImGui::SetCursorPosY(start + (inner - ImGui::GetTextLineHeight()) / 2.0F);
    ui_.icon(message.tone == Tone::Success ? std::string_view("check") : severity_icon(level), tint);
    ImGui::SameLine();
    ImGui::SetCursorPosY(start + (inner - lines) / 2.0F);
    ui_.wrap_at(ImGui::GetCursorPosX() + std::max(room, ui_.u(80.0F)));
    ui_.wrapped(message.text);
    ui_.wrap_at(0.0F);
    ImGui::SameLine();
    ImGui::SetCursorPos(ImVec2(ImGui::GetWindowWidth() - std::round(gaps.l) - edge, start + (inner - edge) / 2.0F));
    if (ui_.icon_button("dismiss", "x", "Dismiss this message", tokens_.palette.text)) {
      board_->notice.reset();
    }
  }
  ImGui::EndChild();
}

void View::nav_button(std::size_t index) {
  const service::NavItem& item = service::navigation.at(index);
  const bool current = index == board_->page;
  const std::string label = board_->nav_label(item);
  const theme::Spacing& gaps = tokens_.spacing;
  const float width = ImGui::GetContentRegionAvail().x;
  const float height = ui_.row_height();
  const ImVec2 corner = ImGui::GetCursorScreenPos();
  ImGui::PushStyleVar(ImGuiStyleVar_FrameBorderSize, 0.0F);
  ImGui::PushStyleColor(ImGuiCol_Button, current ? color(tokens_.palette.surface_alt) : IM_COL32(0, 0, 0, 0));
  const std::string name = text::concat("##nav-", item.id);
  const bool pressed = ImGui::Button(name.c_str(), ImVec2(width, height));
  ImGui::PopStyleColor();
  ImGui::PopStyleVar();
  ImDrawList* canvas = ImGui::GetWindowDrawList();
  const float side = ui_.icon_side();
  const float left = board_->collapsed ? corner.x + (width - side) / 2.0F : corner.x + gaps.m;
  ui_.paint_icon(canvas, nav_icon(item.id), ImVec2(left, corner.y + (height - side) / 2.0F), side,
                 color(tokens_.palette.text_muted));
  if (!board_->collapsed) {
    const float body = tokens_.type_scale.body;
    ui_.paint_fitted(canvas, label, ImVec2(left + side + gaps.s, corner.y + (height - body) / 2.0F),
                     width - gaps.m * 2.0F - side - gaps.s, color(tokens_.palette.text));
  }
  if (current) {
    const float inset = gaps.m / 2.0F;
    const float thick = ui_.u(nav_marker);
    const ImVec2 from(corner.x - gaps.s + ui_.u(2.0F), corner.y + inset);
    canvas->AddRectFilled(from, ImVec2(from.x + thick, corner.y + height - inset), color(tokens_.palette.accent),
                          thick / 2.0F);
  }
  ui_.note(SeenKind::Button, label, corner, ImVec2(corner.x + width, corner.y + height));
  ui_.tooltip(board_->collapsed ? text::concat(label, ": ", item.hint) : std::string(item.hint));
  if (pressed) {
    board_->page = index;
  }
}

void View::sidebar(float width, float height) {
  const theme::Spacing& gaps = tokens_.spacing;
  const float pad = std::round(gaps.s);
  if (panel("sidebar", ImVec2(width, height), ImVec2(pad, pad), std::nullopt, false, ImGuiChildFlags_None,
            ImGuiWindowFlags_NoScrollbar)) {
    std::size_t low = 0;
    for (std::size_t index = 0; index < service::navigation.size(); ++index) {
      if (service::navigation[index].bottom) {
        ++low;
      } else {
        nav_button(index);
      }
    }
    const float spacing = ImGui::GetStyle().ItemSpacing.y;
    const float block = static_cast<float>(low + 1) * (ui_.row_height() + spacing) + spacing + 1.0F;
    const float wanted = ImGui::GetWindowHeight() - pad - block;
    ImGui::SetCursorPosY(std::max(ImGui::GetCursorPosY(), wanted));
    for (std::size_t index = 0; index < service::navigation.size(); ++index) {
      if (service::navigation[index].bottom) {
        nav_button(index);
      }
    }
    ImGui::Separator();
    const float room = ImGui::GetContentRegionAvail().x;
    const std::string_view hint = board_->collapsed ? "Expand the sidebar" : "Collapse the sidebar to icons";
    ImGui::PushStyleVar(ImGuiStyleVar_FramePadding, ImVec2(board_->collapsed ? (room - ui_.icon_side()) / 2.0F : gaps.m,
                                                           ImGui::GetStyle().FramePadding.y));
    ImGui::PushStyleVar(ImGuiStyleVar_ButtonTextAlign, ImVec2(0.0F, 0.5F));
    if (ui_.button(board_->collapsed ? std::string_view() : std::string_view("Collapse"), "sidebar", ButtonKind::Bare,
                   true, room)) {
      act_ = [this] { board_->toggle_sidebar(); };
    }
    ImGui::PopStyleVar(2);
    ui_.tooltip(hint);
  }
  ImGui::EndChild();
}

void View::dialogs() {
  if (board_->form) {
    form_dialog();
  } else if (board_->confirm) {
    confirm_dialog();
  }
}

void View::form_dialog() {
  Form& form = *board_->form;
  const ImGuiViewport* viewport = ImGui::GetMainViewport();
  const float width = std::min(ui_.u(460.0F), viewport->Size.x - ui_.u(48.0F));
  if (!ImGui::IsPopupOpen("##form-dialog")) {
    ImGui::OpenPopup("##form-dialog");
  }
  ImGui::SetNextWindowPos(viewport->GetCenter(), ImGuiCond_Always, ImVec2(0.5F, 0.5F));
  ImGui::SetNextWindowSize(ImVec2(width + ImGui::GetStyle().WindowPadding.x * 2.0F, 0.0F));
  Verdict verdict = Verdict::Open;
  ImGui::PushStyleColor(ImGuiCol_Border, color(tokens_.palette.border));
  const bool open = ImGui::BeginPopupModal("##form-dialog", nullptr,
                                           ImGuiWindowFlags_NoTitleBar | ImGuiWindowFlags_NoResize |
                                               ImGuiWindowFlags_NoMove | ImGuiWindowFlags_NoSavedSettings);
  ImGui::PopStyleColor();
  if (!open) {
    return;
  }
  ui_.heading(form.title);
  ui_.space(tokens_.spacing.s);
  const bool fresh = form.fresh;
  form.fresh = false;
  ImGui::SetNextWindowSizeConstraints(ImVec2(width, 0.0F),
                                      ImVec2(width, std::max(viewport->Size.y - ui_.u(220.0F), ui_.u(120.0F))));
  if (ImGui::BeginChild("fields", ImVec2(width, 0.0F), ImGuiChildFlags_AutoResizeY)) {
    const float room = ImGui::GetContentRegionAvail().x;
    for (std::size_t index = 0; index < form.fields.size(); ++index) {
      Field& field = form.fields[index];
      ImGui::PushID(static_cast<int>(index));
      ui_.strong(field.label);
      if (!field.options.empty()) {
        ui_.combo("choice", field.shown(), field.options, field.value, std::min(room, ui_.u(420.0F)));
      } else if (field.multiline) {
        ui_.area("area", field.value, field.limit, room, ImGui::GetTextLineHeight() * 4.0F + tokens_.spacing.s * 2.0F);
      } else {
        const Edited edited = ui_.input("text", "", field.value, field.limit, room, fresh && index == 0);
        if (edited.entered) {
          verdict = Verdict::Accepted;
        }
      }
      if (!field.help.empty()) {
        ui_.small(field.help);
      }
      ui_.space(tokens_.spacing.xs);
      ImGui::PopID();
    }
  }
  ImGui::EndChild();
  if (!form.problem.empty()) {
    ui_.colored(text::concat("Not saved: ", form.problem), tokens_.semantic.error);
  }
  ui_.space(tokens_.spacing.s);
  if (ui_.button(form.accept, {}, ButtonKind::Primary)) {
    verdict = Verdict::Accepted;
  }
  ImGui::SameLine();
  if (ui_.button("Cancel")) {
    verdict = Verdict::Cancelled;
  }
  if (verdict == Verdict::Open && ImGui::IsKeyPressed(ImGuiKey_Escape, false)) {
    verdict = Verdict::Cancelled;
  }
  if (verdict != Verdict::Open) {
    ImGui::CloseCurrentPopup();
  }
  ImGui::EndPopup();
  if (verdict == Verdict::Accepted) {
    act_ = [this] { board_->submit(); };
  } else if (verdict == Verdict::Cancelled) {
    act_ = [this] { board_->cancel(); };
  }
}

void View::confirm_dialog() {
  const Confirm& confirm = *board_->confirm;
  const ImGuiViewport* viewport = ImGui::GetMainViewport();
  const float width = std::min(ui_.u(480.0F), viewport->Size.x - ui_.u(48.0F));
  if (!ImGui::IsPopupOpen("##confirm-dialog")) {
    ImGui::OpenPopup("##confirm-dialog");
  }
  ImGui::SetNextWindowPos(viewport->GetCenter(), ImGuiCond_Always, ImVec2(0.5F, 0.5F));
  ImGui::SetNextWindowSize(ImVec2(width + ImGui::GetStyle().WindowPadding.x * 2.0F, 0.0F));
  Verdict verdict = Verdict::Open;
  ImGui::PushStyleColor(ImGuiCol_Border, color(tokens_.palette.border));
  const bool open = ImGui::BeginPopupModal("##confirm-dialog", nullptr,
                                           ImGuiWindowFlags_NoTitleBar | ImGuiWindowFlags_NoResize |
                                               ImGuiWindowFlags_NoMove | ImGuiWindowFlags_NoSavedSettings);
  ImGui::PopStyleColor();
  if (!open) {
    return;
  }
  ui_.heading(confirm.title);
  ui_.space(tokens_.spacing.s);
  ImGui::SetNextWindowSizeConstraints(ImVec2(width, 0.0F),
                                      ImVec2(width, std::max(viewport->Size.y - ui_.u(220.0F), ui_.u(120.0F))));
  if (ImGui::BeginChild("statement", ImVec2(width, 0.0F), ImGuiChildFlags_AutoResizeY)) {
    ui_.wrapped(confirm.statement);
  }
  ImGui::EndChild();
  ui_.space(tokens_.spacing.m);
  if (confirm.action) {
    if (ui_.button(confirm.accept, {}, confirm.danger ? ButtonKind::Danger : ButtonKind::Primary)) {
      verdict = Verdict::Accepted;
    }
    ImGui::SameLine();
  }
  if (ui_.button(confirm.cancel)) {
    verdict = Verdict::Cancelled;
  }
  if (verdict == Verdict::Open && ImGui::IsKeyPressed(ImGuiKey_Escape, false)) {
    verdict = Verdict::Cancelled;
  }
  if (verdict != Verdict::Open) {
    ImGui::CloseCurrentPopup();
  }
  ImGui::EndPopup();
  if (verdict == Verdict::Accepted) {
    act_ = [this] { board_->answer(true); };
  } else if (verdict == Verdict::Cancelled) {
    act_ = [this] { board_->answer(false); };
  }
}

void View::overview_page() {
  const theme::Spacing& gaps = tokens_.spacing;
  const records::Overview& overview = board_->overview;
  struct Figure {
    std::string_view title;
    std::int64_t number = 0;
    std::string_view note;
  };
  const std::array<Figure, 4> figures = {{
      {"Open records", overview.open, "not done and enabled"},
      {"Urgent records", overview.urgent, "open with high or critical severity"},
      {"Unread notifications", overview.unread_notifications, "see Notifications"},
      {"Disabled records", overview.disabled, "kept but not counted"},
  }};
  if (card("figures")) {
    ImGui::PushStyleVar(ImGuiStyleVar_CellPadding, ImVec2(std::round(gaps.xl / 2.0F), 0.0F));
    if (ImGui::BeginTable("figure-columns", static_cast<int>(figures.size()),
                          ImGuiTableFlags_SizingStretchProp | ImGuiTableFlags_NoPadOuterX)) {
      for (std::size_t index = 0; index < figures.size(); ++index) {
        ImGui::TableSetupColumn("", ImGuiTableColumnFlags_WidthStretch, figure_shares[index]);
      }
      ImGui::TableNextRow();
      for (std::size_t index = 0; index < figures.size(); ++index) {
        ImGui::TableSetColumnIndex(static_cast<int>(index));
        const Figure& figure = figures[index];
        ui_.small(figure.title);
        ui_.figure(std::to_string(figure.number), index == 0 ? tokens_.palette.accent : tokens_.palette.text);
        ui_.small(figure.note);
      }
      ImGui::EndTable();
    }
    ImGui::PopStyleVar();
  }
  ImGui::EndChild();
  ui_.space(gaps.l);
  if (overview.total() == 0) {
    ui_.wrapped("No records yet. Open Records and choose Add record, or load labeled demo data to look around.");
  } else {
    const float full = ImGui::GetContentRegionAvail().x;
    const float gap = std::round(gaps.l);
    const float usable = std::max(full - gap, 0.0F);
    const float first = std::round(usable * chart_shares[0] / (chart_shares[0] + chart_shares[1]));
    const float pad = std::round(gaps.m);
    float tallest = 0.0F;
    const float fixed = panel_height_;
    const ImGuiChildFlags sizing = fixed > 0.0F ? ImGuiChildFlags_None : ImGuiChildFlags_AutoResizeY;
    if (panel("tally", ImVec2(first, fixed), ImVec2(pad, pad), tokens_.palette.surface, true, sizing,
              ImGuiWindowFlags_NoScrollbar)) {
      ui_.subheading("Enabled records by severity and status");
      ui_.space(gaps.s);
      const std::int64_t unit = ledger::show(ui_, overview.breakdown);
      ui_.space(gaps.s);
      ui_.small(ledger::caption(unit));
      tallest = std::max(tallest, ImGui::GetCursorPosY());
    }
    ImGui::EndChild();
    ImGui::SameLine(0.0F, gap);
    if (panel("breakdown", ImVec2(usable - first, fixed), ImVec2(pad, pad), tokens_.palette.surface, true, sizing,
              ImGuiWindowFlags_NoScrollbar)) {
      ui_.subheading("The same as a table");
      ui_.space(gaps.s);
      const float space = gaps.s;
      const std::array<table::Spec, 3> specs = {table::flexible(1.0F, ui_.u(96.0F)),
                                                table::flexible(1.0F, ui_.u(96.0F)), table::fixed(ui_.u(64.0F))};
      const std::vector<table::Slot> slots =
          table::layout(specs, ImGui::GetContentRegionAvail().x - space * 2.0F, space);
      const std::array<std::string_view, 3> titles = {"Status", "Severity", "Records"};
      table::header(ui_, slots, titles);
      for (std::size_t index = 0; index < overview.breakdown.size(); ++index) {
        const records::Breakdown& line = overview.breakdown[index];
        const std::vector<table::Cell> cells = {
            table::Cell::marked(status_icon(line.status), tokens_.palette.text_muted,
                                std::string(records::label(line.status))),
            table::Cell::marked(severity_icon(line.severity), tokens_.semantic.severity(line.severity),
                                std::string(theme::label(line.severity))),
            table::Cell::plain(std::to_string(line.total)),
        };
        table::row(ui_, text::concat("breakdown-", index), slots, cells, false, false);
      }
      tallest = std::max(tallest, ImGui::GetCursorPosY());
    }
    ImGui::EndChild();
    panel_height_ = std::round(tallest + pad);
  }
  ui_.space(gaps.l);
  if (overview.demo > 0) {
    ui_.wrapped_muted(text::concat("Demo data is loaded: ", overview.demo,
                                   " sample records whose names start with \"Demo:\". Remove them in Settings, Data, "
                                   "Remove demo data."));
  } else if (ui_.button(text::concat("Load demo data (", records::demo_records().size(), " sample records)"),
                        "download")) {
    act_ = [this] { board_->load_demo(); };
  }
}

}
