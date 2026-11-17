#include <doctest/doctest.h>

#include <set>
#include <string>
#include <tuple>
#include <vector>

#include "core/error.hpp"
#include "core/guide.hpp"
#include "core/records.hpp"
#include "core/text.hpp"
#include "core/transfer.hpp"
#include "support/fixture.hpp"
#include "support/random.hpp"
#include "support/testkit.hpp"

using app::AppError;
using app::records::Input;
using app::records::Record;
using app::records::Status;
using app::theme::Severity;
using app::transfer::Format;
namespace guide = app::guide;
namespace records = app::records;
namespace text = app::text;
namespace transfer = app::transfer;

namespace {

std::vector<Record> everything(const records::Records& all) {
  records::Query query;
  query.include_disabled = true;
  return all.list(query);
}

std::vector<Input> inputs_of(const std::vector<Record>& list) {
  std::vector<Input> out;
  for (const Record& record : list) {
    out.push_back(record.input());
  }
  return out;
}

Input named(std::string name) {
  Input input;
  input.name = std::move(name);
  return input;
}

const std::string sample =
    "Lead text.\n\n# Usage\n\nIntro.\n\n## Getting started\n\nOpen the **dashboard** and press `n`.\n\n```\n# not a "
    "heading\ncommand --flag\n```\n\n## Getting started\n\nSecond section with the same title.\n\n### Tables "
    "###\n\n| Key | Action |\n|-----|--------|\n| `q` | Quit |\n";

}

TEST_CASE("an export can be imported into an empty database in both formats") {
  const testkit::Fixture source("transfer-roundtrip");
  const records::Records all = source.records();
  all.create(Input{"=SUM(A1), \"quoted\"", Status::InProgress, Severity::High, "two\nlines, and a comma", false});
  all.create(named("plain"));
  const std::vector<Input> original = inputs_of(everything(all));
  for (const Format format : {Format::Csv, Format::Json}) {
    const std::string exported = transfer::export_text(format, everything(all));
    if (format == Format::Csv) {
      CHECK_MESSAGE(exported.starts_with("name,status,severity,note,enabled\n"), exported);
      CHECK_MESSAGE(text::contains(exported, "\"'=SUM(A1), \"\"quoted\"\"\""), exported);
    } else {
      CHECK_MESSAGE(exported.starts_with("[\n  {\n    \"name\": "), exported);
      CHECK(exported.ends_with("]\n"));
    }
    const testkit::Fixture target("transfer-target");
    const records::Records copy = target.records();
    const transfer::ImportResult result = transfer::import_records(copy, exported, format, false);
    CHECK_MESSAGE(result.created == 2, transfer::label(format));
    CHECK(result.updated == 0);
    CHECK(result.skipped == 0);
    CHECK(result.problems.empty());
    CHECK(inputs_of(everything(copy)) == original);
    const transfer::ImportResult again = transfer::import_records(copy, exported, format, false);
    CHECK(again.created == 0);
    CHECK(again.updated == 0);
    CHECK(again.skipped == 2);
  }
  CHECK(transfer::export_text(Format::Json, {}) == "[]\n");
  CHECK(transfer::export_text(Format::Csv, {}) == "name,status,severity,note,enabled\n");
}

TEST_CASE("a dry run reports and changes nothing") {
  const testkit::Fixture fixture("transfer-dry");
  const records::Records all = fixture.records();
  all.create(named("kept"));
  const std::string content = "name,status,severity\nkept,done,low\nfresh,open,info\n,open,info\n";
  const transfer::ImportResult result = transfer::import_records(all, content, Format::Csv, true);
  CHECK(result.summary() == "Dry run: would have created 1, updated 1, unchanged 0, rejected 1 row");
  CHECK(everything(all).size() == 1);
  CHECK(everything(all).front().status == Status::Open);
  CHECK(fixture.notifications().unread_count() == 0);
  const transfer::ImportResult real = transfer::import_records(all, content, Format::Csv, false);
  CHECK(real.summary() == "created 1, updated 1, unchanged 0, rejected 1 row");
  CHECK(everything(all).size() == 2);
}

TEST_CASE("bad rows are listed with their number and good rows still load") {
  const std::string content =
      "Name,Status,Severity,Note,Enabled\nfirst,open,low,,yes\nsecond,closed,low,,\nthird,open,urgent,,\nfourth,open,"
      "low,,maybe\nFIRST,open,low,,\n";
  const transfer::Parsed parsed = transfer::parse(content, Format::Csv);
  REQUIRE(parsed.inputs.size() == 1);
  CHECK(parsed.inputs.front().name == "first");
  REQUIRE(parsed.problems.size() == 4);
  CHECK_MESSAGE(parsed.problems[0].display().starts_with("row 3: status: unknown status \"closed\""),
                parsed.problems[0].display());
  CHECK_MESSAGE(parsed.problems[1].display().starts_with("row 4: severity: unknown severity"),
                parsed.problems[1].display());
  CHECK_MESSAGE(parsed.problems[2].display().starts_with("row 5: enabled:"), parsed.problems[2].display());
  CHECK(parsed.problems[3].display() == "row 6: name: the name repeats row 2. Keep one row per name");
}

