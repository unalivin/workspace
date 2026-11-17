#include <doctest/doctest.h>

#include <string>
#include <vector>

#include "core/build_config.hpp"
#include "core/clock.hpp"
#include "core/error.hpp"
#include "core/meta.hpp"
#include "core/text.hpp"
#include "support/random.hpp"
#include "support/testkit.hpp"

using app::AppError;
using app::clock::Clock;
using app::clock::Timestamp;
using app::clock::Zone;
namespace meta = app::meta;
namespace text = app::text;

TEST_CASE("validation errors name the field the problem and the fix") {
  const AppError error = AppError::invalid("name", "the name is empty", "Enter a name of 1 to 200 characters");
  CHECK(error.message() == "name: the name is empty. Enter a name of 1 to 200 characters");
  CHECK(std::string(error.what()) == error.message());
  CHECK(error.is_usage());
  CHECK(error.field() == "name");
  CHECK_FALSE(AppError::storage("x").is_usage());
  CHECK(AppError::not_found("record 7").message() == "record 7 was not found");
  CHECK(AppError::busy().is_busy());
  CHECK_FALSE(AppError::storage("database is slow").is_busy());
}

TEST_CASE("text helpers trim split join and count characters") {
  CHECK(text::trim("  a b \n") == "a b");
  CHECK(text::trim(" \t ").empty());
  CHECK(text::lower("MiXed-1") == "mixed-1");
  CHECK(text::upper("MiXed-1") == "MIXED-1");
  CHECK(text::equals_ignoring_case("WAL", "wal"));
  CHECK_FALSE(text::equals_ignoring_case("wal", "wall"));
  CHECK(text::split("a,,b", ',') == std::vector<std::string>{"a", "", "b"});
  CHECK(text::split("", ',') == std::vector<std::string>{""});
  CHECK(text::lines("one\r\ntwo\n") == std::vector<std::string>{"one", "two"});
  CHECK(text::lines("").empty());
  CHECK(text::join({"a", "b", "c"}, ", ") == "a, b, c");
  CHECK(text::replace_all("a.b.c", ".", "::") == "a::b::c");
  CHECK(text::replace_all("abc", "", "x") == "abc");
  CHECK(text::padded(7, 3) == "007");
  CHECK(text::padded(-7, 3) == "-007");
  CHECK(text::padded(12345, 2) == "12345");
  CHECK(text::parse_integer("42") == 42);
  CHECK(text::parse_integer("-42") == -42);
  for (const char* bad : {"", "+1", "4 2", "1x", "99999999999999999999"}) {
    CHECK_FALSE(text::parse_integer(bad).has_value());
  }
  CHECK(text::length("h\xC3\xA9llo") == 5);
  CHECK(text::truncated("h\xC3\xA9llo", 2) == "h\xC3\xA9");
  CHECK(text::truncated("abc", 10) == "abc");
  CHECK(text::is_valid_utf8("h\xC3\xA9llo \xE2\x82\xAC \xF0\x9F\x98\x80"));
  for (const char* bad : {"\xC3", "\xC0\xAF", "\xE0\x80\xAF", "\xED\xA0\x80", "\xF4\x90\x80\x80", "\xFF", "a\x80"}) {
    CHECK_FALSE(text::is_valid_utf8(bad));
  }
  CHECK(text::plural(1, "record", "records") == "1 record");
  CHECK(text::plural(3, "record", "records") == "3 records");
  CHECK(text::concat("a", 1, 'b', std::string("c"), true) == "a1bctrue");
}

TEST_CASE("utf8 validation never reads past the end of random bytes") {
  testkit::Random random(17);
  for (int round = 0; round < 2000; ++round) {
    const std::string bytes = random.bytes(24);
    const bool valid = text::is_valid_utf8(bytes);
    if (valid) {
      CHECK(text::length(bytes) <= bytes.size());
      CHECK(text::truncated(bytes, 3).size() <= bytes.size());
    }
  }
}

TEST_CASE("identity is complete") {
  CHECK_FALSE(meta::app_name.empty());
  CHECK_FALSE(meta::slug.empty());
  CHECK_FALSE(meta::version().empty());
  CHECK(meta::app_id.ends_with(meta::slug));
  for (const char letter : meta::slug) {
    CHECK(((letter >= 'a' && letter <= 'z') || (letter >= '0' && letter <= '9') || letter == '-'));
  }
  CHECK(text::contains(meta::license_text(), "MIT License"));
  std::string copyright;
  for (const std::string& line : text::lines(meta::license_text())) {
    if (line.starts_with("Copyright (c) ")) {
      copyright = line;
    }
  }
  CHECK_MESSAGE(copyright.ends_with(text::concat(" ", meta::publisher)), copyright);
  std::vector<std::string> opening = text::lines(meta::notices_text());
  opening.resize(std::min<std::size_t>(opening.size(), 8));
  CHECK(text::contains(text::join(opening, "\n"), text::concat("published by ", meta::publisher, " under")));
  CHECK(text::contains(meta::usage_text(), "Getting started"));
}

TEST_CASE("the build system and the metadata module agree") {
  CHECK(meta::app_name == app::build_config::app_name);
  CHECK(meta::slug == app::build_config::slug);
  CHECK(meta::app_id == app::build_config::app_id);
  CHECK(meta::publisher == app::build_config::publisher);
  const meta::BuildInfo info = meta::build(true);
  CHECK(info.version == meta::version());
  CHECK(info.dashboard);
  CHECK_FALSE(info.os.empty());
  CHECK_FALSE(info.arch.empty());
  CHECK((info.profile == "release" || info.profile == "debug"));
}

