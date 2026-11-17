#pragma once

#include <atomic>
#include <cstdint>
#include <functional>
#include <memory>
#include <optional>
#include <string>
#include <string_view>
#include <utility>

namespace app::clock {

struct Civil {
  std::int64_t year = 0;
  std::int64_t month = 0;
  std::int64_t day = 0;
  std::int64_t hour = 0;
  std::int64_t minute = 0;
  std::int64_t second = 0;

  bool operator==(const Civil&) const = default;
};

class Timestamp {
 public:
  constexpr Timestamp() = default;

  [[nodiscard]] static Timestamp from_unix(std::int64_t seconds);
  [[nodiscard]] static std::optional<Timestamp> from_civil(const Civil& civil);
  [[nodiscard]] static Timestamp now();
  [[nodiscard]] static std::optional<Timestamp> parse(std::string_view text);

  [[nodiscard]] std::int64_t unix_seconds() const;
  [[nodiscard]] Timestamp plus_seconds(std::int64_t seconds) const;
  [[nodiscard]] Civil civil(std::int64_t offset_minutes) const;
  [[nodiscard]] std::string to_rfc3339() const;
  [[nodiscard]] std::string display(std::int64_t offset_minutes) const;
  [[nodiscard]] std::int64_t minute_of_day(std::int64_t offset_minutes) const;

  auto operator<=>(const Timestamp&) const = default;

 private:
  std::int64_t seconds_ = 0;
};

class Zone {
 public:
  [[nodiscard]] static Zone utc();
  [[nodiscard]] static Zone local();
  [[nodiscard]] static Zone fixed(std::int64_t offset_minutes);

  [[nodiscard]] std::int64_t offset_minutes(Timestamp moment) const;
  [[nodiscard]] std::string display(Timestamp moment) const;
  [[nodiscard]] std::int64_t minute_of_day(Timestamp moment) const;
  [[nodiscard]] bool is_utc() const;

  bool operator==(const Zone&) const = default;

 private:
  enum class Kind { Utc, Local, Fixed };
  Kind kind_ = Kind::Utc;
  std::int64_t minutes_ = 0;
};

class ManualClock {
 public:
  explicit ManualClock(std::shared_ptr<std::atomic<std::int64_t>> seconds);
  void advance(std::int64_t seconds) const;
  void set(Timestamp moment) const;

 private:
  std::shared_ptr<std::atomic<std::int64_t>> seconds_;
};

class Clock {
 public:
  [[nodiscard]] static Clock system();
  [[nodiscard]] static Clock fixed(Timestamp moment);
  [[nodiscard]] static std::pair<Clock, ManualClock> manual(Timestamp start);

  [[nodiscard]] Timestamp now() const;

 private:
  explicit Clock(std::function<Timestamp()> source);
  std::function<Timestamp()> source_;
};

}
