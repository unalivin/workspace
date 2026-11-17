#include <algorithm>
#include <utility>

#include "core/files.hpp"
#include "core/text.hpp"
#include "tui/model.hpp"

namespace app::tui {

namespace {

constexpr std::size_t search_limit = 200;
constexpr std::size_t path_limit = 1024;

std::vector<Option> formats() {
  return {{"", "From the file name"}, {"csv", "CSV"}, {"json", "JSON"}};
}

std::vector<Option> status_options() {
  std::vector<Option> options;
  for (const records::Status status : records::all_statuses) {
    options.emplace_back(records::name(status), text::concat(records::glyph(status), " ", records::label(status)));
  }
  return options;
}

template <typename Value, std::size_t Count>
std::optional<Value> next(const std::array<Value, Count>& all, const std::optional<Value>& current) {
  if (!current) {
    return all.front();
  }
  const auto found = std::ranges::find(all, *current);
  if (found == all.end() || found + 1 == all.end()) {
    return std::nullopt;
  }
  return *(found + 1);
}

std::string_view enabled_label(bool enabled) {
  return enabled ? "Enabled" : "Disabled";
}

Form record_form(std::string title, const records::Input& input) {
  return Form(std::move(title),
              {
                  Field::text("name", "Name", input.name,
                              text::concat("Required, unique, at most ", records::max_name_length, " characters."),
                              records::max_name_length),
                  Field::choice("status", "Status", records::name(input.status), "", status_options()),
                  Field::choice("severity", "Severity", theme::name(input.severity),
                                "High and critical open records raise a notification.", severity_options()),
                  Field::text("note", "Note", input.note, "Optional free text, searchable.", records::max_note_length),
                  Field::yes_no("enabled", "Enabled", input.enabled, "Disabled records are kept but not counted."),
              });
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

std::size_t less(std::size_t value, std::size_t amount) {
  return value > amount ? value - amount : 0;
}

struct Shown {
  std::string_view key;
  std::size_t width = 0;
  bool flexible = false;
};

}

RecordsPage::RecordsPage() : search("", search_limit) {
  query.sort = "updated";
  query.descending = true;
}

const records::Record* RecordsPage::selected() const {
  return cursor < list.size() ? &list[cursor] : nullptr;
}

void RecordsPage::select(std::int64_t id) {
  const auto found = std::ranges::find(list, id, &records::Record::id);
  if (found != list.end()) {
    cursor = static_cast<std::size_t>(found - list.begin());
  }
}

bool RecordsPage::filtered() const {
  return !query.search.empty() || query.status.has_value() || query.severity.has_value();
}

std::vector<Option> severity_options() {
  std::vector<Option> options;
  for (const theme::Severity level : theme::all_severities) {
    options.emplace_back(theme::name(level), text::concat(theme::glyph(level), " ", theme::label(level)));
  }
  return options;
}

void Model::load_records() {
  const records::Record* current = rec_.selected();
  const std::optional<std::int64_t> previous = current == nullptr ? std::nullopt : std::optional(current->id);
  if (const settings::Value* value = setting(settings::key_record_sort)) {
    auto [key, descending] = settings::split_sort(value->raw);
    if (!key.empty()) {
      rec_.query.sort = std::move(key);
      rec_.query.descending = descending;
    }
  }
  records::Query query = rec_.query;
  query.limit = list_limit;
  if (!attempt([&] { rec_.list = app_->records().list(query); })) {
    return;
  }
  rec_.cursor = std::min(rec_.cursor, less(rec_.list.size(), 1));
  if (previous) {
    rec_.select(*previous);
  }
}

void Model::records_key(Key key) {
  if (rec_.searching) {
    if (key == keys::enter) {
      rec_.searching = false;
    } else if (key == keys::esc) {
      rec_.searching = false;
      rec_.search = Input("", search_limit);
      rec_.query.search.clear();
      load_records();
    } else if (rec_.search.handle(key)) {
      rec_.query.search = rec_.search.value();
      rec_.cursor = 0;
      load_records();
    }
    return;
  }
  const std::size_t count = rec_.list.size();
  if (move_cursor(rec_.cursor, key, count)) {
    return;
  }
  if (key == keys::home || key.is(U'g')) {
    rec_.cursor = 0;
  } else if (key == keys::end || key.is(U'G')) {
    rec_.cursor = less(count, 1);
  } else if (key == keys::enter) {
    rec_.detail = rec_.selected() != nullptr && !rec_.detail;
  } else if (key == keys::esc) {
    if (rec_.detail) {
      rec_.detail = false;
    } else if (rec_.filtered()) {
      rec_.query.search.clear();
      rec_.query.status.reset();
      rec_.query.severity.reset();
      rec_.search = Input("", search_limit);
      load_records();
    }
  } else if (key.is(U'/')) {
    rec_.searching = true;
    rec_.search = Input(rec_.query.search, search_limit);
  } else if (key.is(U'f')) {
    rec_.query.status = next(records::all_statuses, rec_.query.status);
    load_records();
  } else if (key.is(U'v')) {
    rec_.query.severity = next(theme::all_severities, rec_.query.severity);
    load_records();
  } else if (key.is(U's')) {
    const auto& sorts = records::sort_keys;
    const auto found = std::ranges::find(sorts, rec_.query.sort);
    const std::size_t at = found == sorts.end() ? 0 : static_cast<std::size_t>(found - sorts.begin()) + 1;
    sort_by(sorts.at(at % sorts.size()), rec_.query.descending);
  } else if (key.is(U'S')) {
    const std::string sort = rec_.query.sort;
    sort_by(sort, !rec_.query.descending);
  } else if (key.is(U'c')) {
    column_form();
  } else if (key.is(U'a')) {
    rec_.query.include_disabled = !rec_.query.include_disabled;
    load_records();
  } else if (key.is(U'n')) {
    form_ = OpenForm{record_form("Add record", records::Input{}), Submit{SubmitKind::AddRecord, 0, 0, {}}};
  } else if (key.is(U'i')) {
    form_ = OpenForm{
        Form("Import records",
             {
                 Field::text("path", "File", "",
                             "Path of a CSV or JSON file. Rows are matched by name: new names are added, known "
                             "names are updated. Nothing is written until you confirm the preview.",
                             path_limit),
                 Field::choice("format", "Format", "", "", formats()),
             }),
        Submit{SubmitKind::Import, 0, 0, {}}};
  } else if (key.is(U'o')) {
    form_ = OpenForm{
        Form("Export records",
             {
                 Field::text("path", "File", "records.csv",
                             "Path of the file to write. Every record is exported, including disabled ones. The "
                             "file is readable only by you.",
                             path_limit),
                 Field::choice("format", "Format", "", "", formats()),
                 Field::choice("overwrite", "If the file exists", "false", "",
                               {{"false", "Stop and keep it"}, {"true", "Overwrite it"}}),
             }),
        Submit{SubmitKind::Export, 0, 0, {}}};
  } else if (const records::Record* selected = rec_.selected()) {
    const records::Record record = *selected;
    record_key(key, record);
  }
}

void Model::record_key(Key key, const records::Record& record) {
  if (key.is(U'e')) {
    form_ = OpenForm{record_form(text::concat("Edit record ", record.id), record.input()),
                     Submit{SubmitKind::EditRecord, record.id, record.revision, {}}};
  } else if (key.is(U'x')) {
    records::Record updated;
    if (!attempt([&] { updated = app_->records().set_enabled(record.id, !record.enabled); })) {
      return;
    }
    if (!updated.enabled) {
      rec_.query.include_disabled = true;
    }
    changed(text::concat("Record ", updated.id, " is ", updated.enabled ? "enabled" : "disabled"));
  } else if (key.is(U'd') || key == keys::del) {
    ask("Remove record",
        text::concat("This removes record ", record.id, " (", record.name,
                     ") permanently. Its notifications stay in the list without a link."),
        "remove it", RemoveRecord{record.id});
  }
}

void Model::submit_add(const Form& form) {
  const records::Record created = app_->records().create(form_input(form));
  if (!created.enabled) {
    rec_.query.include_disabled = true;
  }
  changed(text::concat("Added record ", created.id, ": ", created.name));
  rec_.select(created.id);
}

void Model::submit_edit(const Form& form, std::int64_t id, std::int64_t revision) {
  try {
    const records::Record updated = app_->records().update(id, revision, form_input(form));
    changed(text::concat("Updated record ", updated.id));
  } catch (const AppError& error) {
    if (error.kind() != ErrorKind::Conflict) {
      throw;
    }
    reload();
    throw AppError::conflict(
        "another window changed this record while you were editing. Your text is still here: "
        "press Esc, press e again to load the new version, and repeat your change");
  }
}

void Model::keep(std::string_view key, std::string_view raw, std::string_view what) {
  const settings::Value saved = app_->settings().set(key, raw);
  reload();
  if (!saved.overridden_by.empty()) {
    say(Notice::Warning, text::concat(what, " saved, but the ", saved.overridden_by, " still takes precedence"));
  }
}

void Model::sort_by(std::string_view key, bool descending) {
  const std::string raw = text::concat(key, ":", descending ? "desc" : "asc");
  attempt([&] { keep(settings::key_record_sort, raw, "Sort order"); });
}

void Model::column_form() {
  const settings::Value* value = setting(settings::key_record_columns);
  if (value == nullptr) {
    return;
  }
  const std::vector<std::string> stored =
      settings::split_columns(value->stored ? std::string_view(*value->stored) : value->definition.fallback);
  std::vector<Field> fields;
  for (const std::string_view column : value->definition.options) {
    if (std::ranges::find(value->definition.pinned, column) != value->definition.pinned.end()) {
      continue;
    }
    const bool shown = std::ranges::find(stored, column) != stored.end();
    fields.push_back(Field::choice(column, records::column_label(column), shown ? "show" : "hide", "",
                                   {{"show", "Shown"}, {"hide", "Hidden"}}));
  }
  form_ = OpenForm{Form("Columns of the record table", std::move(fields)), Submit{SubmitKind::Columns, 0, 0, {}}};
}

void Model::submit_columns(const Form& form) {
  std::vector<std::string> shown;
  for (const std::string_view column : settings::record_columns()) {
    if (form.value(column) == "show") {
      shown.emplace_back(column);
    }
  }
  keep(settings::key_record_columns, text::join(shown, ","), "Columns");
}

void Model::submit_import(const Form& form) {
  const std::string path(text::trim_whitespace(form.value("path")));
  const std::string format = form.value("format");
  if (path.empty()) {
    throw AppError::usage("give the path of the file to import");
  }
  const transfer::ImportResult result = app_->import_file(files::from_utf8(path), format, true);
  std::vector<std::string> lines = {text::concat("Dry run of ", path, ":"), "", result.summary(), ""};
  for (const transfer::RowError& problem : result.problems) {
    lines.push_back(text::concat("✖ rejected ", problem.display()));
  }
  std::optional<Action> action;
  if (result.created + result.updated == 0) {
    lines.emplace_back("There is nothing to import.");
  } else {
    action = ImportFile{path, format};
  }
  preview_ = Preview{"Import preview", std::move(lines), 0, "import these rows", std::move(action)};
}

void Model::submit_export(const Form& form) {
  const std::string path(text::trim_whitespace(form.value("path")));
  try {
    const std::size_t count =
        app_->export_file(files::from_utf8(path), form.value("format"), form.value("overwrite") == "true");
    say(Notice::Success, text::concat("Exported ", count, " records to ", path));
  } catch (const AppError& error) {
    if (error.kind() != ErrorKind::Conflict) {
      throw;
    }
    throw AppError::conflict(
        text::concat(path, " already exists. Choose another name, or set \"If the file exists\" to \"Overwrite it\""));
  }
}

std::string Model::filter_line() const {
  std::string search;
  if (rec_.searching) {
    search = rec_.search.view(24, true);
    while (!search.empty() && search.back() == ' ') {
      search.pop_back();
    }
  } else {
    search = rec_.query.search.empty() ? "none" : rec_.query.search;
  }
  return text::concat(
      "Search: ", search,
      " · Status: ", rec_.query.status ? records::label(*rec_.query.status) : std::string_view("all"),
      " · Severity: ", rec_.query.severity ? theme::label(*rec_.query.severity) : std::string_view("all"),
      " · Disabled: ", rec_.query.include_disabled ? "shown" : "hidden", " · Sort: ", rec_.query.sort, " ",
      rec_.query.descending ? "descending" : "ascending");
}

Lines Model::records_view(std::size_t size, std::size_t height) {
  Lines lines = wrapped(filter_line(), size, Look::Muted);
  lines.emplace_back();
  if (rec_.list.empty()) {
    std::string_view message =
        "No records yet. Press n to add the first record, i to import a file, or 1 and then "
        "l to load demo data.";
    if (rec_.filtered()) {
      message = "No records match these filters. Press Esc to clear them.";
    } else if (!rec_.query.include_disabled && overview_.disabled > 0) {
      message = "Every record is disabled. Press a to show disabled records.";
    }
    extend(lines, wrapped(message, size, Look::Plain));
    return lines;
  }
  const settings::Value* chosen_value = setting(settings::key_record_columns);
  std::optional<std::vector<std::string>> chosen;
  if (chosen_value != nullptr) {
    chosen = settings::split_columns(chosen_value->raw);
  }
  const std::size_t moment = width(app_->format_time(rec_.list.front().updated_at));
  std::vector<Shown> columns;
  for (const Shown& column :
       {Shown{"id", 5, false}, Shown{"name", 16, true}, Shown{"status", 13, false}, Shown{"severity", 11, false},
        Shown{"updated", moment, false}, Shown{"state", 8, false}}) {
    if (chosen && std::ranges::find(*chosen, column.key) == chosen->end()) {
      continue;
    }
    if (column.key == "updated" && size < 67 + moment) {
      continue;
    }
    columns.push_back(column);
  }
  std::vector<Column> shape;
  std::vector<std::string> titles;
  for (const Shown& column : columns) {
    shape.push_back({column.width, column.flexible});
    titles.push_back(text::upper(records::column_label(column.key)));
  }
  const std::vector<std::size_t> widths = layout(shape, less(size, 2));
  lines.push_back(Line::styled(text::concat("  ", row(titles, widths)), Look::Muted));
  const std::size_t detail_height = rec_.detail ? 8 : 0;
  const std::size_t rows = std::max<std::size_t>(less(height, lines.size() + detail_height + 1), 3);
  if (rec_.cursor < rec_.offset) {
    rec_.offset = rec_.cursor;
  }
  if (rec_.cursor >= rec_.offset + rows) {
    rec_.offset = rec_.cursor + 1 - rows;
  }
  rec_.offset = std::min(rec_.offset, less(rec_.list.size(), rows));
  for (std::size_t index = rec_.offset; index < rec_.list.size() && index < rec_.offset + rows; ++index) {
    const records::Record& record = rec_.list[index];
    std::vector<std::string> cells;
    for (const Shown& column : columns) {
      if (column.key == "id") {
        cells.push_back(std::to_string(record.id));
      } else if (column.key == "name") {
        cells.push_back(record.name);
      } else if (column.key == "status") {
        cells.push_back(text::concat(records::glyph(record.status), " ", records::label(record.status)));
      } else if (column.key == "severity") {
        cells.push_back(text::concat(theme::glyph(record.severity), " ", theme::label(record.severity)));
      } else if (column.key == "updated") {
        cells.push_back(app_->format_time(record.updated_at));
      } else {
        cells.emplace_back(enabled_label(record.enabled));
      }
    }
    lines.push_back(selectable(row(cells, widths), index == rec_.cursor, size));
  }
  lines.push_back(Line::styled(text::concat("Row ", rec_.cursor + 1, " of ", rec_.list.size()), Look::Muted));
  const records::Record* record = rec_.selected();
  if (record == nullptr || !rec_.detail) {
    return lines;
  }
  lines.push_back(Line::styled(repeat("─", size), Look::Border));
  lines.push_back(Line::styled(pad(text::concat("Record ", record->id, ": ", record->name), size), Look::Heading));
  lines.push_back(
      Line::plain(
          text::concat("Status ", records::glyph(record->status), " ", records::label(record->status), " · Severity "))
          .with(text::concat(theme::glyph(record->severity), " ", theme::label(record->severity)),
                look_for(record->severity))
          .with(text::concat(" · ", enabled_label(record->enabled), " · Revision ", record->revision), Look::Plain));
  lines.push_back(Line::styled(text::concat("Created ", app_->format_time(record->created_at), " · Updated ",
                                            app_->format_time(record->updated_at)),
                               Look::Muted));
  Lines note =
      wrapped(record->note.empty() ? std::string_view("No note.") : std::string_view(record->note), size, Look::Plain);
  if (note.size() > 3) {
    note.resize(2);
    note.push_back(Line::plain("… press e to read the whole note"));
  }
  extend(lines, std::move(note));
  return lines;
}

}
