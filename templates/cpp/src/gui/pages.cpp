#include <algorithm>
#include <array>
#include <cmath>
#include <span>
#include <string>
#include <string_view>
#include <vector>

#include "core/text.hpp"
#include "gui/table.hpp"
#include "gui/view.hpp"

namespace app::gui {

namespace {

constexpr float detail_width = 340.0F;

table::Spec column_spec(const Ui& ui, std::string_view column) {
  if (column == "id") {
    return table::fixed(ui.u(44.0F));
  }
  if (column == "name") {
    return table::flexible(1.0F, ui.u(220.0F));
  }
  if (column == "status") {
    return table::fixed(ui.u(116.0F));
  }
  if (column == "severity") {
    return table::fixed(ui.u(104.0F));
  }
  if (column == "updated") {
    return table::fixed(ui.u(152.0F));
  }
  return table::fixed(ui.u(80.0F));
}

std::vector<std::string_view> fitting_columns(const Ui& ui, std::span<const std::string> chosen, float room, float gap,
                                              bool detail_open) {
  std::vector<std::string_view> columns(chosen.begin(), chosen.end());
  if (!detail_open) {
    return columns;
  }
  const std::array<std::string_view, 3> in_detail = {"updated", "state", "id"};
  for (const std::string_view column : in_detail) {
    std::vector<table::Spec> specs;
    for (const std::string_view shown : columns) {
      specs.push_back(column_spec(ui, shown));
    }
    if (table::needed(specs, gap) <= room) {
      break;
    }
    std::erase(columns, column);
  }
  return columns;
}

}

std::string sort_label(std::string_view key) {
  if (key == "updated") {
    return "Sort by update";
  }
  return text::concat("Sort by ", key);
}

void View::record_toolbar(records::Query& query) {
  const Edited edited = ui_.input("record-search", "Search names and notes", query.search, search_limit, ui_.u(220.0F));
  static_cast<void>(edited);
  ImGui::SameLine();
  Choices statuses = {{"", "All statuses"}};
  for (const auto& option : status_options()) {
    statuses.push_back(option);
  }
  std::string status = query.status ? std::string(records::name(*query.status)) : std::string();
  const std::string_view status_title = query.status ? records::label(*query.status) : "All statuses";
  if (ui_.combo("record-status", status_title, statuses, status, ui_.u(150.0F))) {
    query.status = status.empty() ? std::nullopt : records::find_status(status);
  }
  ImGui::SameLine();
  Choices levels = {{"", "All severities"}};
  for (const auto& option : severity_options()) {
    levels.push_back(option);
  }
  std::string level = query.severity ? std::string(theme::name(*query.severity)) : std::string();
  const std::string_view level_title = query.severity ? theme::label(*query.severity) : "All severities";
  if (ui_.combo("record-severity", level_title, levels, level, ui_.u(150.0F))) {
    query.severity = level.empty() ? std::nullopt : theme::parse_severity(level);
  }
  ImGui::SameLine();
  Choices sorts;
  for (const std::string_view key : records::sort_keys) {
    sorts.emplace_back(key, sort_label(key));
  }
  ui_.combo("record-sort", sort_label(query.sort), sorts, query.sort, ui_.u(170.0F));
  ImGui::SameLine();
  ui_.checkbox("Descending", query.descending);
  ImGui::SameLine();
  ui_.checkbox("Show disabled", query.include_disabled);
  ui_.space(tokens_.spacing.xs);
  if (ui_.button("Add record", "plus", ButtonKind::Primary)) {
    act_ = [this] { board_->add_record(); };
  }
  ImGui::SameLine();
  if (ui_.button("Import", "upload")) {
    act_ = [this] { board_->import_records(); };
  }
  ImGui::SameLine();
  if (ui_.button("Export", "download")) {
    act_ = [this] { board_->export_records(); };
  }
  ImGui::SameLine();
  if (ui_.button("Columns", "columns")) {
    act_ = [this] { board_->choose_columns(); };
  }
  if (board_->filtered()) {
    ImGui::SameLine();
    if (ui_.button("Clear filters")) {
      act_ = [this] { board_->clear_filters(); };
    }
  }
  ui_.space(tokens_.spacing.s);
}

void View::record_detail(const records::Record& record) {
  ui_.subheading(record.name);
  ui_.space(tokens_.spacing.xs);
  ui_.wrapped(board_->record_details(record));
  ui_.space(tokens_.spacing.s);
  if (record.note.empty()) {
    ui_.muted("No note.");
  } else {
    ImGui::SetNextWindowSizeConstraints(ImVec2(0.0F, 0.0F), ImVec2(ImGui::GetContentRegionAvail().x, ui_.u(160.0F)));
    if (ImGui::BeginChild("record-note", ImVec2(0.0F, 0.0F), ImGuiChildFlags_AutoResizeY)) {
      ui_.wrapped(record.note);
    }
    ImGui::EndChild();
  }
  if (record.demo) {
    ui_.space(tokens_.spacing.xs);
    ui_.muted("Demo record");
  }
  ui_.space(tokens_.spacing.m);
  const float edge = ImGui::GetContentRegionAvail().x;
  const float spacing = ImGui::GetStyle().ItemSpacing.x;
  float used = 0.0F;
  const auto place = [&](float width) {
    if (used > 0.0F && used + spacing + width <= edge) {
      ImGui::SameLine();
      used += spacing + width;
    } else {
      used = width;
    }
  };
  place(ui_.button_width("Edit", "pencil"));
  if (ui_.button("Edit", "pencil")) {
    act_ = [this] { board_->edit_record(); };
  }
  const std::string_view toggle = record.enabled ? "Disable" : "Enable";
  place(ui_.button_width(toggle, "circle-minus"));
  if (ui_.button(toggle, "circle-minus")) {
    act_ = [this] { board_->toggle_record(); };
  }
  place(ui_.button_width("Copy", "copy"));
  if (ui_.button("Copy", "copy")) {
    act_ = [this] { board_->copy_record(); };
  }
  place(ui_.button_width("Remove"));
  if (ui_.button("Remove", {}, ButtonKind::Danger)) {
    act_ = [this] { board_->remove_record(); };
  }
  place(ui_.button_width("Close"));
  if (ui_.button("Close")) {
    act_ = [this] { board_->rec.selected.reset(); };
  }
}

void View::record_table() {
  const RecordsPage& page = board_->rec;
  if (page.list.empty()) {
    ui_.wrapped(board_->empty_records_text());
    return;
  }
  const float gap = tokens_.spacing.s;
  const ImVec2 room = ImGui::GetContentRegionAvail();
  const std::vector<std::string_view> columns =
      fitting_columns(ui_, page.columns, room.x, gap, board_->selected_record() != nullptr);
  std::vector<table::Spec> specs;
  std::vector<std::string_view> titles;
  for (const std::string_view column : columns) {
    specs.push_back(column_spec(ui_, column));
    titles.push_back(records::column_label(column));
  }
  const float footer = ImGui::GetTextLineHeight() + ImGui::GetStyle().ItemSpacing.y;
  const float width = std::max(room.x, table::needed(specs, gap));
  table_width_ = width;
  table_room_ = room.x;
  ImGui::SetNextWindowContentSize(ImVec2(width, 0.0F));
  if (ImGui::BeginChild("record-table", ImVec2(room.x, std::max(room.y - footer, ui_.row_height() * 2.0F)),
                        ImGuiChildFlags_None, ImGuiWindowFlags_HorizontalScrollbar)) {
    const std::vector<table::Slot> slots = table::layout(specs, width - gap * 2.0F, gap);
    table::header(ui_, slots, titles);
    if (ImGui::BeginChild("record-rows", ImVec2(width, 0.0F))) {
      ImGuiListClipper clipper;
      clipper.Begin(static_cast<int>(page.list.size()), ui_.row_height());
      while (clipper.Step()) {
        for (int index = clipper.DisplayStart; index < clipper.DisplayEnd; ++index) {
          const records::Record& record = page.list[static_cast<std::size_t>(index)];
          std::vector<table::Cell> cells;
          for (const std::string_view column : columns) {
            if (column == "id") {
              cells.push_back(table::Cell::muted(std::to_string(record.id)));
            } else if (column == "name") {
              cells.push_back(table::Cell::plain(record.name));
            } else if (column == "status") {
              cells.push_back(table::Cell::marked(status_icon(record.status), tokens_.palette.text_muted,
                                                  std::string(records::label(record.status))));
            } else if (column == "severity") {
              cells.push_back(table::Cell::marked(severity_icon(record.severity),
                                                  tokens_.semantic.severity(record.severity),
                                                  std::string(theme::label(record.severity))));
            } else if (column == "updated") {
              cells.push_back(table::Cell::muted(board_->app().format_time(record.updated_at)));
            } else {
              cells.push_back(table::Cell::plain(std::string(enabled_label(record.enabled))));
            }
          }
          const bool selected = page.selected == record.id;
          if (table::row(ui_, text::concat("record-", record.id), slots, cells, selected).once) {
            const std::int64_t id = record.id;
            act_ = [this, id, selected] {
              if (selected) {
                board_->rec.selected.reset();
              } else {
                board_->rec.selected = id;
              }
            };
          }
        }
      }
    }
    ImGui::EndChild();
  }
  ImGui::EndChild();
  ui_.muted(text::concat(page.list.size(), " records shown"));
}

void View::records_page() {
  records::Query query = board_->rec.query;
  record_toolbar(query);
  const records::Record* record = board_->selected_record();
  const ImVec2 room = ImGui::GetContentRegionAvail();
  const float gap = std::round(tokens_.spacing.m);
  const float detail = record == nullptr ? 0.0F : std::min(ui_.u(detail_width), room.x * 0.5F);
  const float list = record == nullptr ? room.x : room.x - detail - gap;
  if (ImGui::BeginChild("record-list", ImVec2(list, room.y))) {
    record_table();
  }
  ImGui::EndChild();
  if (record != nullptr) {
    ImGui::SameLine(0.0F, gap);
    if (panel("record-detail", ImVec2(detail, room.y), ImVec2(gap, gap), tokens_.palette.surface, true)) {
      record_detail(*record);
    }
    ImGui::EndChild();
  }
  if (query.sort != board_->rec.query.sort || query.descending != board_->rec.query.descending) {
    const std::string key = query.sort;
    const bool descending = query.descending;
    board_->rec.query = std::move(query);
    board_->sort_by(key, descending);
  } else if (query != board_->rec.query) {
    board_->rec.query = std::move(query);
    board_->load_records();
  }
}

void View::inbox_list() {
  const InboxPage& page = board_->inbox;
  bool unread = page.unread_only;
  if (ui_.checkbox("Unread only", unread)) {
    act_ = [this, unread] {
      board_->inbox.unread_only = unread;
      board_->load_inbox();
    };
  }
  ImGui::SameLine();
  if (ui_.button("Mark as read", "check")) {
    act_ = [this] { board_->mark_read(); };
  }
  ImGui::SameLine();
  ImGui::PushID("all");
  if (ui_.button("Mark all as read", "check")) {
    act_ = [this] { board_->mark_all_read(); };
  }
  ImGui::PopID();
  ImGui::SameLine();
  if (ui_.button("Open record", "arrow-right")) {
    act_ = [this] { board_->open_notification_record(); };
  }
  ui_.space(tokens_.spacing.s);
  if (page.list.empty()) {
    ui_.wrapped(board_->empty_inbox_text());
    return;
  }
  const float gap = tokens_.spacing.s;
  const ImVec2 room = ImGui::GetContentRegionAvail();
  const float footer = ImGui::GetTextLineHeight() + ImGui::GetStyle().ItemSpacing.y;
  const std::array<table::Spec, 5> specs = {table::fixed(ui_.u(112.0F)), table::flexible(3.0F, ui_.u(160.0F)),
                                            table::flexible(1.0F, ui_.u(130.0F)), table::flexible(1.0F, ui_.u(150.0F)),
                                            table::fixed(ui_.u(64.0F))};
  const std::vector<table::Slot> slots = table::layout(specs, room.x - gap * 2.0F, gap);
  const std::array<std::string_view, 5> titles = {"Severity", "Notification", "Event", "Raised", "State"};
  table::header(ui_, slots, titles);
  const float rest = ImGui::GetContentRegionAvail().y - footer;
  if (ImGui::BeginChild("inbox-rows", ImVec2(room.x, std::max(rest, ui_.row_height())))) {
    ImGuiListClipper clipper;
    clipper.Begin(static_cast<int>(page.list.size()), ui_.row_height());
    while (clipper.Step()) {
      for (int index = clipper.DisplayStart; index < clipper.DisplayEnd; ++index) {
        const notify::Notification& item = page.list[static_cast<std::size_t>(index)];
        const std::vector<table::Cell> cells = {
            table::Cell::marked(severity_icon(item.severity), tokens_.semantic.severity(item.severity),
                                std::string(theme::label(item.severity))),
            table::Cell::plain(item.title),
            table::Cell::muted(std::string(event_label(item.event_type))),
            table::Cell::muted(board_->app().format_time(item.created_at)),
            item.unread() ? table::Cell::plain("Unread") : table::Cell::muted("Read"),
        };
        const table::Click click =
            table::row(ui_, text::concat("notification-", item.id), slots, cells, page.selected == item.id);
        const std::int64_t id = item.id;
        if (click.twice) {
          act_ = [this, id] {
            board_->inbox.selected = id;
            board_->open_notification_record();
          };
        } else if (click.once) {
          act_ = [this, id] { board_->inbox.selected = id; };
        }
      }
    }
  }
  ImGui::EndChild();
  ui_.muted(board_->inbox_summary());
}

void View::inbox_rules() {
  const InboxPage& page = board_->inbox;
  if (ui_.button("Edit rule", "pencil")) {
    act_ = [this] { board_->edit_rule(); };
  }
  ImGui::SameLine();
  if (ui_.button("Reset to defaults")) {
    act_ = [this] { board_->reset_rule(); };
  }
  ui_.muted("Select an event type to change its rule.");
  ui_.space(tokens_.spacing.s);
  const float gap = tokens_.spacing.s;
  const std::array<table::Spec, 6> specs = {table::flexible(2.0F, ui_.u(180.0F)), table::fixed(ui_.u(80.0F)),
                                            table::fixed(ui_.u(130.0F)),          table::fixed(ui_.u(130.0F)),
                                            table::fixed(ui_.u(120.0F)),          table::fixed(ui_.u(90.0F))};
  const std::vector<table::Slot> slots = table::layout(specs, ImGui::GetContentRegionAvail().x - gap * 2.0F, gap);
  const std::array<std::string_view, 6> titles = {"Event type",  "Enabled",        "Minimum severity",
                                                  "Quiet hours", "De-duplication", "Source"};
  table::header(ui_, slots, titles);
  if (ImGui::BeginChild("rule-rows")) {
    for (const notify::Rule& rule : page.rule_list) {
      const std::string quiet = rule.quiet_start.empty() && rule.quiet_end.empty()
                                    ? std::string("None")
                                    : text::concat(rule.quiet_start, " to ", rule.quiet_end);
      const std::string dedup = rule.dedup_minutes == 0 ? std::string("Off") : text::concat(rule.dedup_minutes, " min");
      const std::vector<table::Cell> cells = {
          table::Cell::plain(std::string(event_label(rule.event_type))),
          table::Cell::plain(rule.enabled ? "Yes" : "No"),
          table::Cell::marked(severity_icon(rule.min_severity), tokens_.semantic.severity(rule.min_severity),
                              std::string(theme::label(rule.min_severity))),
          table::Cell::muted(quiet),
          table::Cell::muted(dedup),
          table::Cell::muted(rule.customized ? "Saved" : "Default"),
      };
      const bool selected = page.rule && *page.rule == rule.event_type;
      const table::Click click = table::row(ui_, text::concat("rule-", rule.event_type), slots, cells, selected);
      const std::string event = rule.event_type;
      if (click.twice) {
        act_ = [this, event] {
          board_->inbox.rule = event;
          board_->edit_rule();
        };
      } else if (click.once) {
        act_ = [this, event] { board_->inbox.rule = event; };
      }
    }
  }
  ImGui::EndChild();
}

void View::inbox_page() {
  if (ui_.tab("Inbox", !board_->inbox.rules)) {
    act_ = [this] { board_->inbox.rules = false; };
  }
  ImGui::SameLine();
  if (ui_.tab("Rules", board_->inbox.rules)) {
    act_ = [this] { board_->inbox.rules = true; };
  }
  ui_.separator();
  ui_.space(tokens_.spacing.xs);
  if (board_->inbox.rules) {
    inbox_rules();
  } else {
    inbox_list();
  }
}

}
