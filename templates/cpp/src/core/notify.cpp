#include "core/notify.hpp"

#include "core/error.hpp"
#include "core/meta.hpp"
#include "core/text.hpp"

namespace app::notify {

using clock::Timestamp;
using theme::Severity;

namespace {

constexpr std::string_view sql_select_rule =
    "SELECT enabled, min_severity, quiet_start, quiet_end, dedup_minutes FROM notification_rules WHERE event_type = ?1";
constexpr std::string_view sql_upsert_rule =
    "INSERT INTO notification_rules (event_type, enabled, min_severity, quiet_start, quiet_end, dedup_minutes, "
    "updated_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7) ON CONFLICT (event_type) DO UPDATE SET enabled = "
    "excluded.enabled, min_severity = excluded.min_severity, quiet_start = excluded.quiet_start, quiet_end = "
    "excluded.quiet_end, dedup_minutes = excluded.dedup_minutes, updated_at = excluded.updated_at";
constexpr std::string_view sql_delete_rule = "DELETE FROM notification_rules WHERE event_type = ?1";
constexpr std::string_view sql_last_similar =
    "SELECT created_at FROM notifications WHERE event_type = ?1 AND dedup_key = ?2 ORDER BY created_at DESC LIMIT 1";
constexpr std::string_view sql_insert =
    "INSERT INTO notifications (event_type, severity, title, record_kind, record_id, dedup_key, created_at) VALUES "
    "(?1, ?2, ?3, ?4, ?5, ?6, ?7)";
constexpr std::string_view sql_list =
    "SELECT id, event_type, severity, title, record_kind, record_id, created_at, read_at FROM notifications WHERE "
    "(?1 = 0 OR read_at IS NULL) AND (?2 = '' OR severity = ?2) AND (?3 = '' OR event_type = ?3) ORDER BY created_at "
    "DESC, id DESC LIMIT ?4";
constexpr std::string_view sql_unread_count = "SELECT unread_notifications FROM overview_attention";
constexpr std::string_view sql_mark_read = "UPDATE notifications SET read_at = ?1 WHERE id = ?2 AND read_at IS NULL";
constexpr std::string_view sql_mark_all_read = "UPDATE notifications SET read_at = ?1 WHERE read_at IS NULL";
constexpr std::string_view sql_after =
    "SELECT id, event_type, severity, title, record_kind, record_id, created_at, read_at FROM notifications WHERE id "
    "> ?1 ORDER BY id LIMIT ?2";
constexpr std::string_view sql_highest_id = "SELECT coalesce(max(id), 0) FROM notifications";
constexpr std::string_view sql_notification_id = "SELECT id FROM notifications WHERE id = ?1";

constexpr std::array<EventType, 2> known_events = {{
    {event_record_urgent, "Record needs attention", "An open record has high or critical severity."},
    {event_record_completed, "Record completed", "A record was marked as done."},
}};

AppError unknown_event(std::string_view id) {
  return AppError::invalid("event", text::concat("unknown event type \"", id, "\""),
                           "List the notification rules to see the event types");
}

bool is_digit(char letter) {
  return letter >= '0' && letter <= '9';
}

bool secret_like(char32_t letter) {
  return (letter < 0x80 && text::is_word(letter)) || letter == U'+' || letter == U'/' || letter == U'_' ||
         letter == U'=' || letter == U'-';
}

Rule load_rule(const store::Store& database, std::string_view event) {
  Rule rule = Rule::default_for(event);
  store::Statement statement = database.prepare(sql_select_rule);
  statement.bind(event);
  if (statement.step()) {
    rule.enabled = statement.integer(0) == 1;
    rule.min_severity = theme::parse_severity(statement.text(1)).value_or(Severity::Info);
    rule.quiet_start = statement.text(2);
    rule.quiet_end = statement.text(3);
    rule.dedup_minutes = statement.integer(4);
    rule.customized = true;
  }
  return rule;
}

std::vector<Notification> collect(store::Statement& statement) {
  std::vector<Notification> list;
  while (statement.step()) {
    Notification item;
    item.id = statement.integer(0);
    item.event_type = statement.text(1);
    item.severity = theme::parse_severity(statement.text(2)).value_or(Severity::Info);
    item.title = statement.text(3);
    item.record_kind = statement.text(4);
    item.record_id = statement.integer(5);
    const std::optional<Timestamp> created = Timestamp::parse(statement.text(6));
    if (!created) {
      throw AppError::storage(text::concat("notification ", item.id, " has an unreadable timestamp"));
    }
    item.created_at = *created;
    if (!statement.is_null(7)) {
      item.read_at = Timestamp::parse(statement.text(7));
    }
    list.push_back(std::move(item));
  }
  return list;
}

std::int64_t single_integer(const store::Store& database, std::string_view sql) {
  store::Statement statement = database.prepare(sql);
  return statement.step() ? statement.integer(0) : 0;
}

}

std::vector<HotQuery> hot_queries() {
  return {
      {"notification de-duplication lookup", sql_last_similar, 2},
      {"notifications after cursor", sql_after, 2},
      {"mark notification read", sql_mark_read, 2},
  };
}

const std::array<EventType, 2>& event_types() {
  return known_events;
}

const EventType* event_type(std::string_view id) {
  for (const EventType& event : known_events) {
    if (event.id == id) {
      return &event;
    }
  }
  return nullptr;
}

std::optional<std::int64_t> parse_clock(std::string_view text) {
  if (text.size() != 5 || text[2] != ':' || !is_digit(text[0]) || !is_digit(text[1]) || !is_digit(text[3]) ||
      !is_digit(text[4])) {
    return std::nullopt;
  }
  const std::int64_t hours = (text[0] - '0') * 10 + (text[1] - '0');
  const std::int64_t minutes = (text[3] - '0') * 10 + (text[4] - '0');
  if (hours >= 24 || minutes >= 60) {
    return std::nullopt;
  }
  return hours * 60 + minutes;
}

Rule Rule::default_for(std::string_view event_type) {
  Rule rule;
  rule.event_type = std::string(event_type);
  return rule;
}

void Rule::validate() const {
  if (notify::event_type(event_type) == nullptr) {
    throw unknown_event(event_type);
  }
  if (quiet_start.empty() != quiet_end.empty()) {
    throw AppError::invalid("quiet hours", "quiet hours need both a start and an end",
                            "Give both times as HH:MM or leave both empty");
  }
  for (const std::string* time : {&quiet_start, &quiet_end}) {
    if (!time->empty() && !parse_clock(*time)) {
      throw AppError::invalid("quiet hours", text::concat("\"", *time, "\" is not a time of day"),
                              "Use 24-hour HH:MM, for example 22:30");
    }
  }
  if (dedup_minutes < 0 || dedup_minutes > 10080) {
    throw AppError::invalid("de-duplication", text::concat("de-duplication window ", dedup_minutes, " is out of range"),
                            "Use 0 to 10080 minutes (0 turns it off)");
  }
}

bool Rule::in_quiet_hours(std::int64_t minute_of_day) const {
  const std::optional<std::int64_t> start = parse_clock(quiet_start);
  const std::optional<std::int64_t> end = parse_clock(quiet_end);
  if (!start || !end || *start == *end) {
    return false;
  }
  if (*start < *end) {
    return minute_of_day >= *start && minute_of_day < *end;
  }
  return minute_of_day >= *start || minute_of_day < *end;
}

std::string_view label(Outcome outcome) {
  switch (outcome) {
    case Outcome::Delivered:
      return "delivered";
    case Outcome::SuppressedDisabled:
      return "suppressed: rule disabled";
    case Outcome::SuppressedSeverity:
      return "suppressed: below severity threshold";
    case Outcome::SuppressedQuietHours:
      return "suppressed: quiet hours";
    case Outcome::SuppressedDuplicate:
      return "suppressed: duplicate";
  }
  return "delivered";
}

Outcome evaluate(const Rule& rule, Severity severity, Timestamp now, std::int64_t minute_of_day,
                 std::optional<Timestamp> last_similar) {
  if (!rule.enabled) {
    return Outcome::SuppressedDisabled;
  }
  if (severity < rule.min_severity) {
    return Outcome::SuppressedSeverity;
  }
  if (rule.in_quiet_hours(minute_of_day)) {
    return Outcome::SuppressedQuietHours;
  }
  if (last_similar) {
    const std::int64_t elapsed = now.unix_seconds() - last_similar->unix_seconds();
    if (rule.dedup_minutes > 0 && elapsed >= 0 && elapsed < rule.dedup_minutes * 60) {
      return Outcome::SuppressedDuplicate;
    }
  }
  return Outcome::Delivered;
}

std::string safe_title(std::string_view title) {
  std::u32string spaced = text::decode(title);
  for (char32_t& letter : spaced) {
    if (text::is_control(letter)) {
      letter = U' ';
    }
  }
  std::vector<std::string> cleaned_words;
  for (const std::string& word : text::words(text::encode(spaced))) {
    std::u32string letters = text::decode(word);
    letters.push_back(U' ');
    std::string cleaned;
    std::u32string run;
    for (const char32_t letter : letters) {
      if (secret_like(letter)) {
        run.push_back(letter);
        continue;
      }
      cleaned.append(run.size() >= 24 ? std::string("[redacted]") : text::encode(run));
      run.clear();
      if (letter != U' ') {
        text::append(cleaned, letter);
      }
    }
    cleaned_words.push_back(std::move(cleaned));
  }
  std::string cleaned = text::join(cleaned_words, " ");
  if (text::length(cleaned) > max_title_length) {
    cleaned = text::truncated(cleaned, max_title_length - 1) + "\xE2\x80\xA6";
  }
  return cleaned.empty() ? std::string("Notification") : cleaned;
}

bool Notification::unread() const {
  return !read_at.has_value();
}

Published publish_with(const store::Store& database, Timestamp now, clock::Zone zone, const Event& event) {
  if (event_type(event.event_type) == nullptr) {
    throw unknown_event(event.event_type);
  }
  const std::string dedup_key =
      event.dedup_key.empty() ? text::concat(event.record_kind, ":", event.record_id) : event.dedup_key;
  const Rule rule = load_rule(database, event.event_type);
  std::optional<Timestamp> last_similar;
  {
    store::Statement last = database.prepare(sql_last_similar);
    last.bind(event.event_type).bind(dedup_key);
    if (last.step()) {
      last_similar = Timestamp::parse(last.text(0));
    }
  }
  const Outcome outcome = evaluate(rule, event.severity, now, zone.minute_of_day(now), last_similar);
  if (outcome != Outcome::Delivered) {
    return {outcome, 0};
  }
  database.prepare(sql_insert)
      .bind(event.event_type)
      .bind(theme::name(event.severity))
      .bind(safe_title(event.title))
      .bind(event.record_kind)
      .bind(event.record_id)
      .bind(dedup_key)
      .bind(now.to_rfc3339())
      .run();
  return {Outcome::Delivered, database.last_insert_id()};
}

Notifications::Notifications(const store::Store& database, clock::Clock clock, clock::Zone zone)
    : database_(&database), clock_(std::move(clock)), zone_(zone) {}

Rule Notifications::rule(std::string_view event) const {
  if (event_type(event) == nullptr) {
    throw unknown_event(event);
  }
  return load_rule(*database_, event);
}

std::vector<Rule> Notifications::rules() const {
  std::vector<Rule> out;
  for (const EventType& event : known_events) {
    out.push_back(load_rule(*database_, event.id));
  }
  return out;
}

void Notifications::set_rule(const Rule& rule) const {
  rule.validate();
  database_->prepare(sql_upsert_rule)
      .bind(rule.event_type)
      .bind(rule.enabled ? 1 : 0)
      .bind(theme::name(rule.min_severity))
      .bind(rule.quiet_start)
      .bind(rule.quiet_end)
      .bind(rule.dedup_minutes)
      .bind(clock_.now().to_rfc3339())
      .run();
}

void Notifications::reset_rule(std::string_view event) const {
  if (event_type(event) == nullptr) {
    throw unknown_event(event);
  }
  database_->prepare(sql_delete_rule).bind(event).run();
}

Published Notifications::publish(const Event& event) const {
  return publish_with(*database_, clock_.now(), zone_, event);
}

std::vector<Notification> Notifications::list(const Filter& filter) const {
  const std::int64_t limit = filter.limit <= 0 || filter.limit > 1000 ? 200 : filter.limit;
  store::Statement statement = database_->prepare(sql_list);
  statement.bind(filter.unread_only ? 1 : 0)
      .bind(filter.severity ? theme::name(*filter.severity) : std::string_view())
      .bind(filter.event_type)
      .bind(limit);
  return collect(statement);
}

std::vector<Notification> Notifications::after(std::int64_t after_id, std::int64_t limit) const {
  store::Statement statement = database_->prepare(sql_after);
  statement.bind(after_id).bind(limit <= 0 || limit > 500 ? 50 : limit);
  return collect(statement);
}

std::int64_t Notifications::highest_id() const {
  return single_integer(*database_, sql_highest_id);
}

std::int64_t Notifications::unread_count() const {
  return single_integer(*database_, sql_unread_count);
}

void Notifications::mark_read(std::int64_t id) const {
  store::Statement found = database_->prepare(sql_notification_id);
  found.bind(id);
  if (!found.step()) {
    throw AppError::not_found(text::concat("notification ", id, " (list the notifications to see valid identifiers)"));
  }
  database_->prepare(sql_mark_read).bind(clock_.now().to_rfc3339()).bind(id).run();
}

std::int64_t Notifications::mark_all_read() const {
  database_->prepare(sql_mark_all_read).bind(clock_.now().to_rfc3339()).run();
  return database_->changes();
}

Summary summary(const std::vector<Notification>& items, bool show_titles) {
  Summary out{std::string(meta::app_name), {}};
  if (items.empty()) {
    return out;
  }
  if (items.size() == 1) {
    out.body = show_titles ? safe_title(items.front().title) : std::string("1 new notification");
    return out;
  }
  out.body = text::concat(items.size(), " new notifications");
  if (show_titles) {
    const Notification* top = &items.front();
    for (const Notification& item : items) {
      if (item.severity > top->severity) {
        top = &item;
      }
    }
    out.body.append(text::concat(", most severe: ", safe_title(top->title)));
  }
  return out;
}

}
