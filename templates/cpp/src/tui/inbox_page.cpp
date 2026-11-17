#include <algorithm>
#include <utility>

#include "core/text.hpp"
#include "tui/model.hpp"

namespace app::tui {

namespace {

std::string_view event_label(std::string_view id) {
  const notify::EventType* event = notify::event_type(id);
  return event == nullptr ? id : event->label;
}

std::size_t less(std::size_t value, std::size_t amount) {
  return value > amount ? value - amount : 0;
}

struct Shown {
  std::string_view title;
  std::size_t width = 0;
  bool flexible = false;
};

}

void Model::load_inbox() {
  const std::optional<std::int64_t> previous =
      inbox_.cursor < inbox_.list.size() ? std::optional(inbox_.list[inbox_.cursor].id) : std::nullopt;
  notify::Filter filter;
  filter.unread_only = inbox_.unread_only;
  filter.limit = list_limit;
  std::vector<notify::Notification> list;
  std::vector<notify::Rule> rules;
  if (!attempt([&] {
        list = app_->notifications().list(filter);
        rules = app_->notifications().rules();
      })) {
    return;
  }
  inbox_.list = std::move(list);
  inbox_.rule_list = std::move(rules);
  if (previous) {
    const auto found = std::ranges::find(inbox_.list, *previous, &notify::Notification::id);
    if (found != inbox_.list.end()) {
      inbox_.cursor = static_cast<std::size_t>(found - inbox_.list.begin());
    }
  }
  inbox_.cursor = std::min(inbox_.cursor, less(inbox_.list.size(), 1));
  inbox_.rule_cursor = std::min(inbox_.rule_cursor, less(inbox_.rule_list.size(), 1));
}

void Model::inbox_key(Key key) {
  if (key == keys::right || key.is(U']')) {
    inbox_.rules = true;
  } else if (key == keys::left || key.is(U'[')) {
    inbox_.rules = false;
  } else if (inbox_.rules) {
    rule_key(key);
  } else {
    notification_key(key);
  }
}

void Model::notification_key(Key key) {
  if (move_cursor(inbox_.cursor, key, inbox_.list.size())) {
    return;
  }
  if (key.is(U'u')) {
    inbox_.unread_only = !inbox_.unread_only;
    load_inbox();
    return;
  }
  if (key.is(U'M')) {
    std::int64_t marked = 0;
    if (attempt([&] { marked = app_->notifications().mark_all_read(); })) {
      changed(text::concat("Marked ", marked, " notifications as read"));
    }
    return;
  }
  if (!key.is(U'm') && key != keys::enter) {
    return;
  }
  if (inbox_.cursor >= inbox_.list.size()) {
    return;
  }
  const notify::Notification item = inbox_.list[inbox_.cursor];
  if (item.unread()) {
    if (!attempt([&] { app_->notifications().mark_read(item.id); })) {
      return;
    }
    changed(text::concat("Marked notification ", item.id, " as read"));
  }
  if (key != keys::enter) {
    return;
  }
  if (item.record_id == 0) {
    say(Notice::Info, "The record of this notification was removed");
    return;
  }
  rec_.query = records::Query{};
  rec_.query.sort = "updated";
  rec_.query.descending = true;
  rec_.query.include_disabled = true;
  load_records();
  rec_.select(item.record_id);
  rec_.detail = true;
  open(service::nav_records);
}

void Model::rule_key(Key key) {
  if (move_cursor(inbox_.rule_cursor, key, inbox_.rule_list.size())) {
    return;
  }
  if (inbox_.rule_cursor >= inbox_.rule_list.size()) {
    return;
  }
  const notify::Rule rule = inbox_.rule_list[inbox_.rule_cursor];
  if (key.is(U'u')) {
    if (attempt([&] { app_->notifications().reset_rule(rule.event_type); })) {
      changed(text::concat("Rule for ", rule.event_type, " is back to its defaults"));
    }
    return;
  }
  if (!key.is(U'e') && key != keys::enter) {
    return;
  }
  std::vector<Field> fields = {
      Field::yes_no("enabled", "Enabled", rule.enabled, "Off stops notifications of this event type."),
      Field::choice("severity", "Minimum severity", theme::name(rule.min_severity),
                    "Events below this severity raise no notification.", severity_options()),
      Field::text("start", "Quiet hours start", rule.quiet_start,
                  "Time of day as HH:MM. Leave both times empty for no quiet hours.", 5),
      Field::text("end", "Quiet hours end", rule.quiet_end,
                  "Time of day as HH:MM. May be earlier than the start to span midnight.", 5),
      Field::text("dedup", "De-duplication", std::to_string(rule.dedup_minutes),
                  "Minutes in which the same event for the same record is not raised again. 0 turns it off.", 5),
  };
  form_ = OpenForm{Form(text::concat("Rule for ", event_label(rule.event_type)), std::move(fields)),
                   Submit{SubmitKind::Rule, 0, 0, rule.event_type}};
}

void Model::submit_rule(const Form& form, std::string_view event) {
  const std::optional<std::int64_t> minutes = text::parse_integer(text::trim_whitespace(form.value("dedup")));
  if (!minutes) {
    throw AppError::usage("de-duplication needs a whole number of minutes, for example 60");
  }
  notify::Rule rule;
  rule.event_type = std::string(event);
  rule.enabled = form.value("enabled") == "true";
  rule.min_severity = theme::parse_severity(form.value("severity")).value_or(theme::Severity::Info);
  rule.quiet_start = std::string(text::trim_whitespace(form.value("start")));
  rule.quiet_end = std::string(text::trim_whitespace(form.value("end")));
  rule.dedup_minutes = *minutes;
  rule.customized = true;
  app_->notifications().set_rule(rule);
  changed(text::concat("Saved the rule for ", event));
}

Lines Model::inbox_view(std::size_t size, std::size_t height) const {
  const auto tab = [](std::string_view title, bool active) {
    return active ? Line::styled(text::concat("▌ ", title, " "), Look::Selected)
                  : Line::plain(text::concat("  ", title, "  "));
  };
  Line tabs = tab("Inbox", !inbox_.rules).with(" ", Look::Plain);
  for (Span& span : tab("Rules", inbox_.rules).spans) {
    tabs.spans.push_back(std::move(span));
  }
  Lines lines = {std::move(tabs), Line{}};
  if (inbox_.rules) {
    const std::vector<std::size_t> widths =
        layout({{22, true}, {7, false}, {12, false}, {11, false}, {7, false}, {7, false}}, less(size, 2));
    lines.push_back(Line::styled(
        text::concat("  ", row({"EVENT", "ENABLED", "MIN SEVERITY", "QUIET HOURS", "DEDUP", "SOURCE"}, widths)),
        Look::Muted));
    for (std::size_t index = 0; index < inbox_.rule_list.size(); ++index) {
      const notify::Rule& rule = inbox_.rule_list[index];
      const std::string quiet =
          rule.quiet_start.empty() ? std::string("none") : text::concat(rule.quiet_start, "-", rule.quiet_end);
      const std::vector<std::string> cells = {
          std::string(event_label(rule.event_type)),
          rule.enabled ? "Yes" : "No",
          text::concat(theme::glyph(rule.min_severity), " ", theme::label(rule.min_severity)),
          quiet,
          text::concat(rule.dedup_minutes, " min"),
          rule.customized ? "changed" : "default",
      };
      lines.push_back(selectable(row(cells, widths), index == inbox_.rule_cursor, size));
    }
    if (inbox_.rule_cursor < inbox_.rule_list.size()) {
      if (const notify::EventType* event = notify::event_type(inbox_.rule_list[inbox_.rule_cursor].event_type)) {
        lines.emplace_back();
        extend(lines, wrapped(event->description, size, Look::Plain));
      }
    }
    return lines;
  }
  lines.push_back(Line::styled(text::concat("Showing ", inbox_.unread_only ? "unread only" : "all notifications", " · ",
                                            overview_.unread_notifications, " unread"),
                               Look::Muted));
  lines.emplace_back();
  if (inbox_.list.empty()) {
    extend(lines,
           wrapped(inbox_.unread_only ? "No unread notifications. Press u to show all of them."
                                      : "No notifications. They appear when a record needs attention or is completed.",
                   size, Look::Plain));
    return lines;
  }
  const std::size_t moment = width(app_->format_time(inbox_.list.front().created_at));
  std::vector<Shown> columns;
  for (const Shown& column : {Shown{"ID", 4, false}, Shown{"WHEN", moment, false}, Shown{"SEVERITY", 11, false},
                              Shown{"STATE", 6, false}, Shown{"TITLE", 20, true}, Shown{"LINK", 10, false}}) {
    if ((column.title == "WHEN" && size < 54 + moment) || (column.title == "LINK" && size < 76 + moment)) {
      continue;
    }
    columns.push_back(column);
  }
  std::vector<Column> shape;
  std::vector<std::string> titles;
  for (const Shown& column : columns) {
    shape.push_back({column.width, column.flexible});
    titles.emplace_back(column.title);
  }
  const std::vector<std::size_t> widths = layout(shape, less(size, 2));
  lines.push_back(Line::styled(text::concat("  ", row(titles, widths)), Look::Muted));
  const std::size_t rows = std::max<std::size_t>(less(height, lines.size() + 1), 3);
  const auto [start, end] = window(inbox_.list.size(), inbox_.cursor, rows);
  for (std::size_t index = start; index < end; ++index) {
    const notify::Notification& item = inbox_.list[index];
    std::vector<std::string> cells;
    for (const Shown& column : columns) {
      if (column.title == "ID") {
        cells.push_back(std::to_string(item.id));
      } else if (column.title == "WHEN") {
        cells.push_back(app_->format_time(item.created_at));
      } else if (column.title == "SEVERITY") {
        cells.push_back(text::concat(theme::glyph(item.severity), " ", theme::label(item.severity)));
      } else if (column.title == "STATE") {
        cells.emplace_back(item.unread() ? "Unread" : "Read");
      } else if (column.title == "TITLE") {
        cells.push_back(item.title);
      } else {
        cells.push_back(item.record_id == 0 ? std::string("-") : text::concat(item.record_kind, " ", item.record_id));
      }
    }
    const std::string shown = row(cells, widths);
    if (index != inbox_.cursor && item.unread()) {
      lines.push_back(Line::plain(text::concat("● ", shown)));
    } else {
      lines.push_back(selectable(shown, index == inbox_.cursor, size));
    }
  }
  return lines;
}

}
