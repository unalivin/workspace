#include <doctest/doctest.h>

#include <string>
#include <utility>
#include <vector>

#include "core/error.hpp"
#include "core/meta.hpp"
#include "core/notify.hpp"
#include "core/text.hpp"
#include "support/fixture.hpp"
#include "support/random.hpp"
#include "support/testkit.hpp"

using app::AppError;
using app::clock::Clock;
using app::clock::Zone;
using app::notify::Event;
using app::notify::Filter;
using app::notify::Notification;
using app::notify::Notifications;
using app::notify::Outcome;
using app::notify::Rule;
using app::theme::Severity;
namespace notify = app::notify;
namespace text = app::text;

namespace {

Event event(Severity severity, std::int64_t record_id) {
  return {std::string(notify::event_record_urgent), severity, "A record needs attention", "record", record_id, {}};
}

Rule rule() {
  return Rule::default_for(notify::event_record_urgent);
}

Rule quiet(std::string start, std::string end) {
  Rule out = rule();
  out.quiet_start = std::move(start);
  out.quiet_end = std::move(end);
  return out;
}

std::vector<std::int64_t> ids(const std::vector<Notification>& items) {
  std::vector<std::int64_t> out;
  for (const Notification& item : items) {
    out.push_back(item.id);
  }
  return out;
}

}

TEST_CASE("evaluation checks enabled severity quiet hours then duplicates") {
  const auto now = testkit::fixed_time();
  const std::int64_t minute = Zone::utc().minute_of_day(now);
  CHECK(notify::evaluate(rule(), Severity::Info, now, minute, std::nullopt) == Outcome::Delivered);
  Rule disabled = rule();
  disabled.enabled = false;
  CHECK(notify::evaluate(disabled, Severity::Critical, now, minute, std::nullopt) == Outcome::SuppressedDisabled);
  Rule strict = rule();
  strict.min_severity = Severity::High;
  CHECK(notify::evaluate(strict, Severity::Medium, now, minute, std::nullopt) == Outcome::SuppressedSeverity);
  CHECK(notify::evaluate(strict, Severity::High, now, minute, std::nullopt) == Outcome::Delivered);
  CHECK(notify::evaluate(quiet("09:00", "10:00"), Severity::Critical, now, minute, std::nullopt) ==
        Outcome::SuppressedQuietHours);
  const auto recent = now.plus_seconds(-59 * 60);
  CHECK(notify::evaluate(rule(), Severity::Critical, now, minute, recent) == Outcome::SuppressedDuplicate);
  CHECK(notify::evaluate(rule(), Severity::Critical, now, minute, now.plus_seconds(-60 * 60)) == Outcome::Delivered);
  Rule no_window = rule();
  no_window.dedup_minutes = 0;
  CHECK(notify::evaluate(no_window, Severity::Critical, now, minute, recent) == Outcome::Delivered);
  CHECK(notify::evaluate(rule(), Severity::Critical, now, minute, now.plus_seconds(600)) == Outcome::Delivered);
  CHECK(notify::label(Outcome::SuppressedQuietHours) == "suppressed: quiet hours");
  CHECK(notify::label(Outcome::Delivered) == "delivered");
}

TEST_CASE("quiet hours may cross midnight") {
  const Rule night = quiet("22:30", "06:00");
  const std::vector<std::pair<std::string, bool>> cases = {
      {"22:29", false}, {"22:30", true},  {"23:59", true},  {"00:00", true},
      {"05:59", true},  {"06:00", false}, {"12:00", false},
  };
  for (const auto& [time, expected] : cases) {
    CHECK_MESSAGE(night.in_quiet_hours(notify::parse_clock(time).value()) == expected, time);
  }
  CHECK_FALSE(quiet("08:00", "08:00").in_quiet_hours(8 * 60));
  CHECK_FALSE(rule().in_quiet_hours(0));
}

