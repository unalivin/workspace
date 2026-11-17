#include <doctest/doctest.h>

#include <filesystem>
#include <string>
#include <utility>
#include <vector>

#include "core/csv.hpp"
#include "core/error.hpp"
#include "core/files.hpp"
#include "core/text.hpp"
#include "core/userconfig.hpp"
#include "support/random.hpp"
#include "support/testkit.hpp"

using app::AppError;
namespace csv = app::csv;
namespace files = app::files;
namespace text = app::text;
namespace userconfig = app::userconfig;

TEST_CASE("rows fields quotes and line endings are read") {
  using Rows = std::vector<csv::Row>;
  CHECK(csv::parse("a,b,c\r\n\"x,1\",\"say \"\"hi\"\"\",\"two\nlines\"\n\nlast,,\n", 100) ==
        Rows{{"a", "b", "c"}, {"x,1", "say \"hi\"", "two\nlines"}, {"last", "", ""}});
  CHECK(csv::parse("only", 100) == Rows{{"only"}});
  CHECK(csv::parse("a,\"\"", 100) == Rows{{"a", ""}});
  CHECK(csv::parse("\"\"\nnext\n", 100) == Rows{{""}, {"next"}});
  CHECK(csv::parse("", 100).empty());
  CHECK(csv::parse("a\rb\n", 100) == Rows{{"a\rb"}});
}

TEST_CASE("malformed input is rejected with the row") {
  const std::vector<std::pair<std::string, std::string>> cases = {
      {"a,b\nc,\"open", "row 2: a quoted field is never closed"},
      {"a\"b,c\n", "row 1: a quote appears inside a field"},
      {"\"a\"b,c\n", "row 1: text follows a closing quote"},
  };
  for (const auto& [input, expected] : cases) {
    const std::string error = testkit::failure_of([&] { static_cast<void>(csv::parse(input, 100)); });
    CHECK_MESSAGE(text::contains(error, expected), error);
  }
  const std::string many = testkit::failure_of([] { static_cast<void>(csv::parse("1\n2\n3\n", 2)); });
  CHECK_MESSAGE(text::contains(many, "more than 1 rows"), many);
  CHECK(csv::parse("1\n2\n", 2).size() == 2);
}

TEST_CASE("formula cells are neutralised and restored") {
  for (const char* risky : {"=SUM(A1)", "+1", "-1", "@cmd", "\tx", "\rx"}) {
    const std::string guarded = csv::guard_cell(risky);
    CHECK_MESSAGE(guarded.starts_with('\''), risky);
    CHECK(csv::unguard_cell(guarded) == risky);
  }
  CHECK(csv::guard_cell("plain") == "plain");
  CHECK(csv::guard_cell("").empty());
  CHECK(csv::unguard_cell("'written by hand") == "'written by hand");
  CHECK(csv::unguard_cell(csv::guard_cell("'=kept as typed")) == "'=kept as typed");
  CHECK(csv::unguard_cell(csv::guard_cell("'")) == "'");
}

TEST_CASE("written rows are read back unchanged and parsing survives noise") {
  testkit::Random random(13);
  const std::vector<std::string_view> alphabet = {"a", "b", ",", "\"", "\n", "\r", " ", "=", "'", "\xC3\xA9", "\t"};
  for (int round = 0; round < 800; ++round) {
    std::vector<csv::Row> rows;
    std::string written;
    const auto row_count = random.between(0, 5);
    for (std::int64_t row_index = 0; row_index < row_count; ++row_index) {
      csv::Row row;
      const auto field_count = random.between(1, 4);
      for (std::int64_t field_index = 0; field_index < field_count; ++field_index) {
        row.push_back(random.pieces(12, alphabet));
      }
      csv::write_row(written, {row.begin(), row.end()});
      rows.push_back(std::move(row));
    }
    CHECK(csv::parse(written, 100) == rows);
  }
  for (int round = 0; round < 3000; ++round) {
    const std::string noise = round % 2 == 0 ? random.bytes(200) : random.pieces(60, alphabet);
    try {
      const std::vector<csv::Row> rows = csv::parse(noise, 50);
      CHECK(rows.size() <= 50);
    } catch (const AppError& error) {
      CHECK(error.is_usage());
    }
  }
}

TEST_CASE("a missing file means defaults and saved values come back") {
  const testkit::TempDir directory("config");
  const std::filesystem::path nested = directory.path() / "settings";
  CHECK(userconfig::load(nested) == userconfig::Config{});
  const userconfig::Config config{files::shown(directory.path() / "x.db"), "tui"};
  userconfig::save(nested, config);
  CHECK(userconfig::load(nested) == config);
  const std::string written = files::read_all(userconfig::file(nested));
  CHECK_MESSAGE((text::contains(written, "\"front_end\": \"tui\"") && written.ends_with('\n')), written);
}

TEST_CASE("broken files are reported with the way out") {
  const testkit::TempDir directory("config-broken");
  const std::filesystem::path path = userconfig::file(directory.path());
  const auto failure = [&](std::string_view content) {
    files::write_private(path, content, true);
    return testkit::failure_of([&] { static_cast<void>(userconfig::load(directory.path())); });
  };
  CHECK(text::contains(failure("{not json"), "delete it to start with defaults"));
  CHECK(text::contains(failure("[1, 2]"), "delete it to start with defaults"));
  CHECK(text::contains(failure("{\"front_end\":7}"), "delete it to start with defaults"));
  CHECK(text::contains(failure("{\"front_end\":\"web\"}"), "front_end must be"));
  CHECK(text::contains(failure("{\"database_path\":\"relative.db\"}"), "absolute path"));
  CHECK(text::contains(failure(std::string(userconfig::max_file_size + 1, ' ')), "larger than"));
  files::write_private(path, "{\"later_option\":1}", true);
  CHECK(userconfig::load(directory.path()) == userconfig::Config{});
  userconfig::Config web;
  web.front_end = "web";
  CHECK_THROWS_AS(userconfig::save(directory.path(), web), AppError);
}