TEST_CASE("environment names follow the setting keys") {
  CHECK(meta::env_name("appearance.density") == text::concat(meta::env_prefix, "_APPEARANCE_DENSITY"));
  CHECK(meta::env_name("config-dir") == text::concat(meta::env_prefix, "_CONFIG_DIR"));
}

TEST_CASE("only links below the repository are accepted") {
  const std::string base(meta::repository_url);
  for (const std::string& link :
       {meta::source_url(), meta::advisory_url(), meta::new_issue_url("A title", "Line one\nLine two & more")}) {
    CHECK_MESSAGE(meta::is_repository_url(link), link);
  }
  const std::vector<std::string> rejected = {
      "https://example.test/unalivin/workspace",
      base + "-other",
      base + "/../other",
      base + "//issues",
      base + "/%2e%2e/other",
      base + "/a%2Fb",
      base + "/a\\b",
      base + "/a b",
      base + "/caf\xC3\xA9",
      text::replace_all(base, "https://", "http://"),
      text::replace_all(base, "https://", "https://user@"),
      "",
  };
  for (const std::string& link : rejected) {
    CHECK_FALSE_MESSAGE(meta::is_repository_url(link), link);
  }
}

TEST_CASE("issue links encode every reserved character") {
  const std::string link = meta::new_issue_url("a b&c", "x=y\n#z");
  CHECK_MESSAGE(link.ends_with("/issues/new?title=a%20b%26c&body=x%3Dy%0A%23z"), link);
}

TEST_CASE("known moments round trip") {
  const Timestamp moment = Timestamp::from_civil({2026, 3, 2, 9, 30, 0}).value();
  CHECK(moment.unix_seconds() == 1'772'443'800);
  CHECK(moment.to_rfc3339() == "2026-03-02T09:30:00Z");
  CHECK(Timestamp::parse("2026-03-02T09:30:00Z") == moment);
  CHECK(Timestamp::from_unix(0).to_rfc3339() == "1970-01-01T00:00:00Z");
  CHECK(Timestamp::from_unix(951'782'400).to_rfc3339() == "2000-02-29T00:00:00Z");
  CHECK(testkit::fixed_time() == moment);
}

TEST_CASE("display applies the offset and drops seconds") {
  const Timestamp moment = Timestamp::from_civil({2026, 12, 31, 23, 15, 59}).value();
  CHECK(moment.display(0) == "2026-12-31 23:15");
  CHECK(moment.display(120) == "2027-01-01 01:15");
  CHECK(moment.display(-330) == "2026-12-31 17:45");
  CHECK(moment.minute_of_day(0) == 23 * 60 + 15);
}

TEST_CASE("parse rejects malformed text") {
  for (const char* candidate :
       {"", "2026-03-02", "2026-03-02T09:30:00", "2026-13-02T09:30:00Z", "2026-02-30T09:30:00Z", "2026-03-02 09:30:00Z",
        "2026-03-02T24:00:00Z", "2026-00-10T00:00:00Z", "0000-01-01T00:00:00Z",
        "\xEF\xBC\x92\x30\x32\x36-03-02T09:30:00Z", "2026-3-02T09:30:00ZZ", "2026-+3-02T09:30:00Z"}) {
    CHECK_FALSE_MESSAGE(Timestamp::parse(candidate).has_value(), candidate);
  }
  CHECK_FALSE(Timestamp::from_civil({2026, 2, 29, 0, 0, 0}).has_value());
  CHECK(Timestamp::from_civil({2028, 2, 29, 0, 0, 0}).has_value());
  CHECK_FALSE(Timestamp::from_civil({2026, 0, 1, 0, 0, 0}).has_value());
  CHECK_FALSE(Timestamp::from_civil({2026, 13, 1, 0, 0, 0}).has_value());
}

TEST_CASE("zones shift the display and the minute of day") {
  const Timestamp moment = Timestamp::from_civil({2026, 3, 2, 23, 30, 0}).value();
  CHECK(Zone::utc().display(moment) == "2026-03-02 23:30 UTC");
  CHECK(Zone::fixed(60).display(moment) == "2026-03-03 00:30");
  CHECK(Zone::fixed(60).minute_of_day(moment) == 30);
  CHECK(Zone::utc().minute_of_day(moment) == 23 * 60 + 30);
  const std::int64_t local = Zone::local().offset_minutes(moment);
  CHECK(local >= -14 * 60);
  CHECK(local <= 14 * 60);
}

TEST_CASE("manual clock moves only when told") {
  const Timestamp start = testkit::fixed_time();
  const auto [clock, hand] = Clock::manual(start);
  CHECK(clock.now() == start);
  hand.advance(90);
  CHECK(clock.now() == start.plus_seconds(90));
  hand.set(start);
  CHECK(clock.now() == start);
  CHECK(Clock::fixed(start).now() == start);
  CHECK(Clock::system().now().unix_seconds() > 1'700'000'000);
}

TEST_CASE("every second round trips through text and parse survives noise") {
  testkit::Random random(2026);
  for (int round = 0; round < 5000; ++round) {
    const Timestamp moment = Timestamp::from_unix(random.between(-62'135'596'800, 253'402'300'799));
    CHECK(Timestamp::parse(moment.to_rfc3339()) == moment);
  }
  for (int round = 0; round < 5000; ++round) {
    const std::string noise = random.text(24, "0123456789-:TZ +x");
    const auto parsed = Timestamp::parse(noise);
    if (parsed) {
      CHECK(parsed->to_rfc3339() == noise);
    }
  }
}