TEST_CASE("rules are validated before they are stored") {
  std::vector<std::pair<Rule, std::string>> cases;
  Rule candidate = rule();
  candidate.event_type = "nope";
  cases.emplace_back(candidate, "unknown event type");
  cases.emplace_back(quiet("22:00", ""), "both a start and an end");
  cases.emplace_back(quiet("25:00", "06:00"), "not a time of day");
  cases.emplace_back(quiet("9:00", "06:00"), "not a time of day");
  candidate = rule();
  candidate.dedup_minutes = -1;
  cases.emplace_back(candidate, "out of range");
  candidate.dedup_minutes = 10081;
  cases.emplace_back(candidate, "out of range");
  for (const auto& [broken, expected] : cases) {
    const std::string error = testkit::failure_of([&] { broken.validate(); });
    CHECK_MESSAGE(text::contains(error, expected), error);
  }
  CHECK_NOTHROW(rule().validate());
}

TEST_CASE("publishing stores delivers and suppresses duplicates") {
  const testkit::Fixture fixture("notify-publish");
  const Notifications service = fixture.notifications();
  const notify::Published first = service.publish(event(Severity::High, 7));
  CHECK(first.outcome == Outcome::Delivered);
  CHECK(first.id > 0);
  CHECK(service.publish(event(Severity::High, 7)) == notify::Published{Outcome::SuppressedDuplicate, 0});
  CHECK(service.publish(event(Severity::High, 8)).outcome == Outcome::Delivered);
  fixture.hand.advance(61 * 60);
  CHECK(service.publish(event(Severity::High, 7)).outcome == Outcome::Delivered);
  CHECK(service.unread_count() == 3);
  CHECK(service.highest_id() == 3);
  using Ids = std::vector<std::int64_t>;
  CHECK(ids(service.list({})) == Ids{3, 2, 1});
  CHECK(ids(service.after(1, 0)) == Ids{2, 3});
  CHECK(ids(service.after(1, 1)) == Ids{2});
  service.mark_read(1);
  const std::string missing = testkit::failure_of([&] { service.mark_read(99); });
  CHECK_MESSAGE(text::contains(missing, "notification 99"), missing);
  Filter unread;
  unread.unread_only = true;
  CHECK(service.list(unread).size() == 2);
  Filter low;
  low.severity = Severity::Low;
  CHECK(service.list(low).empty());
  Filter typed;
  typed.event_type = std::string(notify::event_record_completed);
  CHECK(service.list(typed).empty());
  Filter limited;
  limited.limit = 1;
  CHECK(service.list(limited).size() == 1);
  const std::vector<Notification> listed = service.list({});
  CHECK_FALSE(listed.back().unread());
  CHECK(listed.back().read_at == fixture.clock.now());
  CHECK(service.mark_all_read() == 2);
  CHECK(service.unread_count() == 0);
  Event unknown = event(Severity::High, 1);
  unknown.event_type = "nope";
  CHECK_THROWS_AS(service.publish(unknown), AppError);
  Event keyed = event(Severity::High, 50);
  keyed.dedup_key = "shared";
  CHECK(service.publish(keyed).outcome == Outcome::Delivered);
  keyed.record_id = 51;
  CHECK(service.publish(keyed).outcome == Outcome::SuppressedDuplicate);
}

TEST_CASE("stored rules change what is delivered") {
  const testkit::Fixture fixture("notify-rules");
  const Notifications service(fixture.store, Clock::fixed(testkit::fixed_time()), Zone::fixed(60));
  CHECK_FALSE(service.rule(notify::event_record_urgent).customized);
  Rule critical = rule();
  critical.min_severity = Severity::Critical;
  service.set_rule(critical);
  const Rule stored = service.rule(notify::event_record_urgent);
  CHECK(stored.customized);
  CHECK(stored.min_severity == Severity::Critical);
  CHECK(service.publish(event(Severity::High, 1)).outcome == Outcome::SuppressedSeverity);
  service.set_rule(quiet("10:00", "11:00"));
  CHECK(service.publish(event(Severity::High, 1)).outcome == Outcome::SuppressedQuietHours);
  service.reset_rule(notify::event_record_urgent);
  CHECK(service.publish(event(Severity::High, 1)).outcome == Outcome::Delivered);
  CHECK(service.rules().size() == notify::event_types().size());
  CHECK_THROWS_AS(static_cast<void>(service.rule("nope")), AppError);
  CHECK_THROWS_AS(service.reset_rule("nope"), AppError);
  Rule negative = rule();
  negative.dedup_minutes = -5;
  CHECK_THROWS_AS(service.set_rule(negative), AppError);
  CHECK(notify::event_type(notify::event_record_completed)->label == "Record completed");
  CHECK(notify::event_type("nope") == nullptr);
}

