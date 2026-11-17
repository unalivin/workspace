#pragma once

#include <array>
#include <cstdint>
#include <optional>
#include <string>
#include <string_view>
#include <vector>

#include "core/clock.hpp"
#include "core/notify.hpp"
#include "core/store.hpp"
#include "core/theme.hpp"

namespace app::records {

inline constexpr std::size_t max_name_length = 200;
inline constexpr std::size_t max_note_length = 4000;
inline constexpr std::string_view kind = "record";
inline constexpr std::array<std::string_view, 4> sort_keys = {"name", "status", "severity", "updated"};

[[nodiscard]] std::vector<notify::HotQuery> hot_queries();

enum class Status { Open, InProgress, Done };

inline constexpr std::array<Status, 3> all_statuses = {Status::Open, Status::InProgress, Status::Done};

[[nodiscard]] std::optional<Status> find_status(std::string_view text);
[[nodiscard]] std::string_view name(Status status);
[[nodiscard]] std::string_view label(Status status);
[[nodiscard]] std::string_view glyph(Status status);
[[nodiscard]] std::string_view column_label(std::string_view column);

struct Input {
  std::string name;
  Status status = Status::Open;
  theme::Severity severity = theme::Severity::Info;
  std::string note;
  bool enabled = true;
  bool operator==(const Input&) const = default;
};

struct Record {
  std::int64_t id = 0;
  std::string name;
  Status status = Status::Open;
  theme::Severity severity = theme::Severity::Info;
  std::string note;
  bool enabled = true;
  bool demo = false;
  std::int64_t revision = 0;
  clock::Timestamp created_at;
  clock::Timestamp updated_at;

  [[nodiscard]] Input input() const;
  [[nodiscard]] std::string_view state_label() const;
  bool operator==(const Record&) const = default;
};

[[nodiscard]] Status parse_status(std::string_view text);
[[nodiscard]] theme::Severity parse_severity(std::string_view text);
[[nodiscard]] Input normalize(const Input& input);
[[nodiscard]] std::string search_expression(std::string_view text);

struct Query {
  std::string search;
  std::optional<Status> status;
  std::optional<theme::Severity> severity;
  bool include_disabled = false;
  std::string sort;
  bool descending = false;
  std::int64_t limit = 0;
  std::int64_t offset = 0;
  bool operator==(const Query&) const = default;
};

struct Breakdown {
  Status status = Status::Open;
  theme::Severity severity = theme::Severity::Info;
  std::int64_t total = 0;
  std::int64_t demo = 0;
  bool operator==(const Breakdown&) const = default;
};

struct Overview {
  std::int64_t open = 0;
  std::int64_t urgent = 0;
  std::int64_t disabled = 0;
  std::int64_t demo = 0;
  std::int64_t unread_notifications = 0;
  std::vector<Breakdown> breakdown;

  [[nodiscard]] std::int64_t total() const;
  [[nodiscard]] std::int64_t by_status(Status status) const;
  [[nodiscard]] std::int64_t by_severity(theme::Severity severity) const;
  bool operator==(const Overview&) const = default;
};

struct DemoRecord {
  std::string_view name;
  Status status = Status::Open;
  theme::Severity severity = theme::Severity::Info;
  std::string_view note;
  bool enabled = true;
};

[[nodiscard]] const std::array<DemoRecord, 6>& demo_records();

struct ImportTally {
  std::size_t created = 0;
  std::size_t updated = 0;
  std::size_t skipped = 0;
  bool operator==(const ImportTally&) const = default;
};

ImportTally apply_import(const store::Store& database, clock::Timestamp now, clock::Zone zone,
                         const std::vector<Input>& inputs);

class Records {
 public:
  Records(const store::Store& database, clock::Clock clock, clock::Zone zone);

  [[nodiscard]] const store::Store& database() const;
  [[nodiscard]] clock::Timestamp now() const;
  [[nodiscard]] clock::Zone zone() const;

  Record create(const Input& input) const;
  std::vector<Record> create_all(const std::vector<Input>& inputs) const;
  [[nodiscard]] Record get(std::int64_t id) const;
  [[nodiscard]] std::optional<Record> by_name(std::string_view wanted) const;
  Record update(std::int64_t id, std::int64_t revision, const Input& input) const;
  Record set_enabled(std::int64_t id, bool enabled) const;
  void remove(std::int64_t id) const;
  [[nodiscard]] std::vector<Record> list(const Query& query) const;
  [[nodiscard]] std::int64_t count() const;
  [[nodiscard]] Overview overview() const;
  std::size_t load_demo() const;
  [[nodiscard]] std::int64_t demo_count() const;
  std::int64_t remove_demo(bool dry_run) const;

 private:
  const store::Store* database_;
  clock::Clock clock_;
  clock::Zone zone_;
};

}
