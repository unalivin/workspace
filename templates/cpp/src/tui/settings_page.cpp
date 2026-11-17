#include <algorithm>
#include <utility>

#include "core/files.hpp"
#include "core/meta.hpp"
#include "core/text.hpp"
#include "core/userconfig.hpp"
#include "tui/model.hpp"

namespace app::tui {

namespace {

constexpr std::string_view notifier_help =
    "Enter turns the per-user sign-in entry on or off. The notifier shows system notifications while the "
    "application is closed. It needs no administrator rights and makes no network requests.";
constexpr std::string_view launcher_help =
    "Which front end opens when the application starts without a command. Enter changes it, u asks again at every "
    "start.";
constexpr std::string_view demo_help =
    "Enter removes the sample records whose names start with \"Demo:\" after confirmation. Your own records are "
    "not touched.";
constexpr std::string_view notifier_title = "Notifier at sign-in";

std::string shown_value(const settings::Value& value) {
  std::string_view shown = value.raw;
  if (value.raw == "true") {
    shown = "Yes";
  } else if (value.raw == "false") {
    shown = "No";
  } else if (value.key == settings::key_theme) {
    shown = theme::label(value.raw);
  }
  return shown.empty() ? std::string("(empty)") : std::string(shown);
}

std::string_view saved_value(const settings::Value& value) {
  return value.stored ? std::string_view(*value.stored) : value.definition.fallback;
}

std::string example_database_path() {
  if (platform::current_os() == platform::Os::Windows) {
    return text::concat("D:\\Data\\", meta::slug, ".db");
  }
  return text::concat("/home/you/data/", meta::slug, ".db");
}

std::size_t less(std::size_t value, std::size_t amount) {
  return value > amount ? value - amount : 0;
}

}

std::string_view front_end_label(std::string_view mode) {
  if (mode == userconfig::front_end_gui) {
    return "Dashboard window";
  }
  if (mode == userconfig::front_end_tui) {
    return "Terminal UI";
  }
  return "Ask every time";
}

void Model::load_settings() {
  std::vector<settings::Value> list;
  if (!attempt([&] { list = app_->settings().list(); })) {
    return;
  }
  std::vector<SettingRow> rows;
  for (const settings::Value& value : list) {
    std::string source(settings::label(value.source));
    std::string help(value.definition.help);
    if (!value.overridden_by.empty()) {
      source = "overridden";
      help.append(text::concat(" Overridden by the ", value.overridden_by, ". The saved value is \"",
                               saved_value(value), "\" and applies when the override is removed."));
    }
    if (!value.override_error.empty()) {
      help.push_back(' ');
      help.append(value.override_error);
    }
    rows.push_back({std::string(value.definition.group), std::string(value.definition.label), shown_value(value),
                    std::move(source), std::move(help), value.key, RowAction::None});
  }
  const service::Location& location = app_->location();
  const std::string path = files::shown(location.path);
  if (location.overridden_by.empty()) {
    rows.push_back({"Data", "Database location", path, std::string(location.source),
                    text::concat("In use: ", path,
                                 ". Enter copies the database to a new place and uses it, or switches to another "
                                 "database. The old file is never deleted."),
                    "", RowAction::Database});
  } else {
    rows.push_back({"Data", "Database location", path, "overridden",
                    text::concat("In use: ", path, ". Set by the ", location.overridden_by,
                                 ". Remove the override to change the location here."),
                    "", RowAction::Database});
  }
  rows.push_back({"Data", "Remove demo data", text::concat(overview_.demo, " demo records"), "action",
                  std::string(demo_help), "", RowAction::Demo});
  std::string notifier = "Not available";
  std::string notifier_note;
  if (options_.autostart) {
    try {
      const notifier::Placement placement = options_.autostart->status();
      notifier = placement.present ? "On" : "Off";
      if (placement.present) {
        notifier_note = text::concat("Entry: ", files::shown(placement.path), ". ");
      }
    } catch (const AppError& error) {
      notifier = "Unknown";
      notifier_note = text::concat(error.what(), " ");
    }
  }
  rows.push_back({"Notifications", std::string(notifier_title), std::move(notifier), "action",
                  text::concat(notifier_note, notifier_help), "", RowAction::Notifier});
  const std::string variable = meta::env_name("ui");
  const std::string& front_end = location.config.front_end;
  const std::optional<std::string> forced = options_.env.get(variable);
  std::string launcher_source = front_end.empty() ? "default" : "saved";
  std::string launcher_note;
  if (forced && !forced->empty()) {
    launcher_source = "overridden";
    launcher_note = text::concat("Overridden by the environment variable ", variable, "=", *forced, ". ");
  }
  rows.push_back({"Launcher", "Open with", std::string(front_end_label(front_end)), std::move(launcher_source),
                  text::concat(launcher_note, launcher_help), "", RowAction::Launcher});
  std::vector<std::string> groups;
  for (const std::string_view group : settings::groups(list)) {
    groups.emplace_back(group);
  }
  for (const std::string_view extra : {"Data", "Notifications", "Launcher"}) {
    if (std::ranges::find(groups, extra) == groups.end()) {
      groups.emplace_back(extra);
    }
  }
  std::vector<SettingRow> ordered;
  ordered.reserve(rows.size());
  for (const std::string& group : groups) {
    for (const SettingRow& row : rows) {
      if (row.group == group) {
        ordered.push_back(row);
      }
    }
  }
  set_.cursor = std::min(set_.cursor, less(ordered.size(), 1));
  set_.rows = std::move(ordered);
}

void Model::settings_key(Key key) {
  if (move_cursor(set_.cursor, key, set_.rows.size())) {
    return;
  }
  if (set_.cursor >= set_.rows.size()) {
    return;
  }
  const SettingRow row = set_.rows[set_.cursor];
  if (!row.key.empty()) {
    if (key == keys::enter) {
      edit_setting(row.key);
    } else if (key.is(U'u')) {
      std::optional<settings::Value> value;
      if (!attempt([&] { value = app_->settings().reset(row.key); })) {
        return;
      }
      const std::string_view label = value->definition.label;
      changed(text::concat(label, " is back to its default"));
      if (!value->overridden_by.empty()) {
        say(Notice::Warning,
            text::concat(label, " is back to its default, but the ", value->overridden_by, " still takes precedence"));
      }
    }
    return;
  }
  if (row.action == RowAction::Launcher && (key == keys::enter || key.is(U'u'))) {
    std::string_view target;
    if (key == keys::enter) {
      const std::string& current = app_->location().config.front_end;
      if (current.empty()) {
        target = userconfig::front_end_gui;
      } else if (current == userconfig::front_end_gui) {
        target = userconfig::front_end_tui;
      }
    }
    if (attempt([&] { app_->remember_front_end(target); })) {
      changed(text::concat("Open with: ", front_end_label(target)));
    }
    return;
  }
  if (key != keys::enter) {
    return;
  }
  if (row.action == RowAction::Database) {
    form_ =
        OpenForm{Form("Database location",
                      {
                          Field::text("path", "New location", "",
                                      text::concat("Full path of the database file, for example ",
                                                   example_database_path(), ". The current file stays where it is."),
                                      1024),
                          Field::choice("mode", "What to do", "copy", "",
                                        {{"copy", "Copy my data there and use it"},
                                         {"switch", "Use the database that is there, or start an empty one"}}),
                      }),
                 Submit{SubmitKind::MoveDatabase, 0, 0, {}}};
  } else if (row.action == RowAction::Demo) {
    std::int64_t count = 0;
    if (!attempt([&] { count = app_->records().remove_demo(true); })) {
      return;
    }
    if (count == 0) {
      say(Notice::Info, "There is no demo data to remove");
      return;
    }
    ask("Remove demo data", text::concat("This removes ", count, " demo records. Your own records are not touched."),
        "remove them", RemoveDemo{});
  } else if (row.action == RowAction::Notifier) {
    toggle_notifier();
  }
}

void Model::edit_setting(std::string_view key) {
  std::optional<settings::Value> found;
  if (!attempt([&] { found = app_->settings().get(key); })) {
    return;
  }
  const settings::Value& value = *found;
  const settings::Definition& definition = value.definition;
  std::string help(definition.help);
  if (!value.overridden_by.empty()) {
    help.append(text::concat(" This value is overridden by the ", value.overridden_by,
                             "; your change is saved and applies when the override is removed."));
  }
  const std::string_view current = saved_value(value);
  std::vector<Field> fields;
  switch (definition.kind) {
    case settings::Kind::Bool:
      fields.push_back(Field::yes_no("value", definition.label, current == "true", help));
      break;
    case settings::Kind::Enum: {
      std::vector<Option> options;
      for (const std::string_view option : definition.options) {
        options.emplace_back(option, key == settings::key_theme ? theme::label(option) : option);
      }
      fields.push_back(Field::choice("value", definition.label, current, help, std::move(options)));
      break;
    }
    case settings::Kind::Int:
      fields.push_back(
          Field::text("value", definition.label, current,
                      text::concat(help, " Whole number from ", definition.min, " to ", definition.max, "."), 12));
      break;
    case settings::Kind::Color:
    case settings::Kind::Text:
    case settings::Kind::Columns:
    case settings::Kind::Sort:
      fields.push_back(Field::text("value", definition.label, current, help, 200));
      break;
  }
  form_ = OpenForm{Form(text::concat(definition.group, ", ", definition.label), std::move(fields)),
                   Submit{SubmitKind::Setting, 0, 0, std::string(key)}};
}

void Model::submit_setting(const Form& form, std::string_view key) {
  const settings::Value saved = app_->settings().set(key, form.value("value"));
  const std::string_view label = saved.definition.label;
  changed(text::concat("Saved ", label));
  if (!saved.overridden_by.empty()) {
    say(Notice::Warning, text::concat("Saved ", label, ", but the ", saved.overridden_by, " still takes precedence"));
  }
  if (key == settings::key_accent && !warnings_.empty()) {
    say(Notice::Warning, warnings_.back());
  }
}

void Model::submit_move(const Form& form) {
  const service::MoveMode mode = form.value("mode") == "switch" ? service::MoveMode::Switch : service::MoveMode::Copy;
  service::MovePlan plan = app_->plan_move(text::trim_whitespace(form.value("path")), mode);
  std::string statement = plan.statement;
  confirm_ = Confirmation{"Database location", std::move(statement), "continue", MoveDatabase{std::move(plan)}};
}

void Model::toggle_notifier() {
  notifier::Placement placement;
  if (!attempt([&] { placement = autostart().status(); })) {
    return;
  }
  if (placement.present) {
    ask(notifier_title,
        text::concat("This removes ", files::shown(placement.path),
                     ". The notifier no longer starts when you sign in. Nothing else changes."),
        "remove the entry", UninstallNotifier{});
    return;
  }
  std::filesystem::path target;
  if (!attempt([&] { target = autostart().install(true); })) {
    return;
  }
  confirm_ = Confirmation{
      std::string(notifier_title),
      text::concat("This writes ", files::shown(target), " so that \"", meta::slug,
                   " notifier run\" starts when you sign in. It needs no administrator rights and can be removed "
                   "here at any time."),
      "write the entry", InstallNotifier{}};
}

Lines Model::settings_view(std::size_t size, std::size_t height) {
  std::size_t label_width = 0;
  for (const SettingRow& row : set_.rows) {
    label_width = std::max(label_width, width(row.label));
  }
  label_width = std::min(label_width, std::max<std::size_t>(less(size, 36), 18));
  const std::size_t value_width = std::max<std::size_t>(less(size, label_width + 18), 12);
  Lines all;
  std::size_t cursor_line = 0;
  std::string_view group;
  for (std::size_t index = 0; index < set_.rows.size(); ++index) {
    const SettingRow& row = set_.rows[index];
    if (row.group != group) {
      if (!group.empty()) {
        all.emplace_back();
      }
      group = row.group;
      all.push_back(Line::styled(row.group, Look::Heading));
    }
    const std::string shown = text::concat(pad(row.label, label_width), "  ",
                                           pad(tail(row.value, value_width), value_width), "  ", row.source);
    if (index == set_.cursor) {
      cursor_line = all.size();
    }
    all.push_back(selectable(shown, index == set_.cursor, size));
  }
  Lines help;
  if (set_.cursor < set_.rows.size()) {
    help = wrapped(set_.rows[set_.cursor].help, size, Look::Muted);
  }
  const std::size_t rows = std::max<std::size_t>(less(height, help.size() + 2), 3);
  std::size_t offset = set_.offset;
  if (cursor_line < offset + 1) {
    offset = less(cursor_line, 1);
  }
  if (cursor_line >= offset + rows) {
    offset = cursor_line + 1 - rows;
  }
  Lines lines;
  for (std::size_t index = offset; index < all.size() && index < offset + rows; ++index) {
    lines.push_back(std::move(all[index]));
  }
  lines.push_back(Line::styled(repeat("─", size), Look::Border));
  extend(lines, std::move(help));
  set_.offset = offset;
  return lines;
}

}
