#include <istream>
#include <optional>
#include <ranges>
#include <utility>

#include <nlohmann/json.hpp>

#include "app/command.hpp"
#include "core/files.hpp"
#include "core/meta.hpp"
#include "core/records.hpp"
#include "core/text.hpp"
#include "core/theme.hpp"
#include "core/transfer.hpp"

namespace app::cli {

namespace {

using records::Record;
using theme::Severity;
using transfer::Format;

nlohmann::ordered_json to_json(const Record& record) {
  nlohmann::ordered_json row;
  row["id"] = record.id;
  row["name"] = record.name;
  row["status"] = records::name(record.status);
  row["severity"] = theme::name(record.severity);
  row["note"] = record.note;
  row["enabled"] = record.enabled;
  row["demo"] = record.demo;
  row["revision"] = record.revision;
  row["created_at"] = record.created_at.to_rfc3339();
  row["updated_at"] = record.updated_at.to_rfc3339();
  return row;
}

std::vector<Flag> input_flags() {
  return {
      text_flag("name", "", "name, 1 to 200 characters"),
      text_flag("status", "", "open, in_progress or done"),
      text_flag("severity", "", "info, low, medium, high or critical"),
      text_flag("note", "", "free text, up to 4000 characters"),
  };
}

std::string status_text(const Record& record) {
  return text::concat(records::glyph(record.status), " ", records::label(record.status));
}

std::string severity_text(Severity severity) {
  return text::concat(theme::glyph(severity), " ", theme::label(severity));
}

void run_overview(Env& env, const Call& call, service::App& app) {
  call.no_arguments();
  const records::Overview overview = app.records().overview();
  const std::string_view format = call.text("format");
  if (format == "json") {
    nlohmann::ordered_json document;
    document["open_records"] = overview.open;
    document["urgent_records"] = overview.urgent;
    document["disabled_records"] = overview.disabled;
    document["demo_records"] = overview.demo;
    document["unread_notifications"] = overview.unread_notifications;
    nlohmann::ordered_json breakdown = nlohmann::ordered_json::array();
    for (const records::Breakdown& row : overview.breakdown) {
      nlohmann::ordered_json item;
      item["status"] = records::name(row.status);
      item["severity"] = theme::name(row.severity);
      item["total"] = row.total;
      item["demo"] = row.demo;
      breakdown.push_back(std::move(item));
    }
    document["breakdown"] = std::move(breakdown);
    json(env, document);
    return;
  }
  if (format != "table") {
    throw usage(text::concat("unknown format \"", format, "\": use table or json"));
  }
  table(env, {"MEASURE", "COUNT"},
        {
            {"Open records", std::to_string(overview.open)},
            {"Open records with high or critical severity", std::to_string(overview.urgent)},
            {"Disabled records", std::to_string(overview.disabled)},
            {"Demo records", std::to_string(overview.demo)},
            {"Unread notifications", std::to_string(overview.unread_notifications)},
        });
  if (overview.total() == 0 && overview.disabled == 0) {
    line(env, text::concat("\nNo records yet. Add one with \"", meta::slug,
                           " records add --name <name>\" or load samples with \"", meta::slug, " demo load\"."));
    return;
  }
  line(env, "\nOpen and in-progress records by severity");
  std::vector<std::vector<std::string>> rows;
  for (const theme::Severity level : std::views::reverse(theme::all_severities)) {
    rows.push_back({severity_text(level), std::to_string(overview.by_severity(level))});
  }
  table(env, {"SEVERITY", "COUNT"}, rows);
  if (overview.demo > 0) {
    line(env, text::concat("\nDemo data is loaded: ", overview.demo, " sample records are labeled \"Demo:\"."));
  }
}

std::optional<records::Status> optional_status(std::string_view raw) {
  if (raw.empty()) {
    return std::nullopt;
  }
  return records::parse_status(raw);
}

std::optional<Severity> optional_severity(std::string_view raw) {
  if (raw.empty()) {
    return std::nullopt;
  }
  return records::parse_severity(raw);
}

void run_list(Env& env, const Call& call, service::App& app) {
  call.no_arguments();
  records::Query query;
  query.search = std::string(call.text("search"));
  query.status = optional_status(call.text("status"));
  query.severity = optional_severity(call.text("severity"));
  query.include_disabled = call.truth("all");
  query.sort = std::string(call.text("sort"));
  query.descending = call.truth("desc");
  query.limit = call.number("limit");
  const std::vector<Record> list = app.records().list(query);
  const std::string_view format = call.text("format");
  if (format == "json") {
    nlohmann::ordered_json rows = nlohmann::ordered_json::array();
    for (const Record& record : list) {
      rows.push_back(to_json(record));
    }
    json(env, rows);
    return;
  }
  if (format == "csv") {
    print(env, transfer::export_text(Format::Csv, list));
    return;
  }
  if (format != "table") {
    throw usage(text::concat("unknown format \"", format, "\": use table, json or csv"));
  }
  if (list.empty()) {
    if (query.search.empty() && !query.status && !query.severity) {
      line(env, text::concat("No records yet. Add one with \"", meta::slug, " records add --name <name>\"."));
    } else {
      line(env, "No records match. Remove a filter or add --all to include disabled records.");
    }
    return;
  }
  std::vector<std::vector<std::string>> rows;
  for (const Record& record : list) {
    rows.push_back({std::to_string(record.id), record.name, status_text(record), severity_text(record.severity),
                    std::string(enabled_text(record.enabled)), app.format_time(record.updated_at)});
  }
  table(env, {"ID", "NAME", "STATUS", "SEVERITY", "ENABLED", "UPDATED"}, rows);
}

void run_show(Env& env, const Call& call, service::App& app) {
  const std::int64_t id = call.identifier("record");
  const Record record = app.records().get(id);
  const std::string_view format = call.text("format");
  if (format == "json") {
    json(env, to_json(record));
    return;
  }
  if (format != "table") {
    throw usage(text::concat("unknown format \"", format, "\": use table or json"));
  }
  table(env, {"FIELD", "VALUE"},
        {
            {"ID", std::to_string(record.id)},
            {"Name", record.demo ? text::concat(record.name, " (demo data)") : record.name},
            {"Status", status_text(record)},
            {"Severity", severity_text(record.severity)},
            {"Enabled", std::string(enabled_text(record.enabled))},
            {"Revision", std::to_string(record.revision)},
            {"Created", app.format_time(record.created_at)},
            {"Updated", app.format_time(record.updated_at)},
            {"Note", text::replace_all(record.note, "\n", " / ")},
        });
}

bool apply_input_flags(const Call& call, records::Input& input) {
  bool changed = false;
  if (call.given("name")) {
    input.name = std::string(call.text("name"));
    changed = true;
  }
  if (call.given("status")) {
    input.status = records::parse_status(call.text("status"));
    changed = true;
  }
  if (call.given("severity")) {
    input.severity = records::parse_severity(call.text("severity"));
    changed = true;
  }
  if (call.given("note")) {
    input.note = std::string(call.text("note"));
    changed = true;
  }
  return changed;
}

void run_add(Env& env, const Call& call, service::App& app) {
  call.no_arguments();
  if (!call.given("name")) {
    throw usage("a name is required: add --name <name>");
  }
  records::Input input;
  input.enabled = !call.truth("disabled");
  apply_input_flags(call, input);
  const Record created = app.records().create(input);
  line(env, text::concat("Added record ", created.id, ": ", created.name));
}

void run_edit(Env& env, const Call& call, service::App& app) {
  const std::int64_t id = call.identifier("record");
  const Record current = app.records().get(id);
  records::Input change = current.input();
  if (!apply_input_flags(call, change)) {
    throw usage("nothing to change: give at least one of --name, --status, --severity or --note");
  }
  const Record updated = app.records().update(id, current.revision, change);
  line(env, text::concat("Updated record ", updated.id, " (revision ", updated.revision, ")"));
}

void run_toggle(Env& env, const Call& call, service::App& app, bool enabled) {
  const std::int64_t id = call.identifier("record");
  const Record updated = app.records().set_enabled(id, enabled);
  line(env, text::concat("Record ", updated.id, " is ", updated.enabled ? "enabled" : "disabled"));
}

void run_remove(Env& env, const Call& call, service::App& app) {
  const std::int64_t id = call.identifier("record");
  const Record record = app.records().get(id);
  const std::string statement = text::concat("This removes record ", record.id, " (", record.name, ") permanently.");
  if (call.truth("dry-run")) {
    line(env, text::concat("Dry run: ", statement, " Nothing was changed."));
    return;
  }
  confirm(env, call, &app, statement);
  app.records().remove(id);
  line(env, text::concat("Removed record ", id));
}

std::string read_bounded(std::istream& input, std::size_t limit) {
  std::string raw;
  std::string block(65536, '\0');
  while (raw.size() < limit && input) {
    input.read(block.data(), static_cast<std::streamsize>(std::min(block.size(), limit - raw.size())));
    raw.append(block.data(), static_cast<std::size_t>(input.gcount()));
  }
  if (input.bad()) {
    throw failure("cannot read standard input: the stream reported an error");
  }
  return raw;
}

void run_import(Env& env, const Call& call, service::App& app) {
  if (call.args.size() != 1) {
    throw usage("give one file to import, or - to read standard input");
  }
  const std::string& source = call.args.front();
  const std::string lowered = text::lower(source);
  if (call.text("format").empty() && !lowered.ends_with(".csv") && !lowered.ends_with(".json")) {
    throw usage("the format cannot be told from the file name: add --format csv or --format json");
  }
  const Format format = transfer::format_for_file(source, call.text("format"));
  std::string raw;
  if (source == "-") {
    raw = read_bounded(*env.input, transfer::max_import_bytes + 1);
  } else {
    try {
      raw = files::read_limited(files::from_utf8(source), transfer::max_import_bytes);
    } catch (const AppError& error) {
      throw failure(text::concat("cannot open the import file: ", error.message()));
    }
  }
  const transfer::ImportResult result = transfer::import_records(app.records(), raw, format, call.truth("dry-run"));
  line(env, capitalize(result.summary()));
  for (const transfer::RowError& problem : result.problems) {
    note(env, text::concat("rejected ", problem.display()));
  }
  if (!result.problems.empty() && result.created + result.updated + result.skipped == 0) {
    throw failure("no row could be imported: fix the rows listed above");
  }
}

void run_export(Env& env, const Call& call, service::App& app) {
  call.no_arguments();
  const std::string_view output = call.text("output");
  if (output.empty()) {
    print(env, app.export_text(transfer::parse_format(call.text("format"))).text);
    return;
  }
  Format format = Format::Csv;
  if (call.given("format")) {
    format = transfer::parse_format(call.text("format"));
  } else {
    try {
      format = transfer::format_for_file(output, "");
    } catch (const AppError&) {
      format = Format::Csv;
    }
  }
  try {
    const std::size_t count = app.export_file(files::from_utf8(output), transfer::label(format), call.truth("force"));
    line(env, text::concat("Exported ", count, " records to ", output));
  } catch (const AppError& error) {
    if (error.kind() == ErrorKind::Conflict) {
      throw usage(text::concat(output, " already exists: choose another name or add --force to overwrite it"));
    }
    throw;
  }
}

void run_demo_load(Env& env, const Call& call, service::App& app) {
  call.no_arguments();
  const std::size_t added = app.records().load_demo();
  if (added == 0) {
    line(env, "Demo data is already loaded.");
  } else {
    line(env, text::concat("Added ", added, " demo records. Their names start with \"Demo:\"."));
  }
}

void run_demo_remove(Env& env, const Call& call, service::App& app) {
  call.no_arguments();
  const std::int64_t count = app.records().remove_demo(true);
  const std::string statement = text::concat("This removes ", count, " demo records. Records you added yourself stay.");
  if (call.truth("dry-run")) {
    line(env, text::concat("Dry run: ", statement, " Nothing was changed."));
    return;
  }
  if (count == 0) {
    line(env, "There is no demo data to remove.");
    return;
  }
  confirm(env, call, &app, statement);
  const std::int64_t removed = app.records().remove_demo(false);
  line(env, text::concat("Removed ", removed, " demo records"));
}

}

Command overview_command() {
  return Command::leaf("overview", "Show what needs attention", run_overview)
      .with({text_flag("format", "table", "table or json")});
}

Command records_command() {
  std::vector<Flag> add_flags = input_flags();
  add_flags.push_back(toggle("disabled", "add the record in the disabled state"));
  return Command::group(
      "records", "Manage records",
      {
          Command::leaf("list", "List, search, filter and sort records", run_list)
              .with({
                  text_flag("search", "", "words to find in names and notes"),
                  text_flag("status", "", "only this status"),
                  text_flag("severity", "", "only this severity"),
                  toggle("all", "include disabled records"),
                  text_flag("sort", "name", "name, status, severity or updated"),
                  toggle("desc", "sort in descending order"),
                  number_flag("limit", "500", "largest number of rows"),
                  text_flag("format", "table", "table, json or csv"),
              }),
          Command::leaf("show", "Show one record", run_show)
              .taking("<id>")
              .with({text_flag("format", "table", "table or json")}),
          Command::leaf("add", "Add a record", run_add).with(std::move(add_flags)),
          Command::leaf("edit", "Change fields of a record", run_edit).taking("<id>").with(input_flags()),
          Command::leaf("enable", "Enable a record",
                        [](Env& env, const Call& call, service::App& app) { run_toggle(env, call, app, true); })
              .taking("<id>"),
          Command::leaf("disable", "Disable a record",
                        [](Env& env, const Call& call, service::App& app) { run_toggle(env, call, app, false); })
              .taking("<id>"),
          Command::leaf("remove", "Remove a record", run_remove)
              .taking("<id>")
              .with({
                  toggle("yes", "do not ask for confirmation"),
                  toggle("dry-run", "show what would be removed without removing it"),
              }),
          Command::leaf("import", "Import records from a CSV or JSON file, or from standard input with -", run_import)
              .taking("<file|->")
              .with({
                  text_flag("format", "", "csv or json, by default taken from the file extension"),
                  toggle("dry-run", "validate and report without storing anything"),
              }),
          Command::leaf("export", "Export records as CSV or JSON", run_export)
              .with({
                  text_flag("format", "csv", "csv or json"),
                  text_flag("output", "", "file to write, by default standard output"),
                  toggle("force", "overwrite the output file if it exists"),
              }),
      });
}

Command demo_command() {
  return Command::group("demo", "Load or remove labeled demo data",
                        {
                            Command::leaf("load", "Add sample records labeled as demo data", run_demo_load),
                            Command::leaf("remove", "Remove every demo record", run_demo_remove)
                                .with({
                                    toggle("yes", "do not ask for confirmation"),
                                    toggle("dry-run", "show how many records would be removed"),
                                }),
                        });
}

}
