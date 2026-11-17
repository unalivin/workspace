#include "core/transfer.hpp"

#include <map>
#include <optional>
#include <utility>

#include <nlohmann/json.hpp>

#include "core/csv.hpp"
#include "core/error.hpp"
#include "core/text.hpp"

namespace app::transfer {

namespace {

constexpr std::string_view layout_hint =
    ": the file must hold a list of objects with name, status, severity, note and enabled";

struct Exported {
  std::string name;
  std::string status;
  std::string severity;
  std::string note;
  std::optional<bool> enabled;
};

struct Candidate {
  std::size_t row = 0;
  Exported item;
  std::optional<AppError> rejected;
};

bool parse_bool(std::string_view value) {
  const std::string lowered = text::lower(text::trim_whitespace(value));
  if (lowered.empty() || lowered == "true" || lowered == "yes" || lowered == "1" || lowered == "on") {
    return true;
  }
  if (lowered == "false" || lowered == "no" || lowered == "0" || lowered == "off") {
    return false;
  }
  throw AppError::invalid("enabled", text::concat("\"", value, "\" is not a yes or no value"), "Use true or false");
}

std::vector<Candidate> csv_candidates(std::string_view content) {
  std::vector<csv::Row> rows = csv::parse(content, max_import_rows + 1);
  std::map<std::string, std::size_t, std::less<>> position;
  if (!rows.empty()) {
    for (std::size_t index = 0; index < rows.front().size(); ++index) {
      position[text::lower(text::trim_whitespace(rows.front()[index]))] = index;
    }
  }
  if (!position.contains("name")) {
    throw AppError::usage(
        "the CSV header has no \"name\" column: the first line must be name,status,severity,note,enabled");
  }
  std::vector<Candidate> candidates;
  for (std::size_t index = 1; index < rows.size(); ++index) {
    const csv::Row& fields = rows[index];
    const auto cell = [&position, &fields](std::string_view column) -> std::string_view {
      const auto found = position.find(column);
      if (found == position.end() || found->second >= fields.size()) {
        return {};
      }
      return fields[found->second];
    };
    Candidate candidate;
    candidate.row = index + 1;
    if (position.contains("enabled")) {
      try {
        candidate.item.enabled = parse_bool(cell("enabled"));
      } catch (const AppError& error) {
        candidate.rejected = error;
      }
    }
    candidate.item.name = std::string(csv::unguard_cell(cell("name")));
    candidate.item.status = std::string(cell("status"));
    candidate.item.severity = std::string(cell("severity"));
    candidate.item.note = std::string(csv::unguard_cell(cell("note")));
    candidates.push_back(std::move(candidate));
  }
  return candidates;
}

AppError unreadable_json(std::string_view detail) {
  return AppError::usage(text::concat("the JSON cannot be read: ", detail, layout_hint));
}

std::string text_member(const nlohmann::json& value, std::size_t row, std::string_view key) {
  if (!value.is_string()) {
    throw unreadable_json(text::concat("item ", row, ": \"", key, "\" must be text"));
  }
  return value.get<std::string>();
}

Exported json_item(const nlohmann::json& object, std::size_t row) {
  if (!object.is_object()) {
    throw unreadable_json(text::concat("item ", row, " is not an object"));
  }
  Exported item;
  bool named = false;
  for (const auto& [key, value] : object.items()) {
    if (key == "name") {
      item.name = text_member(value, row, key);
      named = true;
    } else if (key == "status") {
      item.status = text_member(value, row, key);
    } else if (key == "severity") {
      item.severity = text_member(value, row, key);
    } else if (key == "note") {
      item.note = text_member(value, row, key);
    } else if (key == "enabled") {
      if (value.is_boolean()) {
        item.enabled = value.get<bool>();
      } else if (!value.is_null()) {
        throw unreadable_json(text::concat("item ", row, ": \"enabled\" must be true or false"));
      }
    } else {
      throw unreadable_json(text::concat("item ", row, ": unknown field \"", text::truncated(key, 40), "\""));
    }
  }
  if (!named) {
    throw unreadable_json(text::concat("item ", row, ": missing field \"name\""));
  }
  return item;
}

std::vector<Candidate> json_candidates(std::string_view content) {
  nlohmann::json document;
  try {
    document = nlohmann::json::parse(content);
  } catch (const nlohmann::json::exception& error) {
    throw unreadable_json(error.what());
  }
  if (!document.is_array()) {
    throw unreadable_json("the top level is not a list");
  }
  if (document.size() > max_import_rows) {
    throw AppError::usage(
        text::concat("the import has more than ", max_import_rows, " rows: split it into smaller files"));
  }
  std::vector<Candidate> candidates;
  std::size_t row = 0;
  for (const nlohmann::json& object : document) {
    ++row;
    candidates.push_back({row, json_item(object, row), std::nullopt});
  }
  return candidates;
}

records::Input to_input(const Exported& item) {
  records::Input input;
  input.name = item.name;
  input.status =
      text::trim_whitespace(item.status).empty() ? records::Status::Open : records::parse_status(item.status);
  input.severity =
      text::trim_whitespace(item.severity).empty() ? theme::Severity::Info : records::parse_severity(item.severity);
  input.note = item.note;
  input.enabled = item.enabled.value_or(true);
  return records::normalize(input);
}

}

Format parse_format(std::string_view value) {
  const std::string lowered = text::lower(text::trim_whitespace(value));
  if (lowered == "csv") {
    return Format::Csv;
  }
  if (lowered == "json") {
    return Format::Json;
  }
  throw AppError::usage(text::concat("unknown format \"", lowered, "\": use csv or json"));
}

Format format_for_file(std::string_view name, std::string_view explicit_format) {
  if (!explicit_format.empty()) {
    return parse_format(explicit_format);
  }
  const std::string lowered = text::lower(name);
  if (lowered.ends_with(".csv")) {
    return Format::Csv;
  }
  if (lowered.ends_with(".json")) {
    return Format::Json;
  }
  throw AppError::usage(text::concat("the format of \"", name,
                                     "\" is not clear from its name: use a .csv or .json file name, or pass "
                                     "--format csv or --format json"));
}

std::string_view label(Format format) {
  return format == Format::Csv ? "csv" : "json";
}

std::string export_text(Format format, const std::vector<records::Record>& list) {
  if (format == Format::Csv) {
    std::string output;
    csv::write_row(output, {"name", "status", "severity", "note", "enabled"});
    for (const records::Record& record : list) {
      const std::string guarded_name = csv::guard_cell(record.name);
      const std::string guarded_note = csv::guard_cell(record.note);
      csv::write_row(output, {guarded_name, records::name(record.status), theme::name(record.severity), guarded_note,
                              record.enabled ? "true" : "false"});
    }
    return output;
  }
  nlohmann::ordered_json rows = nlohmann::ordered_json::array();
  for (const records::Record& record : list) {
    nlohmann::ordered_json row;
    row["name"] = record.name;
    row["status"] = records::name(record.status);
    row["severity"] = theme::name(record.severity);
    row["note"] = record.note;
    row["enabled"] = record.enabled;
    rows.push_back(std::move(row));
  }
  return rows.dump(2, ' ', false, nlohmann::ordered_json::error_handler_t::replace) + "\n";
}

std::string RowError::display() const {
  return text::concat("row ", row, ": ", message);
}

Parsed parse(std::string_view raw, Format format) {
  if (raw.size() > max_import_bytes) {
    throw AppError::usage(
        text::concat("the import is larger than ", max_import_bytes >> 20U, " MB: split it into smaller files"));
  }
  if (!text::is_valid_utf8(raw)) {
    throw AppError::usage("the import is not valid UTF-8 text: save it as UTF-8 and try again");
  }
  std::string_view content = raw;
  if (content.starts_with("\xEF\xBB\xBF")) {
    content.remove_prefix(3);
  }
  if (text::trim_whitespace(content).empty()) {
    throw AppError::usage("the import is empty: export a list first to see the expected layout");
  }
  const std::vector<Candidate> candidates = format == Format::Csv ? csv_candidates(content) : json_candidates(content);
  Parsed parsed;
  std::map<std::string, std::size_t, std::less<>> seen;
  for (const Candidate& candidate : candidates) {
    try {
      if (candidate.rejected) {
        throw AppError(*candidate.rejected);
      }
      records::Input input = to_input(candidate.item);
      std::string key = text::lower(input.name);
      if (const auto first = seen.find(key); first != seen.end()) {
        parsed.problems.push_back(
            {candidate.row, text::concat("name: the name repeats row ", first->second, ". Keep one row per name")});
      } else {
        seen.emplace(std::move(key), candidate.row);
        parsed.inputs.push_back(std::move(input));
      }
    } catch (const AppError& error) {
      parsed.problems.push_back({candidate.row, error.message()});
    }
  }
  return parsed;
}

std::string ImportResult::summary() const {
  return text::concat(dry_run ? "Dry run: would have " : "", "created ", created, ", updated ", updated, ", unchanged ",
                      skipped, ", rejected ", text::plural(static_cast<std::int64_t>(problems.size()), "row", "rows"));
}

ImportResult import_records(const records::Records& records, std::string_view raw, Format format, bool dry_run) {
  Parsed parsed = parse(raw, format);
  store::Transaction transaction(records.database());
  const records::ImportTally tally =
      records::apply_import(records.database(), records.now(), records.zone(), parsed.inputs);
  if (dry_run) {
    transaction.rollback();
  } else {
    transaction.commit();
  }
  return {tally.created, tally.updated, tally.skipped, std::move(parsed.problems), dry_run};
}

}
