#include "gui/model.hpp"

#include <algorithm>
#include <cmath>
#include <numeric>

#include "core/files.hpp"
#include "core/meta.hpp"
#include "core/text.hpp"
#include "core/transfer.hpp"
#include "core/userconfig.hpp"

namespace app::gui {

namespace {

constexpr std::string_view busy =
    "The database is busy in another window. Nothing was lost; the view refreshes when it is free.";
constexpr std::string_view conflict =
    "another window changed this record while you were editing. Your text is still here: copy what you need, choose "
    "Cancel, open Edit again to load the new version and repeat your change";

Choices formats() {
  return {{"", "From the file name"}, {"csv", "CSV"}, {"json", "JSON"}};
}

Form record_form(std::string title, std::string_view accept, const records::Input& input, Submit submit) {
  return Form(std::move(title), accept,
              {
                  Field::text("name", "Name", input.name,
                              text::concat("Required, unique, at most ", records::max_name_length, " characters."),
                              records::max_name_length),
                  Field::choice("status", "Status", records::name(input.status), "", status_options()),
                  Field::choice("severity", "Severity", theme::name(input.severity),
                                "High and critical open records raise a notification.", severity_options()),
                  Field::area("note", "Note", input.note, "Optional free text, searchable.", records::max_note_length),
                  Field::yes_no("enabled", "Enabled", input.enabled, "Disabled records are kept but not counted."),
              },
              std::move(submit));
}

records::Input form_input(const Form& form) {
  records::Input input;
  input.name = form.value("name");
  input.status = records::find_status(form.value("status")).value_or(records::Status::Open);
  input.severity = theme::parse_severity(form.value("severity")).value_or(theme::Severity::Info);
  input.note = form.value("note");
  input.enabled = form.value("enabled") == "true";
  return input;
}

std::string trimmed(const std::string& value) {
  return std::string(text::trim_whitespace(value));
}

std::string explain(const AppError& error) {
  return error.is_busy() ? std::string(busy) : error.message();
}

}

theme::Severity severity(Tone tone) {
  switch (tone) {
    case Tone::Info:
    case Tone::Success:
      return theme::Severity::Info;
    case Tone::Warning:
      return theme::Severity::High;
    case Tone::Failure:
      return theme::Severity::Critical;
  }
  return theme::Severity::Info;
}

Choices severity_options() {
  Choices options;
  for (const theme::Severity level : theme::all_severities) {
    options.emplace_back(theme::name(level), theme::label(level));
  }
  return options;
}

Choices status_options() {
  Choices options;
  for (const records::Status status : records::all_statuses) {
    options.emplace_back(records::name(status), records::label(status));
  }
  return options;
}

std::string_view event_label(std::string_view id) {
  const notify::EventType* event = notify::event_type(id);
  return event == nullptr ? id : event->label;
}

std::string_view enabled_label(bool enabled) {
  return enabled ? "Enabled" : "Disabled";
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

WindowSize window_size(const Values& values) {
  const auto side = [&values](std::string_view key, float fallback) {
    const auto found = values.find(key);
    if (found == values.end() || found->second.number() <= 0) {
      return fallback;
    }
    return static_cast<float>(found->second.number());
  };
  return {side(settings::key_window_width, 1180.0F), side(settings::key_window_height, 760.0F)};
}

std::string_view saved_value(const settings::Value& value) {
  return value.stored ? std::string_view(*value.stored) : value.definition.fallback;
}

Dashboard::Dashboard(service::App& app, Options options)
    : tokens(theme::resolve(theme::Options{})), app_(&app), options_(std::move(options)) {
  const bool welcome = app_->created_database();
  rec.query.sort = "updated";
  rec.query.descending = true;
  about.usage = service::usage_guide();
  about.hits.resize(about.usage.sections.size());
  std::iota(about.hits.begin(), about.hits.end(), std::size_t{0});
  reload();
  const auto sidebar = values.find(settings::key_sidebar_collapsed);
  collapsed = sidebar != values.end() && sidebar->second.flag();
  if (welcome) {
    app_->dismiss_welcome();
    confirm = Confirm(
        text::concat("Welcome to ", meta::app_name),
        text::concat("A new database was created at ", files::shown(app_->location().path),
                     ".\n\nEverything stays in this one file on your computer. You can move it later in Settings, "
                     "Data, Database location.\n\nLoad demo data to look around with labeled sample records, or "
                     "cancel to start with an empty database."),
        "Load demo data", Action{LoadDemo{}});
  }
}

const service::App& Dashboard::app() const {
  return *app_;
}

const Options& Dashboard::options() const {
  return options_;
}

const std::string& Dashboard::next() const {
  return next_;
}

bool Dashboard::finished() const {
  return closing_;
}

WindowSize Dashboard::size() const {
  return window_size(values);
}

void Dashboard::follow_system(bool dark) {
  if (dark != options_.system_dark) {
    options_.system_dark = dark;
    reload();
  }
}

double Dashboard::interval() const {
  const auto found = values.find(settings::key_refresh_seconds);
  const std::int64_t seconds = found == values.end() ? 0 : found->second.number();
  return seconds < 1 ? 2.0 : static_cast<double>(seconds);
}

void Dashboard::close(std::string_view next) {
  next_ = std::string(next);
  closing_ = true;
}

void Dashboard::keep_window_size(float width, float height) {
  const std::array<std::pair<std::string_view, float>, 2> sides = {
      {{settings::key_window_width, width}, {settings::key_window_height, height}}};
  for (const auto& [key, side] : sides) {
    const std::string wanted = std::to_string(std::lround(side));
    const auto found = values.find(key);
    if (found != values.end() && saved_value(found->second) != wanted) {
      try {
        static_cast<void>(app_->settings().set(key, wanted));
      } catch (const AppError&) {
        continue;
      }
    }
  }
}

void Dashboard::say(Tone tone, std::string text) {
  notice = Message{tone, std::move(text)};
  ++flash;
}

void Dashboard::report(const AppError& error) {
  if (error.is_busy()) {
    say(Tone::Warning, std::string(busy));
  } else {
    say(Tone::Failure, error.message());
  }
}

bool Dashboard::attempt(const std::function<void()>& work) {
  try {
    work();
    return true;
  } catch (const AppError& error) {
    report(error);
    return false;
  }
}

void Dashboard::changed(std::string text) {
  reload();
  say(Tone::Success, std::move(text));
}

void Dashboard::reload() {
  Values loaded;
  theme::Tokens look;
  if (!attempt([&] {
        loaded = app_->settings().effective();
        look = app_->appearance(options_.system_dark);
      })) {
    return;
  }
  restyled = restyled || look != tokens;
  tokens = std::move(look);
  values = std::move(loaded);
  if (!attempt([this] { overview = app_->records().overview(); })) {
    return;
  }
  load_records();
  load_inbox();
  load_settings();
  try {
    about.diagnostics = app_->diagnostics(true);
    static_cast<void>(app_->poll());
  } catch (const AppError&) {
    return;
  }
}

void Dashboard::tick() {
  try {
    if (app_->poll()) {
      reload();
      if (!form && !confirm) {
        say(Tone::Info, "Updated by another window");
      }
    }
  } catch (const AppError& error) {
    if (!error.is_busy()) {
      report(error);
    }
  }
}

std::string_view Dashboard::page_id() const {
  return service::navigation.at(page).id;
}

void Dashboard::show(std::string_view id) {
  const auto found = std::ranges::find(service::navigation, id, &service::NavItem::id);
  if (found != service::navigation.end()) {
    page = static_cast<std::size_t>(found - service::navigation.begin());
  }
}

void Dashboard::show_about(AboutTab tab) {
  show(service::nav_about);
  about.tab = tab;
}

std::string Dashboard::nav_label(const service::NavItem& item) const {
  if (item.id == service::nav_notifications && overview.unread_notifications > 0) {
    return text::concat(item.label, " (", overview.unread_notifications, ")");
  }
  return std::string(item.label);
}

std::string Dashboard::bell_label() const {
  if (overview.unread_notifications == 0) {
    return "Notifications: nothing unread";
  }
  return text::concat("Notifications: ", overview.unread_notifications, " unread");
}

void Dashboard::toggle_sidebar() {
  collapsed = !collapsed;
  if (attempt([this] {
        static_cast<void>(app_->settings().set(settings::key_sidebar_collapsed, collapsed ? "true" : "false"));
      })) {
    try {
      static_cast<void>(app_->poll());
    } catch (const AppError&) {
      return;
    }
  }
}

void Dashboard::refresh() {
  changed("Refreshed");
}

std::vector<std::string> Dashboard::terminal_arguments() const {
  std::vector<std::string> arguments = {"--ui", "tui"};
  const service::Options& opened = app_->options();
  if (opened.database_path) {
    arguments.emplace_back("--db");
    arguments.push_back(files::shown(*opened.database_path));
  }
  for (const auto& [key, value] : opened.overrides) {
    arguments.emplace_back("--set");
    arguments.push_back(text::concat(key, "=", value));
  }
  return arguments;
}

void Dashboard::open_terminal() {
  if (options_.in_terminal) {
    close(userconfig::front_end_tui);
    return;
  }
  if (!options_.open_terminal) {
    say(Tone::Warning,
        text::concat("No terminal window can be opened from here. Start ", meta::slug, " --ui tui in a terminal."));
    return;
  }
  const std::vector<std::string> arguments = terminal_arguments();
  if (attempt([&] { options_.open_terminal(arguments); })) {
    close("");
  }
}

void Dashboard::shortcut(Shortcut pressed) {
  if (pressed.kind == ShortcutKind::Quit) {
    close("");
    return;
  }
  if (form || confirm) {
    return;
  }
  switch (pressed.kind) {
    case ShortcutKind::Page:
      if (pressed.page < service::navigation.size()) {
        page = pressed.page;
      }
      break;
    case ShortcutKind::Search:
      focus_search = true;
      break;
    case ShortcutKind::Settings:
      show(service::nav_settings);
      break;
    case ShortcutKind::Add:
      show(service::nav_records);
      add_record();
      break;
    case ShortcutKind::Refresh:
      refresh();
      break;
    case ShortcutKind::Sidebar:
      toggle_sidebar();
      break;
    case ShortcutKind::Guide:
      show_about(AboutTab::Guide);
      break;
    case ShortcutKind::Remove:
      if (page_id() == service::nav_records) {
        remove_record();
      }
      break;
    case ShortcutKind::Escape:
      if (page_id() == service::nav_records) {
        rec.selected.reset();
      }
      break;
    case ShortcutKind::Quit:
      break;
  }
}

void Dashboard::search_everywhere() {
  const std::string query = trimmed(search);
  if (query.empty()) {
    say(Tone::Info, "Type one or more words, then press Enter.");
    return;
  }
  std::vector<records::Record> found;
  records::Query wanted;
  wanted.search = query;
  wanted.include_disabled = true;
  wanted.limit = list_limit;
  if (!attempt([&] { found = app_->records().list(wanted); })) {
    return;
  }
  const std::size_t sections = about.usage.search(query).size();
  if (!found.empty()) {
    show(service::nav_records);
    rec.query.include_disabled = true;
    rec.query.search = query;
    load_records();
    say(Tone::Info, text::concat(found.size(), " records and ", sections, " usage guide sections match \"", query,
                                 "\". The guide is under About, Usage guide."));
  } else if (sections > 0) {
    show_about(AboutTab::Guide);
    about.search = query;
    filter_guide();
    say(Tone::Info, text::concat("No record matches \"", query, "\". ", sections, " usage guide sections do."));
  } else {
    say(Tone::Info,
        text::concat("Nothing matches \"", query, "\". Search covers record names, record notes and the usage guide."));
  }
}

void Dashboard::ask(Confirm question) {
  const auto found = values.find(settings::key_confirm);
  const bool wanted = found == values.end() || found->second.flag();
  if (question.action && !wanted) {
    run(*question.action);
    return;
  }
  confirm = std::move(question);
}

void Dashboard::answer(bool accepted) {
  if (!confirm) {
    return;
  }
  const Confirm asked = std::move(*confirm);
  confirm.reset();
  if (accepted && asked.action) {
    run(*asked.action);
  }
}

void Dashboard::run(const Action& action) {
  if (std::holds_alternative<LoadDemo>(action)) {
    load_demo();
    return;
  }
  std::string done;
  if (attempt([&] { done = perform(action); })) {
    changed(std::move(done));
  }
}

std::string Dashboard::perform(const Action& action) {
  if (const auto* record = std::get_if<RemoveRecord>(&action)) {
    app_->records().remove(record->id);
    if (rec.selected == record->id) {
      rec.selected.reset();
    }
    return text::concat("Removed record ", record->id);
  }
  if (std::holds_alternative<RemoveDemo>(action)) {
    return text::concat("Removed ", app_->records().remove_demo(false), " demo records");
  }
  if (std::holds_alternative<InstallNotifier>(action)) {
    static_cast<void>(autostart().install(false));
    return "The notifier starts the next time you sign in";
  }
  if (std::holds_alternative<UninstallNotifier>(action)) {
    static_cast<void>(autostart().uninstall(false));
    return "The notifier no longer starts at sign-in";
  }
  if (const auto* move = std::get_if<MoveDatabase>(&action)) {
    app_->move_database(move->plan);
    return text::concat("Now using ", files::shown(app_->location().path));
  }
  if (const auto* source = std::get_if<ImportFile>(&action)) {
    const transfer::ImportResult done = app_->import_file(files::from_utf8(source->path), source->format, false);
    rec.query.include_disabled = true;
    return text::concat("Import finished: ", done.summary());
  }
  if (const auto* link = std::get_if<OpenLink>(&action)) {
    return open_link(link->link);
  }
  return {};
}

const notifier::Autostart& Dashboard::autostart() const {
  if (!options_.autostart) {
    throw AppError::usage(std::string(notifier_missing));
  }
  return *options_.autostart;
}

std::string Dashboard::open_link(const service::Link& link) const {
  if (!link.allowed) {
    throw AppError::usage(link.reason);
  }
  if (!options_.open_url) {
    throw AppError::usage(text::concat("No browser can be started from here. Copy this link: ", link.url));
  }
  try {
    options_.open_url(link.url);
  } catch (const AppError& error) {
    throw AppError::io(text::concat(error.message(), ": ", link.url));
  }
  return text::concat("Opened ", link.label, " in your browser");
}

void Dashboard::follow(const service::Link& link) {
  std::string done;
  if (attempt([&] { done = open_link(link); })) {
    say(Tone::Success, std::move(done));
  }
}

void Dashboard::load_demo() {
  std::size_t added = 0;
  if (!attempt([&] { added = app_->records().load_demo(); })) {
    return;
  }
  if (added == 0) {
    changed("The demo data is already loaded");
  } else {
    changed(text::concat("Added ", added, " demo records. Remove them in Settings, Data, Remove demo data."));
  }
}

void Dashboard::submit() {
  if (!form) {
    return;
  }
  Form sent = std::move(*form);
  form.reset();
  try {
    switch (sent.submit.kind) {
      case SubmitKind::AddRecord:
        submit_add(sent);
        break;
      case SubmitKind::EditRecord:
        submit_edit(sent, sent.submit.id, sent.submit.revision);
        break;
      case SubmitKind::Columns:
        submit_columns(sent);
        break;
      case SubmitKind::Import:
        submit_import(sent);
        break;
      case SubmitKind::Export:
        submit_export(sent);
        break;
      case SubmitKind::Rule:
        submit_rule(sent, sent.submit.name);
        break;
      case SubmitKind::MoveDatabase:
        submit_move(sent);
        break;
    }
  } catch (const AppError& error) {
    sent.problem = explain(error);
    form = std::move(sent);
  }
}

void Dashboard::cancel() {
  form.reset();
}

void Dashboard::load_records() {
  if (const auto found = values.find(settings::key_record_sort); found != values.end()) {
    const auto [key, descending] = settings::split_sort(found->second.raw);
    if (!key.empty()) {
      rec.query.sort = key;
      rec.query.descending = descending;
    }
  }
  if (const auto found = values.find(settings::key_record_columns); found != values.end()) {
    rec.columns = settings::split_columns(found->second.raw);
  }
  records::Query query = rec.query;
  query.limit = list_limit;
  attempt([&] { rec.list = app_->records().list(query); });
}

const records::Record* Dashboard::selected_record() const {
  if (!rec.selected) {
    return nullptr;
  }
  const auto found = std::ranges::find(rec.list, *rec.selected, &records::Record::id);
  return found == rec.list.end() ? nullptr : &*found;
}

bool Dashboard::filtered() const {
  return !rec.query.search.empty() || rec.query.status || rec.query.severity;
}

std::string_view Dashboard::empty_records_text() const {
  if (filtered()) {
    return "No records match these filters. Clear the search box and the filters to see every record.";
  }
  if (!rec.query.include_disabled && overview.disabled > 0) {
    return "Every record is disabled. Select Show disabled to see them.";
  }
  return "No records yet. Choose Add record to create the first one, Import to read a CSV or JSON file, or load demo "
         "data on the Overview.";
}

void Dashboard::clear_filters() {
  rec.query.search.clear();
  rec.query.status.reset();
  rec.query.severity.reset();
  load_records();
}

void Dashboard::add_record() {
  form = record_form("Add record", "Add record", records::Input{}, Submit{SubmitKind::AddRecord, 0, 0, {}});
}

void Dashboard::edit_record() {
  const records::Record* record = selected_record();
  if (record == nullptr) {
    return;
  }
  form = record_form(text::concat("Edit record ", record->id), "Save", record->input(),
                     Submit{SubmitKind::EditRecord, record->id, record->revision, {}});
}

void Dashboard::toggle_record() {
  const records::Record* record = selected_record();
  if (record == nullptr) {
    return;
  }
  const std::int64_t id = record->id;
  const bool enable = !record->enabled;
  records::Record updated;
  if (!attempt([&] { updated = app_->records().set_enabled(id, enable); })) {
    return;
  }
  if (!updated.enabled) {
    rec.query.include_disabled = true;
  }
  changed(text::concat("Record ", updated.id, " is ", updated.enabled ? "enabled" : "disabled"));
}

void Dashboard::remove_record() {
  const records::Record* record = selected_record();
  if (record == nullptr) {
    return;
  }
  Confirm question("Remove record",
                   text::concat("This removes record ", record->id, " (", record->name,
                                ") permanently. Its notifications stay in the list without a link."),
                   "Remove", Action{RemoveRecord{record->id}});
  question.danger = true;
  ask(std::move(question));
}

void Dashboard::copy_record() {
  const records::Record* record = selected_record();
  if (record == nullptr) {
    return;
  }
  clipboard = text::concat("Record ", record->id, ": ", record->name, "\nStatus: ", records::label(record->status),
                           "\nSeverity: ", theme::label(record->severity), "\nState: ", enabled_label(record->enabled),
                           "\nUpdated: ", app_->format_time(record->updated_at));
  say(Tone::Success, text::concat("Copied record ", record->id, " to the clipboard"));
}

std::string Dashboard::record_details(const records::Record& record) const {
  return text::concat("Record ", record.id, "\nStatus: ", records::label(record.status),
                      "\nSeverity: ", theme::label(record.severity), "\nState: ", enabled_label(record.enabled),
                      "\nCreated: ", app_->format_time(record.created_at),
                      "\nUpdated: ", app_->format_time(record.updated_at), "\nRevision: ", record.revision);
}

void Dashboard::submit_add(const Form& sent) {
  const records::Record created = app_->records().create(form_input(sent));
  if (!created.enabled) {
    rec.query.include_disabled = true;
  }
  changed(text::concat("Added record ", created.id, ": ", created.name));
  rec.selected = created.id;
}

void Dashboard::submit_edit(const Form& sent, std::int64_t id, std::int64_t revision) {
  try {
    const records::Record updated = app_->records().update(id, revision, form_input(sent));
    changed(text::concat("Updated record ", updated.id));
  } catch (const AppError& error) {
    if (error.kind() != ErrorKind::Conflict) {
      throw;
    }
    reload();
    throw AppError::conflict(std::string(conflict));
  }
}

void Dashboard::keep(std::string_view key, std::string_view raw, std::string_view what) {
  const settings::Value saved = app_->settings().set(key, raw);
  reload();
  if (!saved.overridden_by.empty()) {
    say(Tone::Warning, text::concat(what, " saved, but the ", saved.overridden_by, " still takes precedence"));
  }
}

void Dashboard::sort_by(std::string_view key, bool descending) {
  const std::string raw = text::concat(key, ":", descending ? "desc" : "asc");
  attempt([&] { keep(settings::key_record_sort, raw, "Sort order"); });
}

void Dashboard::choose_columns() {
  const auto found = values.find(settings::key_record_columns);
  if (found == values.end()) {
    return;
  }
  const settings::Value& value = found->second;
  const std::vector<std::string> stored = settings::split_columns(saved_value(value));
  std::vector<Field> fields;
  for (const std::string_view column : value.definition.options) {
    if (std::ranges::find(value.definition.pinned, column) != value.definition.pinned.end()) {
      continue;
    }
    const bool shown = std::ranges::find(stored, column) != stored.end();
    fields.push_back(Field::choice(column, records::column_label(column), shown ? "show" : "hide", "",
                                   {{"show", "Shown"}, {"hide", "Hidden"}}));
  }
  form = Form("Columns of the record table", "Save", std::move(fields), Submit{SubmitKind::Columns, 0, 0, {}});
}

void Dashboard::submit_columns(const Form& sent) {
  const settings::Definition* definition = settings::lookup(settings::key_record_columns);
  std::vector<std::string> shown;
  for (const std::string_view column : settings::record_columns()) {
    const bool pinned =
        definition != nullptr && std::ranges::find(definition->pinned, column) != definition->pinned.end();
    if (pinned || sent.value(column) == "show") {
      shown.emplace_back(column);
    }
  }
  const settings::Value saved = app_->settings().set(settings::key_record_columns, text::join(shown, ","));
  reload();
  if (saved.overridden_by.empty()) {
    say(Tone::Success, "Saved the columns of the record table");
  } else {
    say(Tone::Warning, text::concat("Columns saved, but the ", saved.overridden_by, " still takes precedence"));
  }
}

void Dashboard::import_records() {
  form = Form("Import records", "Preview",
              {Field::text("path", "File", "",
                           "A CSV or JSON file. Rows are matched by name: new names are added, known names are "
                           "updated. Nothing is written until you confirm the preview.",
                           path_limit),
               Field::choice("format", "Format", "", "", formats())},
              Submit{SubmitKind::Import, 0, 0, {}});
}

void Dashboard::submit_import(const Form& sent) {
  const std::string path = trimmed(sent.value("path"));
  const std::string format = sent.value("format");
  if (path.empty()) {
    throw AppError::usage("choose the file to import");
  }
  const transfer::ImportResult result = app_->import_file(files::from_utf8(path), format, true);
  std::string statement = text::concat("Dry run of ", path, ":\n\n", result.summary(), "\n");
  for (const transfer::RowError& problem : result.problems) {
    statement += text::concat("\nRejected ", problem.display());
  }
  std::optional<Action> action;
  if (result.created + result.updated == 0) {
    statement += "\n\nThere is nothing to import.";
  } else {
    action = Action{ImportFile{path, format}};
  }
  Confirm question("Import preview", std::move(statement), "Import these rows", std::move(action));
  if (!question.action) {
    question.cancel = "Close";
  }
  confirm = std::move(question);
}

void Dashboard::export_records() {
  form = Form("Export records", "Export",
              {Field::text("path", "File", "records.csv",
                           "Every record is exported, including disabled ones. The file is readable only by your "
                           "user account.",
                           path_limit),
               Field::choice("format", "Format", "", "", formats()),
               Field::choice("overwrite", "If the file exists", "false", "",
                             {{"false", "Stop and keep it"}, {"true", "Overwrite it"}})},
              Submit{SubmitKind::Export, 0, 0, {}});
}

void Dashboard::submit_export(const Form& sent) {
  const std::string path = trimmed(sent.value("path"));
  if (path.empty()) {
    throw AppError::usage("choose the file to write");
  }
  try {
    const std::size_t count =
        app_->export_file(files::from_utf8(path), sent.value("format"), sent.value("overwrite") == "true");
    say(Tone::Success, text::concat("Exported ", count, " records to ", path));
  } catch (const AppError& error) {
    if (error.kind() != ErrorKind::Conflict) {
      throw;
    }
    throw AppError::conflict(
        text::concat(path, " already exists. Choose another name, or set \"If the file exists\" to \"Overwrite it\""));
  }
}

void Dashboard::load_inbox() {
  notify::Filter filter;
  filter.unread_only = inbox.unread_only;
  filter.limit = list_limit;
  std::vector<notify::Notification> list;
  std::vector<notify::Rule> rules;
  if (!attempt([&] {
        list = app_->notifications().list(filter);
        rules = app_->notifications().rules();
      })) {
    return;
  }
  inbox.list = std::move(list);
  inbox.rule_list = std::move(rules);
  if (inbox.selected && std::ranges::find(inbox.list, *inbox.selected, &notify::Notification::id) == inbox.list.end()) {
    inbox.selected.reset();
  }
}

std::string Dashboard::inbox_summary() const {
  const auto unread = std::ranges::count_if(inbox.list, [](const notify::Notification& item) { return item.unread(); });
  return text::concat(inbox.list.size(), " shown, ", unread, " unread");
}

std::string_view Dashboard::empty_inbox_text() const {
  if (inbox.unread_only) {
    return "No unread notifications. Clear Unread only to see all of them.";
  }
  return "No notifications. They appear when a record needs attention or is completed.";
}

std::optional<notify::Notification> Dashboard::selected_notification() {
  if (inbox.selected) {
    const auto found = std::ranges::find(inbox.list, *inbox.selected, &notify::Notification::id);
    if (found != inbox.list.end()) {
      return *found;
    }
  }
  say(Tone::Info, "Select a notification first");
  return std::nullopt;
}

void Dashboard::mark_read() {
  const std::optional<notify::Notification> item = selected_notification();
  if (!item) {
    return;
  }
  if (attempt([&] { app_->notifications().mark_read(item->id); })) {
    changed("Marked the notification as read");
  }
}

void Dashboard::mark_all_read() {
  std::int64_t marked = 0;
  if (attempt([&] { marked = app_->notifications().mark_all_read(); })) {
    changed(text::concat("Marked ", marked, " notifications as read"));
  }
}

void Dashboard::open_notification_record() {
  const std::optional<notify::Notification> item = selected_notification();
  if (!item) {
    return;
  }
  if (item->unread()) {
    if (!attempt([&] { app_->notifications().mark_read(item->id); })) {
      return;
    }
    reload();
  }
  if (item->record_id == 0) {
    say(Tone::Info, "The record of this notification was removed");
    return;
  }
  records::Query query;
  query.sort = rec.query.sort;
  query.descending = rec.query.descending;
  query.include_disabled = true;
  rec.query = std::move(query);
  load_records();
  rec.selected = item->record_id;
  show(service::nav_records);
}

std::optional<notify::Rule> Dashboard::selected_rule() {
  if (inbox.rule) {
    const auto found = std::ranges::find(inbox.rule_list, *inbox.rule, &notify::Rule::event_type);
    if (found != inbox.rule_list.end()) {
      return *found;
    }
  }
  say(Tone::Info, "Select an event type first");
  return std::nullopt;
}

void Dashboard::edit_rule() {
  const std::optional<notify::Rule> rule = selected_rule();
  if (!rule) {
    return;
  }
  form = Form(text::concat("Rule for ", event_label(rule->event_type)), "Save",
              {Field::yes_no("enabled", "Enabled", rule->enabled, "Off stops notifications of this event type."),
               Field::choice("severity", "Minimum severity", theme::name(rule->min_severity),
                             "Events below this severity raise no notification.", severity_options()),
               Field::text("start", "Quiet hours start", rule->quiet_start,
                           "Time of day as HH:MM. Leave both times empty for no quiet hours.", 5),
               Field::text("end", "Quiet hours end", rule->quiet_end,
                           "Time of day as HH:MM. May be earlier than the start to span midnight.", 5),
               Field::text("dedup", "De-duplication", std::to_string(rule->dedup_minutes),
                           "Minutes in which the same event for the same record is not raised again. 0 turns it "
                           "off.",
                           5)},
              Submit{SubmitKind::Rule, 0, 0, rule->event_type});
}

void Dashboard::submit_rule(const Form& sent, std::string_view event) {
  const std::string entered = trimmed(sent.value("dedup"));
  const std::optional<std::int64_t> minutes = text::parse_integer(entered);
  if (!minutes) {
    throw AppError::usage("de-duplication needs a whole number of minutes, for example 60");
  }
  notify::Rule rule;
  rule.event_type = std::string(event);
  rule.enabled = sent.value("enabled") == "true";
  rule.min_severity = theme::parse_severity(sent.value("severity")).value_or(theme::Severity::Info);
  rule.quiet_start = trimmed(sent.value("start"));
  rule.quiet_end = trimmed(sent.value("end"));
  rule.dedup_minutes = *minutes;
  rule.customized = true;
  app_->notifications().set_rule(rule);
  changed(text::concat("Saved the rule for ", event_label(event)));
}

void Dashboard::reset_rule() {
  const std::optional<notify::Rule> rule = selected_rule();
  if (!rule) {
    return;
  }
  if (attempt([&] { app_->notifications().reset_rule(rule->event_type); })) {
    changed(text::concat("The rule for ", event_label(rule->event_type), " is back to its defaults"));
  }
}

void Dashboard::load_settings() {
  std::vector<settings::Value> list;
  if (!attempt([&] { list = app_->settings().list(); })) {
    return;
  }
  std::vector<std::string> groups;
  for (const std::string_view group : settings::groups(list)) {
    groups.emplace_back(group);
  }
  for (const std::string_view extra : {"Data", "Notifications", "Launcher"}) {
    if (std::ranges::find(groups, extra) == groups.end()) {
      groups.emplace_back(extra);
    }
  }
  for (const settings::Value& value : list) {
    if (!set.dirty.contains(value.key)) {
      set.drafts.insert_or_assign(value.key, std::string(saved_value(value)));
    }
  }
  set.list = std::move(list);
  set.groups = std::move(groups);
  if (!options_.autostart) {
    set.notifier = {NotifierKind::Missing, {}};
    return;
  }
  try {
    const notifier::Placement placement = options_.autostart->status();
    set.notifier = placement.present ? NotifierState{NotifierKind::On, files::shown(placement.path)}
                                     : NotifierState{NotifierKind::Off, {}};
  } catch (const AppError& error) {
    set.notifier = {NotifierKind::Unknown, error.message()};
  }
}

std::string Dashboard::setting_note(const settings::Value& value) const {
  std::string note;
  if (!value.overridden_by.empty()) {
    note = text::concat("overridden by ", value.overridden_by);
  }
  if (!value.override_error.empty()) {
    if (!note.empty()) {
      note += ". ";
    }
    note += value.override_error;
  }
  return note;
}

void Dashboard::save_setting(std::string_view key, std::string_view raw) {
  settings::Value saved;
  try {
    saved = app_->settings().set(key, raw);
  } catch (const AppError& error) {
    set.problems.insert_or_assign(std::string(key), explain(error));
    return;
  }
  if (const auto found = set.problems.find(key); found != set.problems.end()) {
    set.problems.erase(found);
  }
  if (const auto found = set.dirty.find(key); found != set.dirty.end()) {
    set.dirty.erase(found);
  }
  const std::string_view label = saved.definition.label;
  changed(text::concat("Saved ", label));
  if (!saved.overridden_by.empty()) {
    say(Tone::Warning, text::concat("Saved ", label, ", but the ", saved.overridden_by, " still takes precedence"));
  } else if (key == settings::key_accent && !tokens.warnings.empty()) {
    say(Tone::Warning, tokens.warnings.back());
  }
}

void Dashboard::reset_setting(std::string_view key) {
  settings::Value value;
  if (!attempt([&] { value = app_->settings().reset(key); })) {
    return;
  }
  if (const auto found = set.problems.find(key); found != set.problems.end()) {
    set.problems.erase(found);
  }
  if (const auto found = set.dirty.find(key); found != set.dirty.end()) {
    set.dirty.erase(found);
  }
  changed(text::concat(value.definition.label, " is back to its default"));
}

std::string Dashboard::database_note() const {
  const service::Location& location = app_->location();
  if (location.overridden_by.empty()) {
    return "Change copies the database to a new place and uses it, or switches to another database. The old file is "
           "never deleted.";
  }
  return text::concat("Set by the ", location.overridden_by, ". Remove the override to change the location here.");
}

bool Dashboard::database_locked() const {
  return !app_->location().overridden_by.empty();
}

std::string Dashboard::database_path() const {
  return files::shown(app_->location().path);
}

void Dashboard::change_database() {
  form = Form("Database location", "Continue",
              {Field::text("path", "New location", "",
                           "Full path of the database file. The current file stays where it is.", path_limit),
               Field::choice("mode", "What to do", "copy", "",
                             {{"copy", "Copy my data there and use it"},
                              {"switch", "Use the database that is there, or start an empty one"}})},
              Submit{SubmitKind::MoveDatabase, 0, 0, {}});
}

void Dashboard::submit_move(const Form& sent) {
  const service::MoveMode mode = sent.value("mode") == "switch" ? service::MoveMode::Switch : service::MoveMode::Copy;
  service::MovePlan plan = app_->plan_move(trimmed(sent.value("path")), mode);
  std::string statement = plan.statement;
  confirm = Confirm("Database location", std::move(statement), "Continue", Action{MoveDatabase{std::move(plan)}});
}

void Dashboard::remove_demo() {
  std::int64_t count = 0;
  if (!attempt([&] { count = app_->records().remove_demo(true); })) {
    return;
  }
  if (count == 0) {
    say(Tone::Info, "There is no demo data to remove");
    return;
  }
  Confirm question("Remove demo data",
                   text::concat("This removes ", count, " demo records. Your own records are not touched."), "Remove",
                   Action{RemoveDemo{}});
  question.danger = true;
  ask(std::move(question));
}

void Dashboard::toggle_notifier() {
  notifier::Placement placement;
  if (!attempt([&] { placement = autostart().status(); })) {
    return;
  }
  if (placement.present) {
    run(Action{UninstallNotifier{}});
    return;
  }
  confirm = Confirm("Start at sign-in",
                    text::concat("This writes ", files::shown(placement.path), " so that \"", meta::slug,
                                 " notifier run\" starts when you sign in. It needs no administrator rights and can "
                                 "be turned off here at any time."),
                    "Turn on", Action{InstallNotifier{}});
}

std::optional<std::string> Dashboard::launcher_override() const {
  const std::string variable = meta::env_name("ui");
  const std::optional<std::string> value = options_.env.get(variable);
  if (!value || value->empty()) {
    return std::nullopt;
  }
  return text::concat("Overridden by the environment variable ", variable, "=", *value);
}

const std::string& Dashboard::front_end() const {
  return app_->location().config.front_end;
}

void Dashboard::remember(std::string_view mode) {
  if (attempt([&] { app_->remember_front_end(mode); })) {
    changed(text::concat("Open with: ", front_end_label(mode)));
  }
}

void Dashboard::filter_guide() {
  const std::string query = trimmed(about.search);
  about.hits.clear();
  if (query.empty()) {
    about.hits.resize(about.usage.sections.size());
    std::iota(about.hits.begin(), about.hits.end(), std::size_t{0});
  } else {
    for (const guide::Hit& hit : about.usage.search(query)) {
      if (std::ranges::find(about.hits, hit.section) == about.hits.end()) {
        about.hits.push_back(hit.section);
      }
    }
  }
  if (about.section && std::ranges::find(about.hits, *about.section) == about.hits.end()) {
    about.section.reset();
  }
}

void Dashboard::report_bug() {
  if (!about.diagnostics) {
    return;
  }
  const service::IssueReport report = about.diagnostics->issue_report();
  confirm = Confirm("Report a bug",
                    text::concat("The report form opens in your browser, prefilled with exactly this text and "
                                 "nothing else. You can edit it before you submit it.\n\n",
                                 report.body),
                    "Open the report form", Action{OpenLink{service::external_link("the report form", report.url)}});
}

std::string Dashboard::diagnostics_text() const {
  std::string out;
  if (!about.diagnostics) {
    return out;
  }
  for (const auto& [label, value] : about.diagnostics->lines()) {
    out += label;
    if (label.size() < 30) {
      out.append(30 - label.size(), ' ');
    }
    out += ' ';
    out += value;
    out += '\n';
  }
  return out;
}

void Dashboard::copy_diagnostics() {
  clipboard = diagnostics_text();
  say(Tone::Success, "Diagnostics copied");
}

void Dashboard::check_database() {
  try {
    app_->store().integrity_check();
    about.diagnostics = app_->diagnostics(true);
    say(Tone::Success, "The database is healthy");
  } catch (const AppError& error) {
    say(Tone::Failure, text::concat("The integrity check failed: ", error.message()));
  }
}

}
