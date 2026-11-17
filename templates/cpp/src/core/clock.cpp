#include "core/clock.hpp"

#include <array>
#include <chrono>
#include <ctime>
#include <utility>

#include "core/text.hpp"

namespace app::clock {

namespace {

constexpr std::int64_t seconds_per_day = 86'400;

std::int64_t floor_div(std::int64_t value, std::int64_t divisor) {
  const std::int64_t quotient = value / divisor;
  return (value % divisor != 0 && (value < 0) != (divisor < 0)) ? quotient - 1 : quotient;
}

std::int64_t floor_mod(std::int64_t value, std::int64_t divisor) {
  return value - floor_div(value, divisor) * divisor;
}

std::int64_t days_from_civil(std::int64_t year, std::int64_t month, std::int64_t day) {
  const std::int64_t shifted_year = month <= 2 ? year - 1 : year;
  const std::int64_t era = floor_div(shifted_year, 400);
  const std::int64_t year_of_era = floor_mod(shifted_year, 400);
  const std::int64_t shifted_month = (month + 9) % 12;
  const std::int64_t day_of_year = (153 * shifted_month + 2) / 5 + day - 1;
  const std::int64_t day_of_era = year_of_era * 365 + year_of_era / 4 - year_of_era / 100 + day_of_year;
  return era * 146'097 + day_of_era - 719'468;
}

Civil civil_from_days(std::int64_t days) {
  const std::int64_t shifted = days + 719'468;
  const std::int64_t era = floor_div(shifted, 146'097);
  const std::int64_t day_of_era = floor_mod(shifted, 146'097);
  const std::int64_t year_of_era = (day_of_era - day_of_era / 1'460 + day_of_era / 36'524 - day_of_era / 146'096) / 365;
  const std::int64_t day_of_year = day_of_era - (365 * year_of_era + year_of_era / 4 - year_of_era / 100);
  const std::int64_t shifted_month = (5 * day_of_year + 2) / 153;
  const std::int64_t day = day_of_year - (153 * shifted_month + 2) / 5 + 1;
  const std::int64_t month = shifted_month < 10 ? shifted_month + 3 : shifted_month - 9;
  const std::int64_t year = year_of_era + era * 400 + (month <= 2 ? 1 : 0);
  return {year, month, day, 0, 0, 0};
}

std::int64_t days_in_month(std::int64_t year, std::int64_t month) {
  static constexpr std::array<std::int64_t, 12> lengths = {31, 28, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31};
  const bool leap = (year % 4 == 0 && year % 100 != 0) || year % 400 == 0;
  if (month == 2 && leap) {
    return 29;
  }
  return lengths.at(static_cast<std::size_t>(month - 1));
}

bool within(std::int64_t value, std::int64_t least, std::int64_t most) {
  return value >= least && value <= most;
}

std::optional<std::int64_t> digits(std::string_view part) {
  if (!text::is_digits(part)) {
    return std::nullopt;
  }
  return text::parse_integer(part);
}

std::int64_t local_offset_minutes(std::int64_t unix_seconds) {
  const auto moment = static_cast<std::time_t>(unix_seconds);
  std::tm local{};
#if defined(_WIN32)
  if (localtime_s(&local, &moment) != 0) {
    return 0;
  }
#else
  if (localtime_r(&moment, &local) == nullptr) {
    return 0;
  }
#endif
  const std::int64_t days = days_from_civil(local.tm_year + 1900, local.tm_mon + 1, local.tm_mday);
  const std::int64_t as_utc = days * seconds_per_day + local.tm_hour * 3600 + local.tm_min * 60 + local.tm_sec;
  return floor_div(as_utc - unix_seconds, 60);
}

}

Timestamp Timestamp::from_unix(std::int64_t seconds) {
  Timestamp moment;
  moment.seconds_ = seconds;
  return moment;
}

std::optional<Timestamp> Timestamp::from_civil(const Civil& civil) {
  const bool valid = within(civil.year, 1, 9999) && within(civil.month, 1, 12) &&
                     within(civil.day, 1, within(civil.month, 1, 12) ? days_in_month(civil.year, civil.month) : 0) &&
                     within(civil.hour, 0, 23) && within(civil.minute, 0, 59) && within(civil.second, 0, 59);
  if (!valid) {
    return std::nullopt;
  }
  return from_unix(days_from_civil(civil.year, civil.month, civil.day) * seconds_per_day + civil.hour * 3600 +
                   civil.minute * 60 + civil.second);
}

Timestamp Timestamp::now() {
  const auto elapsed = std::chrono::system_clock::now().time_since_epoch();
  return from_unix(std::chrono::duration_cast<std::chrono::seconds>(elapsed).count());
}

std::int64_t Timestamp::unix_seconds() const {
  return seconds_;
}

Timestamp Timestamp::plus_seconds(std::int64_t seconds) const {
  return from_unix(seconds_ + seconds);
}

Civil Timestamp::civil(std::int64_t offset_minutes) const {
  const std::int64_t local = seconds_ + offset_minutes * 60;
  Civil civil = civil_from_days(floor_div(local, seconds_per_day));
  const std::int64_t rest = floor_mod(local, seconds_per_day);
  civil.hour = rest / 3600;
  civil.minute = rest % 3600 / 60;
  civil.second = rest % 60;
  return civil;
}

std::string Timestamp::to_rfc3339() const {
  const Civil c = civil(0);
  return text::concat(text::padded(c.year, 4), "-", text::padded(c.month, 2), "-", text::padded(c.day, 2), "T",
                      text::padded(c.hour, 2), ":", text::padded(c.minute, 2), ":", text::padded(c.second, 2), "Z");
}

std::optional<Timestamp> Timestamp::parse(std::string_view text) {
  if (text.size() != 20) {
    return std::nullopt;
  }
  static constexpr std::array<std::pair<std::size_t, char>, 6> separators = {
      {{4, '-'}, {7, '-'}, {10, 'T'}, {13, ':'}, {16, ':'}, {19, 'Z'}}};
  for (const auto& [index, expected] : separators) {
    if (text[index] != expected) {
      return std::nullopt;
    }
  }
  const std::array<std::optional<std::int64_t>, 6> parts = {digits(text.substr(0, 4)),  digits(text.substr(5, 2)),
                                                            digits(text.substr(8, 2)),  digits(text.substr(11, 2)),
                                                            digits(text.substr(14, 2)), digits(text.substr(17, 2))};
  for (const auto& part : parts) {
    if (!part) {
      return std::nullopt;
    }
  }
  return from_civil({*parts[0], *parts[1], *parts[2], *parts[3], *parts[4], *parts[5]});
}

std::string Timestamp::display(std::int64_t offset_minutes) const {
  const Civil c = civil(offset_minutes);
  return text::concat(text::padded(c.year, 4), "-", text::padded(c.month, 2), "-", text::padded(c.day, 2), " ",
                      text::padded(c.hour, 2), ":", text::padded(c.minute, 2));
}

std::int64_t Timestamp::minute_of_day(std::int64_t offset_minutes) const {
  const Civil c = civil(offset_minutes);
  return c.hour * 60 + c.minute;
}

Zone Zone::utc() {
  return {};
}

Zone Zone::local() {
  Zone zone;
  zone.kind_ = Kind::Local;
  return zone;
}

Zone Zone::fixed(std::int64_t offset_minutes) {
  Zone zone;
  zone.kind_ = Kind::Fixed;
  zone.minutes_ = offset_minutes;
  return zone;
}

std::int64_t Zone::offset_minutes(Timestamp moment) const {
  switch (kind_) {
    case Kind::Utc:
      return 0;
    case Kind::Fixed:
      return minutes_;
    case Kind::Local:
      return local_offset_minutes(moment.unix_seconds());
  }
  return 0;
}

std::string Zone::display(Timestamp moment) const {
  std::string shown = moment.display(offset_minutes(moment));
  if (is_utc()) {
    shown.append(" UTC");
  }
  return shown;
}

std::int64_t Zone::minute_of_day(Timestamp moment) const {
  return moment.minute_of_day(offset_minutes(moment));
}

bool Zone::is_utc() const {
  return kind_ == Kind::Utc;
}

ManualClock::ManualClock(std::shared_ptr<std::atomic<std::int64_t>> seconds) : seconds_(std::move(seconds)) {}

void ManualClock::advance(std::int64_t seconds) const {
  seconds_->fetch_add(seconds);
}

void ManualClock::set(Timestamp moment) const {
  seconds_->store(moment.unix_seconds());
}

Clock::Clock(std::function<Timestamp()> source) : source_(std::move(source)) {}

Clock Clock::system() {
  return Clock([] { return Timestamp::now(); });
}

Clock Clock::fixed(Timestamp moment) {
  return Clock([moment] { return moment; });
}

std::pair<Clock, ManualClock> Clock::manual(Timestamp start) {
  auto seconds = std::make_shared<std::atomic<std::int64_t>>(start.unix_seconds());
  Clock clock([seconds] { return Timestamp::from_unix(seconds->load()); });
  return {std::move(clock), ManualClock(seconds)};
}

Timestamp Clock::now() const {
  return source_();
}

}
