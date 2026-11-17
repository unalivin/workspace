#include <algorithm>
#include <cmath>

#include "core/guide.hpp"
#include "core/meta.hpp"
#include "core/text.hpp"
#include "core/userconfig.hpp"
#include "gui/table.hpp"
#include "gui/view.hpp"

namespace app::gui {

namespace {

constexpr float control_width = 240.0F;
constexpr float label_width = 300.0F;
constexpr float guide_width = 280.0F;
constexpr float logo_side = 64.0F;

}

std::string option_label(std::string_view key, std::string_view option) {
  if (key == settings::key_theme) {
    const std::string_view label = theme::label(option);
    return std::string(label.empty() ? option : label);
  }
  std::string shown(option);
  if (!shown.empty() && shown.front() >= 'a' && shown.front() <= 'z') {
    shown.front() = static_cast<char>(shown.front() - 'a' + 'A');
  }
  return shown;
}

void View::setting_row(std::string_view title, std::string_view help, const std::function<void()>& control) {
  const float room = ImGui::GetContentRegionAvail().x;
  const float width = std::round(std::min(ui_.u(label_width), room * 0.45F));
  const float gap = std::round(tokens_.spacing.m);
  const std::string name = text::concat("row-", title);
  ImGui::PushID(name.c_str());
  ImGui::BeginGroup();
  ui_.wrap_at(ImGui::GetCursorPosX() + width);
  ui_.strong(title);
  if (!help.empty()) {
    ui_.small(help);
  }
  ui_.wrap_at(0.0F);
  ImGui::Dummy(ImVec2(width, 0.0F));
  ImGui::EndGroup();
  ImGui::SameLine(0.0F, gap);
  ImGui::BeginGroup();
  control();
  ImGui::EndGroup();
  ImGui::PopID();
  ui_.space(tokens_.spacing.s);
}

void View::setting_control(const settings::Value& value) {
  const std::string& key = value.key;
  const std::string saved(saved_value(value));
  switch (value.definition.kind) {
    case settings::Kind::Bool: {
      bool on = saved == "true";
      if (ui_.checkbox(on ? "On" : "Off", on)) {
        act_ = [this, key, on] { board_->save_setting(key, on ? "true" : "false"); };
      }
      break;
    }
    case settings::Kind::Enum: {
      Choices options;
      for (const std::string_view option : value.definition.options) {
        options.emplace_back(option, option_label(key, option));
      }
      std::string chosen = saved;
      if (ui_.combo("choice", option_label(key, saved), options, chosen, ui_.u(control_width)) && chosen != saved) {
        act_ = [this, key, chosen] { board_->save_setting(key, chosen); };
      }
      break;
    }
    default: {
      const bool tinted = value.definition.kind == settings::Kind::Color;
      std::string& draft = board_->set.drafts.try_emplace(key, saved).first->second;
      if (tinted) {
        const theme::Color shown = theme::Color::parse(draft).value_or(tokens_.palette.accent);
        const float side = ui_.u(18.0F);
        const float height = ImGui::GetFrameHeight();
        const ImVec2 corner = ImGui::GetCursorScreenPos();
        ImGui::Dummy(ImVec2(side, height));
        const ImVec2 from(corner.x, corner.y + (height - side) / 2.0F);
        const ImVec2 to(from.x + side, from.y + side);
        ImDrawList* canvas = ImGui::GetWindowDrawList();
        canvas->AddRectFilled(from, to, color(shown), tokens_.radius);
        canvas->AddRect(from, to, color(tokens_.palette.border), tokens_.radius);
        ImGui::SameLine();
      }
      const Edited edited = ui_.input("draft", tinted ? "#RRGGBB, empty for the theme accent" : "", draft, 200,
                                      ui_.u(control_width - 60.0F));
      const bool dirty = draft != saved;
      if (edited.changed) {
        if (dirty) {
          board_->set.dirty.insert(key);
        } else {
          board_->set.dirty.erase(key);
        }
      }
      ImGui::SameLine();
      const bool pressed = ui_.button("Save", {}, ButtonKind::Plain, dirty);
      if ((pressed || edited.entered) && dirty) {
        const std::string raw = draft;
        act_ = [this, key, raw] { board_->save_setting(key, raw); };
      }
      break;
    }
  }
  ImGui::SameLine();
  if (ui_.button("Reset", {}, ButtonKind::Plain, value.stored.has_value())) {
    act_ = [this, key] { board_->reset_setting(key); };
  }
  ui_.tooltip("Back to the default");
  const std::string note = board_->setting_note(value);
  std::string source = note.empty() ? option_label("", settings::label(value.source)) : note;
  if (value.definition.kind == settings::Kind::Int) {
    source = text::concat(source, ". Whole number from ", value.definition.min, " to ", value.definition.max);
  }
  ui_.small(source);
  const auto problem = board_->set.problems.find(key);
  if (problem != board_->set.problems.end()) {
    ui_.colored(text::concat("Not saved: ", problem->second), tokens_.semantic.error);
  }
}

void View::special_rows(std::string_view group) {
  if (group == "Data") {
    setting_row("Database location", board_->database_note(), [this] {
      ui_.mono(board_->database_path(), true);
      if (ui_.button("Change…", {}, ButtonKind::Plain, !board_->database_locked())) {
        act_ = [this] { board_->change_database(); };
      }
    });
    setting_row("Remove demo data",
                "Removes the sample records whose names start with \"Demo:\" after confirmation. Your own records "
                "are not touched.",
                [this] {
                  ui_.text(text::concat(board_->overview.demo, " demo records"));
                  if (ui_.button("Remove demo data", {}, ButtonKind::Danger, board_->overview.demo > 0)) {
                    act_ = [this] { board_->remove_demo(); };
                  }
                });
    return;
  }
  if (group == "Notifications") {
    const NotifierState& state = board_->set.notifier;
    std::string shown;
    std::string_view button;
    switch (state.kind) {
      case NotifierKind::Missing:
        shown = notifier_missing;
        break;
      case NotifierKind::Unknown:
        shown = state.detail;
        break;
      case NotifierKind::On:
        shown = text::concat("On. Entry: ", state.detail);
        button = "Turn off";
        break;
      case NotifierKind::Off:
        shown = notifier_off;
        button = "Turn on";
        break;
    }
    setting_row("Start at sign-in", "The notifier shows system notifications while the application is closed.",
                [this, &shown, button] {
                  ui_.wrapped(shown);
                  if (!button.empty() && ui_.button(button)) {
                    act_ = [this] { board_->toggle_notifier(); };
                  }
                });
    return;
  }
  if (group == "Launcher") {
    setting_row("Open with", "Which front end opens when the application starts without a command.", [this] {
      const std::string current = board_->front_end();
      std::string chosen = current;
      Choices modes;
      for (const std::string_view mode : {std::string_view(), userconfig::front_end_gui, userconfig::front_end_tui}) {
        modes.emplace_back(mode, front_end_label(mode));
      }
      if (ui_.combo("open-with", front_end_label(current), modes, chosen, ui_.u(control_width)) && chosen != current) {
        act_ = [this, chosen] { board_->remember(chosen); };
      }
      if (const std::optional<std::string> note = board_->launcher_override()) {
        ui_.small(*note);
      }
    });
  }
}

void View::settings_page() {
  for (const std::string& group : board_->set.groups) {
    section(group);
    const std::string name = text::concat("group-", group);
    if (card(name.c_str())) {
      for (const settings::Value& value : board_->set.list) {
        if (value.definition.group != group) {
          continue;
        }
        const std::string_view title =
            value.key == settings::key_accent ? std::string_view("Custom accent") : value.definition.label;
        setting_row(title, value.definition.help, [this, &value] { setting_control(value); });
      }
      special_rows(group);
    }
    ImGui::EndChild();
  }
}

void View::about_overview() {
  const float side = ui_.u(logo_side);
  const ImVec2 corner = ImGui::GetCursorScreenPos();
  ImGui::Dummy(ImVec2(side, side));
  icons_.logo(ImGui::GetWindowDrawList(), corner, side);
  ui_.note(SeenKind::Icon, "logo", corner, ImVec2(corner.x + side, corner.y + side));
  ImGui::SameLine();
  ImGui::BeginGroup();
  ui_.heading(text::concat(meta::app_name, " ", meta::version()));
  ui_.wrapped(meta::tagline);
  ImGui::EndGroup();
  ui_.space(tokens_.spacing.m);
  ui_.text(text::concat("Publisher: ", meta::publisher));
  ui_.text(text::concat("License: ", meta::license));
  ui_.space(tokens_.spacing.s);
  const service::Link source = service::source_link();
  if (ui_.button(source.label)) {
    act_ = [this] { board_->follow(service::source_link()); };
  }
  ImGui::SameLine();
  ui_.muted(source.url);
  if (ui_.button("Found a bug? Please report it on GitHub.")) {
    act_ = [this] { board_->report_bug(); };
  }
  ui_.muted("The exact text of the report is shown before your browser opens.");
  const service::Link advisory = service::advisory_link();
  if (ui_.button("Report a vulnerability privately")) {
    act_ = [this] { board_->follow(service::advisory_link()); };
  }
  ui_.tooltip(advisory.url);
  ui_.space(tokens_.spacing.m);
  ui_.wrapped_muted("The application makes no network requests. Links open in your browser only when you choose them.");
}

void View::about_guide() {
  if (ui_.input("guide-search", "Search the guide", board_->about.search, 100, ui_.u(320.0F)).changed) {
    act_ = [this] { board_->filter_guide(); };
  }
  ui_.space(tokens_.spacing.s);
  const AboutPage& page = board_->about;
  if (page.hits.empty()) {
    ui_.wrapped("No section mentions these words. Clear the search box to see the contents.");
    return;
  }
  const std::optional<std::size_t> shown = page.section ? page.section : std::optional(page.hits.front());
  const float gap = std::round(tokens_.spacing.m);
  const ImVec2 room = ImGui::GetContentRegionAvail();
  const float contents = std::min(ui_.u(guide_width), room.x * 0.45F);
  if (ImGui::BeginChild("guide-contents", ImVec2(contents, room.y))) {
    for (const std::size_t index : page.hits) {
      const guide::Section& entry = page.usage.sections.at(index);
      const std::size_t level = entry.level == 0 ? 0 : entry.level - 1;
      const auto depth = static_cast<float>(std::min<std::size_t>(level, 3));
      if (ui_.entry(text::concat("section-", index), entry.title, shown == index, depth * tokens_.spacing.m)) {
        act_ = [this, index] { board_->about.section = index; };
      }
    }
  }
  ImGui::EndChild();
  ImGui::SameLine(0.0F, gap * 2.0F);
  if (ImGui::BeginChild("guide-section", ImVec2(0.0F, room.y))) {
    if (shown && *shown < page.usage.sections.size()) {
      const guide::Section& entry = page.usage.sections[*shown];
      ui_.subheading(entry.title);
      ui_.space(tokens_.spacing.xs);
      ui_.wrapped(guide::plain(entry.body));
    }
  }
  ImGui::EndChild();
}

void View::about_shortcuts() {
  const float gap = tokens_.spacing.s;
  const float width = std::min(ImGui::GetContentRegionAvail().x, ui_.u(720.0F));
  const std::array<table::Spec, 2> specs = {table::fixed(ui_.u(180.0F)), table::flexible(1.0F, ui_.u(200.0F))};
  const std::vector<table::Slot> slots = table::layout(specs, width - gap * 2.0F, gap);
  const std::array<std::string_view, 2> titles = {"Keys", "Action"};
  if (ImGui::BeginChild("shortcut-rows", ImVec2(width, 0.0F))) {
    table::header(ui_, slots, titles);
    for (const service::Shortcut& shortcut : service::shortcuts) {
      const std::vector<table::Cell> cells = {table::Cell::plain(std::string(shortcut.keys)),
                                              table::Cell::muted(std::string(shortcut.action))};
      table::row(ui_, text::concat("shortcut-", shortcut.keys), slots, cells, false, false);
    }
  }
  ImGui::EndChild();
}

void View::about_diagnostics() {
  if (ui_.button("Copy diagnostics", "copy")) {
    act_ = [this] { board_->copy_diagnostics(); };
  }
  ImGui::SameLine();
  if (ui_.button("Check the database")) {
    act_ = [this] { board_->check_database(); };
  }
  ui_.muted("Diagnostics stay on this computer. A bug report includes only the lines shown in its preview.");
  ui_.space(tokens_.spacing.s);
  if (ImGui::BeginChild("diagnostics-text", ImVec2(0.0F, 0.0F), ImGuiChildFlags_None,
                        ImGuiWindowFlags_HorizontalScrollbar)) {
    ui_.mono(board_->diagnostics_text(), false);
  }
  ImGui::EndChild();
}

void View::about_page() {
  bool first = true;
  for (const AboutTabName& entry : about_tabs) {
    if (!first) {
      ImGui::SameLine();
    }
    first = false;
    if (ui_.tab(entry.label, board_->about.tab == entry.tab)) {
      const AboutTab tab = entry.tab;
      act_ = [this, tab] { board_->about.tab = tab; };
    }
  }
  ui_.separator();
  ui_.space(tokens_.spacing.xs);
  switch (board_->about.tab) {
    case AboutTab::Overview:
      about_overview();
      break;
    case AboutTab::Guide:
      about_guide();
      break;
    case AboutTab::Shortcuts:
      about_shortcuts();
      break;
    case AboutTab::Diagnostics:
      about_diagnostics();
      break;
    case AboutTab::Licenses:
      if (ImGui::BeginChild("license-text")) {
        ui_.mono(meta::license_text(), true);
        ui_.separator();
        ui_.mono(meta::notices_text(), true);
      }
      ImGui::EndChild();
      break;
  }
}

}
