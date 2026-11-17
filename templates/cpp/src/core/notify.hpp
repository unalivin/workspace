#pragma once

#include <array>
#include <cstdint>
#include <optional>
#include <string>
#include <string_view>
#include <utility>
#include <vector>

#include "core/clock.hpp"
#include "core/store.hpp"
#include "core/theme.hpp"

namespace app::notify {

inline constexpr std::string_view event_record_urgent = "record.urgent";
inline constexpr std::string_view event_record_completed = "record.completed";
inline constexpr std::size_t max_title_length = 120;

struct HotQuery {
  std::string_view label;
  std::string_view sql;
  int parameters = 0;
};

[[nodiscard]] std::vector<HotQuery> hot_queries();

struct EventType {
  std::string_view id;
  std::string_view label;
  std::string_view description;
};

[[nodiscard]] const std::array<EventType, 2>& event_types();
[[nodiscard]] const EventType* event_type(std::string_view id);
[[nodiscard]] std::optional<std::int64_t> parse_clock(std::string_view text);

struct Rule {
  std::string event_type;
  bool enabled = true;
  theme::Severity min_severity = theme::Severity::Info;
  std::string quiet_start;
  std::string quiet_end;
  std::int64_t dedup_minutes = 60;
  bool customized = false;

  [[nodiscard]] static Rule default_for(std::string_view event_type);
  void validate() const;
  [[nodiscard]] bool in_quiet_hours(std::int64_t minute_of_day) const;
  bool operator==(const Rule&) const = default;
};

enum class Outcome { Delivered, SuppressedDisabled, SuppressedSeverity, SuppressedQuietHours, SuppressedDuplicate };

[[nodiscard]] std::string_view label(Outcome outcome);

struct Event {
  std::string event_type;
  theme::Severity severity = theme::Severity::Info;
  std::string title;
  std::string record_kind;
  std::int64_t record_id = 0;
  std::string dedup_key;
  bool operator==(const Event&) const = default;
};

[[nodiscard]] Outcome evaluate(const Rule& rule, theme::Severity severity, clock::Timestamp now,
                               std::int64_t minute_of_day, std::optional<clock::Timestamp> last_similar);
[[nodiscard]] std::string safe_title(std::string_view title);

struct Notification {
  std::int64_t id = 0;
  std::string event_type;
  theme::Severity severity = theme::Severity::Info;
  std::string title;
  std::string record_kind;
  std::int64_t record_id = 0;
  clock::Timestamp created_at;
  std::optional<clock::Timestamp> read_at;

  [[nodiscard]] bool unread() const;
  bool operator==(const Notification&) const = default;
};

struct Filter {
  bool unread_only = false;
  std::optional<theme::Severity> severity;
  std::string event_type;
  std::int64_t limit = 0;
  bool operator==(const Filter&) const = default;
};

struct Published {
  Outcome outcome = Outcome::Delivered;
  std::int64_t id = 0;
  bool operator==(const Published&) const = default;
};

Published publish_with(const store::Store& database, clock::Timestamp now, clock::Zone zone, const Event& event);

class Notifications {
 public:
  Notifications(const store::Store& database, clock::Clock clock, clock::Zone zone);

  [[nodiscard]] Rule rule(std::string_view event) const;
  [[nodiscard]] std::vector<Rule> rules() const;
  void set_rule(const Rule& rule) const;
  void reset_rule(std::string_view event) const;
  Published publish(const Event& event) const;
  [[nodiscard]] std::vector<Notification> list(const Filter& filter) const;
  [[nodiscard]] std::vector<Notification> after(std::int64_t after_id, std::int64_t limit) const;
  [[nodiscard]] std::int64_t highest_id() const;
  [[nodiscard]] std::int64_t unread_count() const;
  void mark_read(std::int64_t id) const;
  std::int64_t mark_all_read() const;

 private:
  const store::Store* database_;
  clock::Clock clock_;
  clock::Zone zone_;
};

struct Summary {
  std::string title;
  std::string body;
  bool operator==(const Summary&) const = default;
};

[[nodiscard]] Summary summary(const std::vector<Notification>& items, bool show_titles);

}