TEST_CASE("unreadable imports are refused whole") {
  const std::vector<std::tuple<std::string, Format, std::string>> cases = {
      {"", Format::Csv, "the import is empty"},
      {"\xEF\xBB\xBF  \n", Format::Csv, "the import is empty"},
      {"status,severity\nopen,low\n", Format::Csv, "no \"name\" column"},
      {"name\n\"open", Format::Csv, "never closed"},
      {"{\"name\":\"x\"}", Format::Json, "the JSON cannot be read: the top level is not a list"},
      {"[{\"name\":\"x\",\"owner\":\"y\"}]", Format::Json, "the JSON cannot be read: item 1: unknown field \"owner\""},
      {"[{\"status\":\"open\"}]", Format::Json, "item 1: missing field \"name\""},
      {"[{\"name\":7}]", Format::Json, "item 1: \"name\" must be text"},
      {"[{\"name\":\"x\",\"enabled\":\"yes\"}]", Format::Json, "\"enabled\" must be true or false"},
      {"[\"x\"]", Format::Json, "item 1 is not an object"},
      {"[{\"name\":\"x\"}", Format::Json, "the JSON cannot be read"},
      {"\xFF\xFE", Format::Json, "not valid UTF-8"},
  };
  for (const auto& [raw, format, expected] : cases) {
    const std::string error = testkit::failure_of([&] { static_cast<void>(transfer::parse(raw, format)); });
    CHECK_MESSAGE(text::contains(error, expected), error);
  }
  const std::string large(transfer::max_import_bytes + 1, 'a');
  CHECK(text::contains(testkit::failure_of([&] { static_cast<void>(transfer::parse(large, Format::Csv)); }),
                       "larger than 5 MB"));
  CHECK(transfer::parse("\xEF\xBB\xBFname\nwith bom\n", Format::Csv).inputs.at(0).name == "with bom");
  const Input minimal = transfer::parse("[{\"name\":\"minimal\",\"enabled\":null}]", Format::Json).inputs.at(0);
  CHECK(minimal.status == Status::Open);
  CHECK(minimal.severity == Severity::Info);
  CHECK(minimal.enabled);
  std::string many = "[";
  for (std::size_t index = 0; index <= transfer::max_import_rows; ++index) {
    many.append(index == 0 ? "{}" : ",{}");
  }
  many.append("]");
  CHECK(text::contains(testkit::failure_of([&] { static_cast<void>(transfer::parse(many, Format::Json)); }),
                       "more than 10000 rows"));
  const std::string deep = std::string(100000, '[') + std::string(100000, ']');
  CHECK(text::contains(testkit::failure_of([&] { static_cast<void>(transfer::parse(deep, Format::Json)); }),
                       "item 1 is not an object"));
}

TEST_CASE("the format comes from the flag or the file name") {
  CHECK(transfer::format_for_file("list.CSV", "") == Format::Csv);
  CHECK(transfer::format_for_file("list.json", "") == Format::Json);
  CHECK(transfer::format_for_file("list.txt", "json") == Format::Json);
  CHECK(transfer::parse_format(" CSV ") == Format::Csv);
  CHECK(text::contains(testkit::failure_of([] { static_cast<void>(transfer::format_for_file("list.txt", "")); }),
                       "--format csv"));
  CHECK(text::contains(testkit::failure_of([] { static_cast<void>(transfer::format_for_file("list.csv", "xml")); }),
                       "use csv or json"));
  CHECK(transfer::label(Format::Json) == "json");
}

TEST_CASE("parsing arbitrary bytes fails only with a usage error") {
  testkit::Random random(77);
  const std::vector<std::string_view> pieces = {"[",     "]",    "{",        "}",  "\"name\"",    ":",  ",",
                                                "\"x\"", "true", "null",     "1",  "\"enabled\"", "\n", " ",
                                                "name",  "\"",   "\xC3\xA9", "\\", "e9"};
  for (int round = 0; round < 1500; ++round) {
    const std::string raw = round % 2 == 0 ? random.bytes(300) : random.pieces(40, pieces);
    for (const Format format : {Format::Csv, Format::Json}) {
      try {
        const transfer::Parsed parsed = transfer::parse(raw, format);
        CHECK(parsed.inputs.size() + parsed.problems.size() <= transfer::max_import_rows);
      } catch (const AppError& error) {
        CHECK(error.is_usage());
      }
    }
  }
}

