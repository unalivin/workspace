#include "core/records.hpp"

#include <algorithm>

#include "core/error.hpp"
#include "core/text.hpp"

namespace app::records {

using clock::Timestamp;
using clock::Zone;
using store::Statement;
using store::Store;
using theme::Severity;

namespace {

constexpr std::string_view sql_insert =
    "INSERT INTO records (name, status, severity, note, enabled, is_demo, created_at, updated_at) VALUES (?1, ?2, ?3, "
    "?4, ?5, ?6, ?7, ?7)";
constexpr std::string_view sql_get =
    "SELECT id, name, status, severity, note, enabled, is_demo, revision, created_at, updated_at FROM records WHERE "
    "id = ?1";
constexpr std::string_view sql_get_by_name =
    "SELECT id, name, status, severity, note, enabled, is_demo, revision, created_at, updated_at FROM records WHERE "
    "name = ?1";
constexpr std::string_view sql_update =
    "UPDATE records SET name = ?1, status = ?2, severity = ?3, note = ?4, enabled = ?5, revision = revision + 1, "
    "updated_at = ?6 WHERE id = ?7 AND revision = ?8";
constexpr std::string_view sql_set_enabled =
    "UPDATE records SET enabled = ?1, revision = revision + 1, updated_at = ?2 WHERE id = ?3 AND enabled <> ?1";
constexpr std::string_view sql_delete = "DELETE FROM records WHERE id = ?1";
constexpr std::string_view sql_delete_demo = "DELETE FROM records WHERE is_demo = 1";
constexpr std::string_view sql_count_demo = "SELECT count(*) FROM records WHERE is_demo = 1";
constexpr std::string_view sql_count = "SELECT count(*) FROM records";
constexpr std::string_view sql_list =
    "SELECT id, name, status, severity, note, enabled, is_demo, revision, created_at, updated_at\n"
    "FROM records\n"
    "WHERE (?1 = '' OR status = ?1)\n"
    "  AND (?2 = '' OR severity = ?2)\n"
    "  AND (?3 = 1 OR enabled = 1)\n"
    "  AND (?4 = '' OR id IN (SELECT rowid FROM records_fts WHERE records_fts MATCH ?4))\n"
    "ORDER BY\n"
    "  CASE WHEN ?5 = 'name' AND ?6 = 0 THEN name END COLLATE NOCASE ASC,\n"
    "  CASE WHEN ?5 = 'name' AND ?6 = 1 THEN name END COLLATE NOCASE DESC,\n"
    "  CASE WHEN ?5 = 'status' AND ?6 = 0 THEN status END ASC,\n"
    "  CASE WHEN ?5 = 'status' AND ?6 = 1 THEN status END DESC,\n"
    "  CASE WHEN ?5 = 'severity' AND ?6 = 0 THEN CASE severity WHEN 'critical' THEN 4 WHEN 'high' THEN 3 WHEN "
    "'medium' THEN 2 WHEN 'low' THEN 1 ELSE 0 END END ASC,\n"
    "  CASE WHEN ?5 = 'severity' AND ?6 = 1 THEN CASE severity WHEN 'critical' THEN 4 WHEN 'high' THEN 3 WHEN "
    "'medium' THEN 2 WHEN 'low' THEN 1 ELSE 0 END END DESC,\n"
    "  CASE WHEN ?5 = 'updated' AND ?6 = 0 THEN updated_at END ASC,\n"
    "  CASE WHEN ?5 = 'updated' AND ?6 = 1 THEN updated_at END DESC,\n"
    "  id ASC\n"
    "LIMIT ?7 OFFSET ?8";
constexpr std::string_view sql_by_status = "SELECT id FROM records WHERE status = ?1 AND severity = ?2";
constexpr std::string_view sql_recent =
    "SELECT id, name, status, severity, note, enabled, is_demo, revision, created_at, updated_at FROM records ORDER "
    "BY updated_at DESC LIMIT ?1";
constexpr std::string_view sql_overview =
    "SELECT open_records, urgent_records, disabled_records, demo_records, unread_notifications FROM "
    "overview_attention";
constexpr std::string_view sql_overview_rows = "SELECT status, severity, total, demo_total FROM overview_record_counts";

constexpr std::array<DemoRecord, 6> demo_set = {{
    {"Demo: edge router firmware review", Status::Open, Severity::High,
     "Sample entry. Compare the installed firmware with the vendor advisory list.", true},
    {"Demo: rotate backup credentials", Status::InProgress, Severity::Critical,
     "Sample entry. Rotation is scheduled for the next maintenance window.", true},
    {"Demo: document the lab network", Status::Open, Severity::Low,
     "Sample entry. Diagram the lab VLANs and their gateways.", true},
    {"Demo: review firewall change log", Status::Done, Severity::Medium,
     "Sample entry. No unexpected rules were found.", true},
    {"Demo: archive old inventory export", Status::Open, Severity::Info,
     "Sample entry. Disabled to show how hidden entries look.", false},
    {"Demo: check time synchronisation", Status::Open, Severity::Medium,
     "Sample entry. Two hosts drifted by more than a second.", true},
}};

Record build(const Statement& row) {
  Record record;
  record.id = row.integer(0);
  const auto unreadable = [&record](std::string_view what) {
    return AppError::storage(text::concat("record ", record.id, " has an unreadable ", what));
  };
  record.name = row.text(1);
  const std::optional<Status> status = find_status(row.text(2));
  const std::optional<Severity> severity = theme::parse_severity(row.text(3));
  const std::optional<Timestamp> created = Timestamp::parse(row.text(8));
  const std::optional<Timestamp> updated = Timestamp::parse(row.text(9));
  if (!status) {
    throw unreadable("status");
  }
  if (!severity) {
    throw unreadable("severity");
  }
  if (!created) {
    throw unreadable("creation time");
  }
  if (!updated) {
    throw unreadable("update time");
  }
  record.status = *status;
  record.severity = *severity;
  record.note = row.text(4);
  record.enabled = row.integer(5) == 1;
  record.demo = row.integer(6) == 1;
  record.revision = row.integer(7);
  record.created_at = *created;
  record.updated_at = *updated;
  return record;
}

std::optional<Record> find(const Store& database, std::int64_t id) {
  Statement statement = database.prepare(sql_get);
  statement.bind(id);
  if (!statement.step()) {
    return std::nullopt;
  }
  return build(statement);
}

std::optional<Record> find_by_name(const Store& database, std::string_view wanted) {
  Statement statement = database.prepare(sql_get_by_name);
  statement.bind(wanted);
  if (!statement.step()) {
    return std::nullopt;
  }
  return build(statement);
}

AppError not_found(std::int64_t id) {
  return AppError::not_found(text::concat("record ", id, " (list the records to see valid identifiers)"));
}

Record require(const Store& database, std::int64_t id) {
  std::optional<Record> found = find(database, id);
  if (!found) {
    throw not_found(id);
  }
  return std::move(*found);
}

bool urgent(Status status, Severity severity) {
  return status != Status::Done && severity >= Severity::High;
}

void run_named(Statement& statement, std::string_view record_name) {
  try {
    statement.run();
  } catch (const store::SqliteError& error) {
    if (error.is_constraint() && text::contains(error.message(), "records.name")) {
      throw AppError::invalid("name", text::concat("a record named \"", record_name, "\" already exists"),
                              "Choose another name or edit the existing record");
    }
    throw;
  }
}

void announce(const Store& database, Timestamp now, Zone zone, const Record* before, const Record& after) {
  if (!after.enabled) {
    return;
  }
  const auto event = [&after](std::string_view event_type, Severity severity, std::string_view title) {
    return notify::Event{std::string(event_type), severity, std::string(title), std::string(kind), after.id, {}};
  };
  const bool was_urgent = before != nullptr && before->enabled && urgent(before->status, before->severity);
  if (urgent(after.status, after.severity) && !was_urgent) {
    notify::publish_with(database, now, zone,
                         event(notify::event_record_urgent, after.severity, "A record needs attention"));
  }
  if (after.status == Status::Done && before != nullptr && before->status != Status::Done) {
    notify::publish_with(database, now, zone,
                         event(notify::event_record_completed, Severity::Info, "A record was completed"));
  }
}

Record create_row(const Store& database, Timestamp now, Zone zone, const Input& input, bool demo) {
  Statement insert = database.prepare(sql_insert);
  insert.bind(input.name)
      .bind(name(input.status))
      .bind(theme::name(input.severity))
      .bind(input.note)
      .bind(input.enabled ? 1 : 0)
      .bind(demo ? 1 : 0)
      .bind(now.to_rfc3339());
  run_named(insert, input.name);
  const Record created = require(database, database.last_insert_id());
  announce(database, now, zone, nullptr, created);
  return created;
}

void bind_update(Statement& statement, const Input& input, Timestamp now, std::int64_t id, std::int64_t revision) {
  statement.bind(input.name)
      .bind(name(input.status))
      .bind(theme::name(input.severity))
      .bind(input.note)
      .bind(input.enabled ? 1 : 0)
      .bind(now.to_rfc3339())
      .bind(id)
      .bind(revision);
}

std::int64_t single_integer(const Store& database, std::string_view sql) {
  Statement statement = database.prepare(sql);
  return statement.step() ? statement.integer(0) : 0;
}

bool unwanted_control(char32_t letter, bool allow_breaks) {
  return text::is_control(letter) && (!allow_breaks || (letter != U'\n' && letter != U'\t'));
}

}

std::vector<notify::HotQuery> hot_queries() {
  return {
      {"records by status and severity", sql_by_status, 2},
      {"recently updated records", sql_recent, 1},
      {"record by name", sql_get_by_name, 1},
      {"record by identifier", sql_get, 1},
  };
}

std::optional<Status> find_status(std::string_view text) {
  const std::string wanted = text::lower(text::trim(text));
  for (const Status status : all_statuses) {
    if (name(status) == wanted) {
      return status;
    }
  }
  return std::nullopt;
}

std::string_view name(Status status) {
  switch (status) {
    case Status::Open:
      return "open";
    case Status::InProgress:
      return "in_progress";
    case Status::Done:
      return "done";
  }
  return "open";
}

std::string_view label(Status status) {
  switch (status) {
    case Status::Open:
      return "Open";
    case Status::InProgress:
      return "In progress";
    case Status::Done:
      return "Done";
  }
  return "Open";
}

std::string_view glyph(Status status) {
  switch (status) {
    case Status::Open:
      return "\xE2\x97\x8B";
    case Status::InProgress:
      return "\xE2\x97\x90";
    case Status::Done:
      return "\xE2\x97\x8F";
  }
  return "\xE2\x97\x8B";
}

std::string_view column_label(std::string_view column) {
  if (column == "id") {
    return "ID";
  }
  if (column == "name") {
    return "Name";
  }
  if (column == "status") {
    return "Status";
  }
  if (column == "severity") {
    return "Severity";
  }
  if (column == "updated") {
    return "Updated";
  }
  if (column == "state") {
    return "State";
  }
  return column;
}

Input Record::input() const {
  return Input{name, status, severity, note, enabled};
}

std::string_view Record::state_label() const {
  if (enabled) {
    return demo ? "Active, demo" : "Active";
  }
  return demo ? "Disabled, demo" : "Disabled";
}

Status parse_status(std::string_view text) {
  const std::optional<Status> status = find_status(text);
  if (!status) {
    throw AppError::invalid("status", text::concat("unknown status \"", text, "\""), "Use open, in_progress or done");
  }
  return *status;
}

Severity parse_severity(std::string_view text) {
  const std::optional<Severity> severity = theme::parse_severity(text);
  if (!severity) {
    throw AppError::invalid("severity", text::concat("unknown severity \"", text, "\""),
                            "Use info, low, medium, high or critical");
  }
  return *severity;
}

Input normalize(const Input& input) {
  if (!text::is_valid_utf8(input.name)) {
    throw AppError::invalid("name", "the name is not valid UTF-8 text", "Enter the name as UTF-8 text");
  }
  if (!text::is_valid_utf8(input.note)) {
    throw AppError::invalid("note", "the note is not valid UTF-8 text", "Enter the note as UTF-8 text");
  }
  std::string clean_name = text::collapse_whitespace(input.name);
  const std::u32string name_letters = text::decode(clean_name);
  if (std::ranges::any_of(name_letters, [](char32_t letter) { return unwanted_control(letter, false); })) {
    throw AppError::invalid("name", "the name contains a control character",
                            "Use letters, digits, spaces and punctuation");
  }
  if (clean_name.empty()) {
    throw AppError::invalid("name", "the name is empty", "Enter a name of 1 to 200 characters");
  }
  if (name_letters.size() > max_name_length) {
    throw AppError::invalid("name", text::concat("the name has more than ", max_name_length, " characters"),
                            "Shorten the name");
  }
  std::string clean_note(text::trim_whitespace(text::replace_all(input.note, "\r\n", "\n")));
  const std::u32string note_letters = text::decode(clean_note);
  if (std::ranges::any_of(note_letters, [](char32_t letter) { return unwanted_control(letter, true); })) {
    throw AppError::invalid("note", "the note contains a control character", "Remove the control character");
  }
  if (note_letters.size() > max_note_length) {
    throw AppError::invalid("note", text::concat("the note has more than ", max_note_length, " characters"),
                            "Shorten the note");
  }
  return Input{std::move(clean_name), input.status, input.severity, std::move(clean_note), input.enabled};
}

std::string search_expression(std::string_view text) {
  std::vector<std::string> terms;
  std::u32string field;
  const auto finish = [&terms, &field] {
    if (!field.empty() && terms.size() < 16) {
      terms.push_back(text::concat("\"", text::encode(std::u32string_view(field).substr(0, 64)), "\"*"));
    }
    field.clear();
  };
  for (const char32_t letter : text::decode(text)) {
    if (text::is_word(letter)) {
      field.push_back(letter);
    } else {
      finish();
    }
  }
  finish();
  return text::join(terms, " ");
}

std::int64_t Overview::total() const {
  std::int64_t sum = 0;
  for (const Breakdown& row : breakdown) {
    sum += row.total;
  }
  return sum;
}

std::int64_t Overview::by_status(Status status) const {
  std::int64_t sum = 0;
  for (const Breakdown& row : breakdown) {
    sum += row.status == status ? row.total : 0;
  }
  return sum;
}

std::int64_t Overview::by_severity(Severity severity) const {
  std::int64_t sum = 0;
  for (const Breakdown& row : breakdown) {
    sum += row.severity == severity && row.status != Status::Done ? row.total : 0;
  }
  return sum;
}

const std::array<DemoRecord, 6>& demo_records() {
  return demo_set;
}

ImportTally apply_import(const Store& database, Timestamp now, Zone zone, const std::vector<Input>& inputs) {
  ImportTally tally;
  for (const Input& input : inputs) {
    const std::optional<Record> before = find_by_name(database, input.name);
    if (!before) {
      create_row(database, now, zone, input, false);
      ++tally.created;
      continue;
    }
    if (before->input() == input) {
      ++tally.skipped;
      continue;
    }
    Statement update = database.prepare(sql_update);
    bind_update(update, input, now, before->id, before->revision);
    run_named(update, input.name);
    const Record after = require(database, before->id);
    announce(database, now, zone, &*before, after);
    ++tally.updated;
  }
  return tally;
}

Records::Records(const Store& database, clock::Clock clock, Zone zone)
    : database_(&database), clock_(std::move(clock)), zone_(zone) {}

const Store& Records::database() const {
  return *database_;
}

Timestamp Records::now() const {
  return clock_.now();
}

Zone Records::zone() const {
  return zone_;
}

Record Records::create(const Input& input) const {
  const Input normalized = normalize(input);
  store::Transaction transaction(*database_);
  Record created = create_row(*database_, clock_.now(), zone_, normalized, false);
  transaction.commit();
  return created;
}

std::vector<Record> Records::create_all(const std::vector<Input>& inputs) const {
  store::Transaction transaction(*database_);
  const Timestamp now = clock_.now();
  std::vector<Record> created;
  created.reserve(inputs.size());
  for (const Input& input : inputs) {
    created.push_back(create_row(*database_, now, zone_, normalize(input), false));
  }
  transaction.commit();
  return created;
}

Record Records::get(std::int64_t id) const {
  return require(*database_, id);
}

std::optional<Record> Records::by_name(std::string_view wanted) const {
  return find_by_name(*database_, wanted);
}

Record Records::update(std::int64_t id, std::int64_t revision, const Input& input) const {
  const Input normalized = normalize(input);
  store::Transaction transaction(*database_);
  const std::optional<Record> before = find(*database_, id);
  if (!before) {
    throw AppError::conflict(text::concat("record ", id, " was removed by another window; reload the list"));
  }
  const auto changed_elsewhere = [&before, id] {
    return AppError::conflict(text::concat("record ", id, " was changed by another window (now revision ",
                                           before->revision, "); reload it and apply your change again"));
  };
  if (before->revision != revision) {
    throw changed_elsewhere();
  }
  const Timestamp now = clock_.now();
  Statement update = database_->prepare(sql_update);
  bind_update(update, normalized, now, id, revision);
  run_named(update, normalized.name);
  if (database_->changes() != 1) {
    throw changed_elsewhere();
  }
  Record updated = require(*database_, id);
  announce(*database_, now, zone_, &*before, updated);
  transaction.commit();
  return updated;
}

Record Records::set_enabled(std::int64_t id, bool enabled) const {
  store::Transaction transaction(*database_);
  const Record before = require(*database_, id);
  const Timestamp now = clock_.now();
  database_->prepare(sql_set_enabled).bind(enabled ? 1 : 0).bind(now.to_rfc3339()).bind(id).run();
  Record updated = require(*database_, id);
  announce(*database_, now, zone_, &before, updated);
  transaction.commit();
  return updated;
}

void Records::remove(std::int64_t id) const {
  database_->prepare(sql_delete).bind(id).run();
  if (database_->changes() == 0) {
    throw not_found(id);
  }
}

std::vector<Record> Records::list(const Query& query) const {
  const std::string_view sort = query.sort.empty() ? std::string_view("name") : std::string_view(query.sort);
  if (std::ranges::find(sort_keys, sort) == sort_keys.end()) {
    throw AppError::invalid("sort", text::concat("unknown sort column \"", query.sort, "\""),
                            "Use name, status, severity or updated");
  }
  const std::int64_t limit = query.limit <= 0 || query.limit > 10000 ? 500 : query.limit;
  const std::string search = search_expression(query.search);
  if (!text::trim_whitespace(query.search).empty() && search.empty()) {
    return {};
  }
  Statement statement = database_->prepare(sql_list);
  statement.bind(query.status ? name(*query.status) : std::string_view())
      .bind(query.severity ? theme::name(*query.severity) : std::string_view())
      .bind(query.include_disabled ? 1 : 0)
      .bind(search)
      .bind(sort)
      .bind(query.descending ? 1 : 0)
      .bind(limit)
      .bind(std::max<std::int64_t>(query.offset, 0));
  std::vector<Record> list;
  while (statement.step()) {
    list.push_back(build(statement));
  }
  return list;
}

std::int64_t Records::count() const {
  return single_integer(*database_, sql_count);
}

Overview Records::overview() const {
  Overview overview;
  Statement totals = database_->prepare(sql_overview);
  if (totals.step()) {
    overview.open = totals.integer(0);
    overview.urgent = totals.integer(1);
    overview.disabled = totals.integer(2);
    overview.demo = totals.integer(3);
    overview.unread_notifications = totals.integer(4);
  }
  Statement rows = database_->prepare(sql_overview_rows);
  while (rows.step()) {
    const std::optional<Status> status = find_status(rows.text(0));
    const std::optional<Severity> severity = theme::parse_severity(rows.text(1));
    if (status && severity) {
      overview.breakdown.push_back({*status, *severity, rows.integer(2), rows.integer(3)});
    }
  }
  return overview;
}

std::size_t Records::load_demo() const {
  store::Transaction transaction(*database_);
  const Timestamp now = clock_.now();
  std::size_t added = 0;
  for (const DemoRecord& demo : demo_set) {
    if (find_by_name(*database_, demo.name)) {
      continue;
    }
    create_row(*database_, now, zone_,
               Input{std::string(demo.name), demo.status, demo.severity, std::string(demo.note), demo.enabled}, true);
    ++added;
  }
  transaction.commit();
  return added;
}

std::int64_t Records::demo_count() const {
  return single_integer(*database_, sql_count_demo);
}

std::int64_t Records::remove_demo(bool dry_run) const {
  if (dry_run) {
    return demo_count();
  }
  database_->prepare(sql_delete_demo).run();
  return database_->changes();
}

}