TEST_CASE("titles are cleaned shortened and never carry secrets") {
  CHECK(notify::safe_title("  two\twords\n") == "two words");
  CHECK(notify::safe_title("") == "Notification");
  CHECK(notify::safe_title("token ghp_abcdefghijklmnopqrstuvwxyz0123 leaked") == "token [redacted] leaked");
  CHECK(notify::safe_title("key=AAAAAAAAAAAAAAAAAAAAAAAAAAAA.") == "[redacted].");
  CHECK(notify::safe_title("short-word stays") == "short-word stays");
  CHECK(notify::safe_title("caf\xC3\xA9 \xE2\x82\xAC"
                           "5") ==
        "caf\xC3\xA9 \xE2\x82\xAC"
        "5");
  std::string repeated;
  for (int index = 0; index < 60; ++index) {
    repeated.append("word ");
  }
  const std::string shortened = notify::safe_title(repeated);
  CHECK(text::length(shortened) == notify::max_title_length);
  CHECK(shortened.ends_with("\xE2\x80\xA6"));
}

TEST_CASE("the system notification shows a count and titles only on request") {
  const auto item = [](std::int64_t id, std::string title, Severity severity) {
    Notification out;
    out.id = id;
    out.event_type = std::string(notify::event_record_urgent);
    out.severity = severity;
    out.title = std::move(title);
    out.record_kind = "record";
    out.record_id = id;
    out.created_at = testkit::fixed_time();
    return out;
  };
  const std::vector<Notification> items = {item(1, "one", Severity::Low), item(2, "two", Severity::Critical),
                                           item(3, "three", Severity::High)};
  const std::vector<Notification> single = {items.front()};
  CHECK(notify::summary({}, true) == notify::Summary{std::string(app::meta::app_name), ""});
  CHECK(notify::summary(single, false).body == "1 new notification");
  CHECK(notify::summary(single, true).body == "one");
  CHECK(notify::summary(items, false).body == "3 new notifications");
  CHECK(notify::summary(items, true).body == "3 new notifications, most severe: two");
  CHECK(items.front().unread());
}

TEST_CASE("safe titles are bounded single line and free of long tokens") {
  testkit::Random random(99);
  const std::vector<std::string_view> alphabet = {
      "a", "B", "7", "+", "/", "_", "=", "-", " ", "\t", "\n", ".", "\x1B", "\xC3\xA9", "\xE2\x82\xAC", "[", "]"};
  for (int round = 0; round < 1500; ++round) {
    const std::string cleaned = notify::safe_title(round % 3 == 0 ? random.bytes(300) : random.pieces(300, alphabet));
    const std::u32string letters = text::decode(cleaned);
    CHECK(letters.size() <= notify::max_title_length);
    CHECK_FALSE(cleaned.empty());
    CHECK(text::is_valid_utf8(cleaned));
    int run = 0;
    for (const char32_t letter : letters) {
      CHECK_FALSE(text::is_control(letter));
      const bool secret = (letter < 0x80 && text::is_word(letter)) || letter == U'+' || letter == U'/' ||
                          letter == U'_' || letter == U'=' || letter == U'-';
      run = secret ? run + 1 : 0;
      CHECK(run < 24);
    }
  }
}

TEST_CASE("clock parsing accepts exactly valid times") {
  testkit::Random random(5);
  int accepted = 0;
  for (int round = 0; round < 20000; ++round) {
    const std::string candidate = random.text(6, "0123456789:");
    if (const auto minutes = notify::parse_clock(candidate)) {
      ++accepted;
      CHECK(*minutes >= 0);
      CHECK(*minutes < 1440);
      CHECK(text::concat(text::padded(*minutes / 60, 2), ":", text::padded(*minutes % 60, 2)) == candidate);
    }
  }
  CHECK(accepted > 0);
  CHECK(notify::parse_clock("23:59") == 23 * 60 + 59);
  for (const char* bad : {"24:00", "12:60", "1200", "12-00", "ab:cd", ""}) {
    CHECK_FALSE(notify::parse_clock(bad).has_value());
  }
}