TEST_CASE("any valid record survives a round trip in both formats") {
  testkit::Random random(3);
  const std::vector<std::string_view> pieces = {
      "a", "B", " ", ",", "\"", "'", "=", "+", "-", "@", "\xC3\xA9", "\xE2\x82\xAC", ";", "9", "\xF0\x9F\x98\x80"};
  std::vector<std::string_view> note_pieces = pieces;
  note_pieces.emplace_back("\n");
  note_pieces.emplace_back("\t");
  int checked = 0;
  for (int round = 0; round < 600; ++round) {
    Input candidate;
    candidate.name = random.pieces(40, pieces);
    candidate.note = random.pieces(60, note_pieces);
    candidate.enabled = round % 2 == 0;
    Input input;
    try {
      input = records::normalize(candidate);
    } catch (const AppError&) {
      continue;
    }
    ++checked;
    Record record;
    record.id = 1;
    record.name = input.name;
    record.note = input.note;
    record.enabled = input.enabled;
    record.revision = 1;
    for (const Format format : {Format::Csv, Format::Json}) {
      const transfer::Parsed parsed = transfer::parse(transfer::export_text(format, {record}), format);
      CHECK(parsed.problems.empty());
      CHECK(parsed.inputs == std::vector<Input>{input});
    }
  }
  CHECK(checked > 400);
}

TEST_CASE("headings become sections with unique anchors") {
  const guide::Guide parsed = guide::parse(sample);
  std::vector<std::tuple<std::size_t, std::string, std::string>> titles;
  for (const guide::Section& section : parsed.sections) {
    titles.emplace_back(section.level, section.title, section.anchor);
  }
  CHECK(titles == std::vector<std::tuple<std::size_t, std::string, std::string>>{
                      {1, "Introduction", "introduction"},
                      {1, "Usage", "usage"},
                      {2, "Getting started", "getting-started"},
                      {2, "Getting started", "getting-started-1"},
                      {3, "Tables", "tables"},
                  });
  CHECK(text::contains(parsed.sections[2].body, "# not a heading"));
  CHECK(parsed.find("getting-started-1") == 3);
  CHECK_FALSE(parsed.find("missing").has_value());
  std::vector<std::string> anchors;
  const auto repeated = guide::parse("# Step\n\n# Step\n\n# Step 1\n\n# Step\n\n# Step-1\n");
  for (const guide::Section& section : repeated.sections) {
    anchors.push_back(section.anchor);
  }
  CHECK(anchors == std::vector<std::string>{"step", "step-1", "step-1-1", "step-2", "step-1-2"});
  CHECK(guide::parse("").sections.empty());
  CHECK(guide::parse("#NoSpace\n####### seven\n# \n").sections.at(0).title == "Introduction");
  CHECK(guide::parse("# !!!\n").sections.at(0).anchor == "section");
}

TEST_CASE("search requires every term and shows an excerpt") {
  const guide::Guide parsed = guide::parse(sample);
  const std::vector<guide::Hit> hits = parsed.search("DASHBOARD press");
  REQUIRE(hits.size() == 1);
  CHECK(hits[0].section == 2);
  CHECK_MESSAGE(text::contains(hits[0].excerpt, "dashboard"), hits[0].excerpt);
  CHECK(parsed.search("dashboard missingword").empty());
  CHECK(parsed.search("   ").empty());
  CHECK(parsed.search("tables").at(0).excerpt == "Tables");
  std::string body = "# Long\n\n";
  for (int index = 0; index < 30; ++index) {
    body.append(
        "b\xC3\xA9"
        "fore ");
  }
  body.append(" needle ");
  for (int index = 0; index < 30; ++index) {
    body.append("after ");
  }
  const std::string excerpt = guide::parse(body).search("needle").at(0).excerpt;
  CHECK_MESSAGE((excerpt.starts_with("\xE2\x80\xA6") && excerpt.ends_with("\xE2\x80\xA6") &&
                 text::contains(excerpt, "needle") && text::is_valid_utf8(excerpt)),
                excerpt);
  CHECK(text::length(excerpt) <= 90);
}

TEST_CASE("plain text drops markup and keeps code and tables readable") {
  const std::string flat = guide::plain(sample);
  CHECK(text::contains(flat, "Open the dashboard and press n."));
  CHECK(text::contains(flat, "    command --flag"));
  CHECK(text::contains(flat, "Key  \xC2\xB7  Action"));
  CHECK(text::contains(flat, "q  \xC2\xB7  Quit"));
  CHECK_FALSE(text::contains(flat, "```"));
  CHECK_FALSE(text::contains(flat, "|--"));
  CHECK_FALSE(text::contains(flat, "**"));
}

TEST_CASE("parsing searching and flattening survive noise and keep anchors unique") {
  testkit::Random random(55);
  const std::vector<std::string_view> pieces = {
      "#", "`", "|", " ", "a", "b", "c", "0", "1", "2", "\xC3\xA9", "\xE2\x80\xA6", "\n", "-", "```"};
  for (int round = 0; round < 1500; ++round) {
    const std::string markdown = round % 4 == 0 ? random.bytes(200) : random.pieces(200, pieces);
    const guide::Guide parsed = guide::parse(markdown);
    for (const guide::Hit& hit : parsed.search(round % 2 == 0 ? random.pieces(4, pieces) : random.bytes(20))) {
      CHECK(hit.section < parsed.sections.size());
    }
    CHECK(guide::plain(markdown).size() <= markdown.size() * 5 + 8);
    std::set<std::string> anchors;
    for (const guide::Section& section : parsed.sections) {
      anchors.insert(section.anchor);
    }
    CHECK(anchors.size() == parsed.sections.size());
  }
}
