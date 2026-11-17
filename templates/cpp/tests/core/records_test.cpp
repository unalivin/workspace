#include <doctest/doctest.h>

#include <algorithm>
#include <functional>
#include <string>
#include <vector>

#include "core/error.hpp"
#include "core/files.hpp"
#include "core/notify.hpp"
#include "core/records.hpp"
#include "core/text.hpp"
#include "support/fixture.hpp"
#include "support/random.hpp"
#include "support/testkit.hpp"

using app::AppError;
using app::records::Input;
using app::records::Query;
using app::records::Record;
using app::records::Status;
using app::theme::Severity;
using testkit::Fixture;
using testkit::input;
namespace notify = app::notify;
namespace records = app::records;
namespace text = app::text;

namespace {

std::vector<std::int64_t> ids(const records::Records& all, const Query& query) {
  std::vector<std::int64_t> out;
  for (const Record& record : all.list(query)) {
    out.push_back(record.id);
  }
  return out;
}

Query with(const std::function<void(Query&)>& change) {
  Query query;
  change(query);
  return query;
}

}

TEST_CASE("records are created read updated and deleted") {
  const Fixture fixture("records-crud");
  const records::Records all = fixture.records();
  Input first;
  first.name = "  Review   the  router ";
  first.note = " line\r\nnext ";
  const Record created = all.create(first);
  CHECK(created.name == "Review the router");
  CHECK(created.note == "line\nnext");
  CHECK(created.revision == 1);
  CHECK(created.created_at == testkit::fixed_time());
  fixture.hand.advance(60);
  Input change = created.input();
  change.status = Status::InProgress;
  const Record updated = all.update(created.id, created.revision, change);
  CHECK(updated.status == Status::InProgress);
  CHECK(updated.revision == 2);
  CHECK(updated.updated_at == testkit::fixed_time().plus_seconds(60));
  CHECK(all.get(created.id) == updated);
  CHECK(all.by_name("Review the router").value().id == created.id);
  CHECK(all.count() == 1);
  all.remove(created.id);
  const std::string missing = testkit::failure_of([&] { static_cast<void>(all.get(created.id)); });
  CHECK_MESSAGE(text::contains(missing, "was not found"), missing);
  CHECK_THROWS_AS(all.remove(created.id), AppError);
  CHECK(all.count() == 0);
}

TEST_CASE("invalid input names the field and the fix") {
  const auto failure = [](const Input& candidate) {
    return testkit::failure_of([&] { static_cast<void>(records::normalize(candidate)); });
  };
  Input candidate;
  candidate.name = "   ";
  CHECK(text::contains(failure(candidate), "name: the name is empty"));
  candidate.name = std::string(201, 'a');
  CHECK(text::contains(failure(candidate), "more than 200 characters"));
  candidate.name = "bell\x07";
  CHECK(text::contains(failure(candidate), "control character"));
  candidate.name = "broken \xC3";
  CHECK(text::contains(failure(candidate), "name: the name is not valid UTF-8 text"));
  candidate.name = "ok";
  candidate.note = std::string(4001, 'n');
  CHECK(text::contains(failure(candidate), "more than 4000 characters"));
  candidate.note = "esc\x1B[0m";
  CHECK(text::contains(failure(candidate), "note: the note contains a control character"));
  candidate.note = "broken \xFF";
  CHECK(text::contains(failure(candidate), "note: the note is not valid UTF-8 text"));
  CHECK(text::contains(testkit::failure_of([] { static_cast<void>(records::parse_status("closed")); }),
                       "Use open, in_progress or done"));
  CHECK(text::contains(testkit::failure_of([] { static_cast<void>(records::parse_severity("urgent")); }),
                       "Use info, low, medium, high or critical"));
  CHECK(records::parse_status(" DONE ") == Status::Done);
  CHECK(records::parse_severity("High") == Severity::High);
  Input tabs;
  tabs.name = "tab\there";
  tabs.note = "keep\ttab";
  CHECK(records::normalize(tabs).note == "keep\ttab");
  CHECK(records::normalize(tabs).name == "tab here");
  Input wide;
  wide.name = std::string("\xE3\x80\x80") + "caf\xC3\xA9\xC2\xA0" + "bar" + "\xE2\x80\x83";
  CHECK(records::normalize(wide).name == "caf\xC3\xA9 bar");
  Input counted;
  for (int index = 0; index < 200; ++index) {
    counted.name.append("\xC3\xA9");
  }
  CHECK(records::normalize(counted).name == counted.name);
}

TEST_CASE("statuses and columns have names labels and distinct glyphs") {
  CHECK(records::name(Status::InProgress) == "in_progress");
  CHECK(records::label(Status::InProgress) == "In progress");
  CHECK(records::glyph(Status::Open) != records::glyph(Status::Done));
  CHECK(records::glyph(Status::Open) != records::glyph(Status::InProgress));
  CHECK_FALSE(records::find_status("closed").has_value());
  CHECK(records::column_label("id") == "ID");
  CHECK(records::column_label("updated") == "Updated");
  CHECK(records::column_label("owner") == "owner");
  Record record;
  CHECK(record.state_label() == "Active");
  record.demo = true;
  CHECK(record.state_label() == "Active, demo");
  record.enabled = false;
  CHECK(record.state_label() == "Disabled, demo");
  record.demo = false;
  CHECK(record.state_label() == "Disabled");
}

TEST_CASE("duplicate names are refused with a clear message") {
  const Fixture fixture("records-duplicate");
  const records::Records all = fixture.records();
  all.create(input("Same", Status::Open, Severity::Info));
  try {
    all.create(input("Same", Status::Open, Severity::Info));
    FAIL("the duplicate was accepted");
  } catch (const AppError& error) {
    CHECK_MESSAGE(text::contains(error.message(), "a record named \"Same\" already exists"), error.message());
    CHECK(error.is_usage());
  }
  const Record other = all.create(input("Other", Status::Open, Severity::Info));
  const std::string error =
      testkit::failure_of([&] { all.update(other.id, other.revision, input("Same", Status::Open, Severity::Info)); });
  CHECK_MESSAGE(text::contains(error, "already exists"), error);
  CHECK(all.count() == 2);
}

TEST_CASE("a stale revision or a removed record is a conflict") {
  const Fixture fixture("records-conflict");
  const records::Records all = fixture.records();
  const Record created = all.create(input("Shared", Status::Open, Severity::Info));
  const Record first = all.update(created.id, created.revision, input("Shared", Status::InProgress, Severity::Info));
  const std::string stale = testkit::failure_of(
      [&] { all.update(created.id, created.revision, input("Shared", Status::Done, Severity::Info)); });
  CHECK_MESSAGE(text::contains(stale, "changed by another window (now revision 2)"), stale);
  CHECK(all.get(created.id) == first);
  all.remove(created.id);
  const std::string removed = testkit::failure_of([&] { all.update(created.id, first.revision, created.input()); });
  CHECK_MESSAGE(text::contains(removed, "removed by another window"), removed);
}

TEST_CASE("listing filters sorts pages and searches") {
  const Fixture fixture("records-list");
  const records::Records all = fixture.records();
  const Record alpha = all.create(input("alpha router", Status::Open, Severity::Critical));
  fixture.hand.advance(10);
  Input second = input("Bravo switch", Status::Done, Severity::Low);
  second.note = "firewall notes";
  const Record bravo = all.create(second);
  fixture.hand.advance(10);
  const Record charlie = all.create(input("charlie router", Status::InProgress, Severity::Medium));
  all.set_enabled(charlie.id, false);
  using Ids = std::vector<std::int64_t>;
  CHECK(ids(all, {}) == Ids{alpha.id, bravo.id});
  CHECK(ids(all, with([](Query& query) { query.include_disabled = true; })) == Ids{alpha.id, bravo.id, charlie.id});
  CHECK(ids(all, with([](Query& query) {
              query.include_disabled = true;
              query.descending = true;
            })) == Ids{charlie.id, bravo.id, alpha.id});
  CHECK(ids(all, with([](Query& query) {
              query.sort = "severity";
              query.descending = true;
            })) == Ids{alpha.id, bravo.id});
  CHECK(ids(all, with([](Query& query) {
              query.sort = "status";
              query.include_disabled = true;
            })) == Ids{bravo.id, charlie.id, alpha.id});
  CHECK(ids(all, with([](Query& query) {
              query.sort = "updated";
              query.descending = true;
              query.include_disabled = true;
            }))
            .front() == charlie.id);
  CHECK(ids(all, with([](Query& query) { query.status = Status::Done; })) == Ids{bravo.id});
  CHECK(ids(all, with([](Query& query) { query.severity = Severity::Critical; })) == Ids{alpha.id});
  CHECK(ids(all, with([](Query& query) {
              query.search = "rout";
              query.include_disabled = true;
            })) == Ids{alpha.id, charlie.id});
  CHECK(ids(all, with([](Query& query) { query.search = "FIREWALL"; })) == Ids{bravo.id});
  CHECK(ids(all, with([](Query& query) { query.search = "\"*()"; })).empty());
  CHECK(ids(all, with([](Query& query) {
              query.limit = 1;
              query.offset = 1;
            })) == Ids{bravo.id});
  CHECK_THROWS_AS(static_cast<void>(all.list(with([](Query& query) { query.sort = "owner"; }))), AppError);
}

TEST_CASE("search follows edits and deletions") {
  const Fixture fixture("records-fts");
  const records::Records all = fixture.records();
  const Record created = all.create(input("printer queue", Status::Open, Severity::Info));
  const auto found = [&all](std::string_view wanted) {
    return all.list(with([wanted](Query& query) { query.search = std::string(wanted); })).size();
  };
  CHECK(found("printer") == 1);
  all.update(created.id, created.revision, input("scanner queue", Status::Open, Severity::Info));
  CHECK(found("printer") == 0);
  CHECK(found("scanner") == 1);
  all.remove(created.id);
  CHECK(found("scanner") == 0);
}

TEST_CASE("urgent and completed records publish notifications once") {
  const Fixture fixture("records-notify");
  const records::Records all = fixture.records();
  const notify::Notifications notifications = fixture.notifications();
  const Record calm = all.create(input("calm", Status::Open, Severity::Low));
  CHECK(notifications.unread_count() == 0);
  const Record raised = all.update(calm.id, calm.revision, input("calm", Status::Open, Severity::High));
  CHECK(notifications.unread_count() == 1);
  const Record still = all.update(raised.id, raised.revision, input("calm", Status::InProgress, Severity::Critical));
  CHECK(notifications.unread_count() == 1);
  all.update(still.id, still.revision, input("calm", Status::Done, Severity::Critical));
  const std::vector<notify::Notification> listed = notifications.list({});
  REQUIRE(listed.size() == 2);
  CHECK(std::ranges::any_of(listed, [](const notify::Notification& item) {
    return item.event_type == notify::event_record_completed && item.severity == Severity::Info;
  }));
  CHECK(std::ranges::all_of(listed, [&calm](const notify::Notification& item) {
    return item.record_id == calm.id && !text::contains(item.title, "calm");
  }));
  Input quiet = input("hidden", Status::Open, Severity::Critical);
  quiet.enabled = false;
  const Record hidden = all.create(quiet);
  CHECK(notifications.list({}).size() == 2);
  fixture.hand.advance(2 * 60 * 60);
  all.set_enabled(hidden.id, true);
  CHECK(notifications.list({}).size() == 3);
}

TEST_CASE("the overview counts follow the fixture") {
  const Fixture fixture("records-overview");
  fixture.store.execute_batch(app::files::read_all(testkit::source_root() / "testdata" / "fixtures" / "overview.sql"));
  const records::Overview overview = fixture.records().overview();
  CHECK(overview.open == 4);
  CHECK(overview.urgent == 3);
  CHECK(overview.disabled == 2);
  CHECK(overview.demo == 2);
  CHECK(overview.unread_notifications == 2);
  CHECK(overview.total() == 5);
  CHECK(overview.by_status(Status::Open) == 3);
  CHECK(overview.by_status(Status::Done) == 1);
  CHECK(overview.by_severity(Severity::Critical) == 2);
  CHECK(overview.by_severity(Severity::High) == 1);
  std::int64_t demo = 0;
  for (const records::Breakdown& row : overview.breakdown) {
    demo += row.demo;
  }
  CHECK(demo == 1);
}

TEST_CASE("demo data is labelled loaded once and removed alone") {
  const Fixture fixture("records-demo");
  const records::Records all = fixture.records();
  const auto demo_total = static_cast<std::int64_t>(records::demo_records().size());
  const Record own = all.create(input("my own record", Status::Open, Severity::Info));
  CHECK(all.load_demo() == records::demo_records().size());
  CHECK(all.load_demo() == 0);
  CHECK(all.demo_count() == demo_total);
  const Query everything = with([](Query& query) { query.include_disabled = true; });
  const std::vector<Record> listed = all.list(everything);
  CHECK(std::ranges::all_of(listed,
                            [](const Record& record) { return !record.demo || record.name.starts_with("Demo: "); }));
  CHECK(std::ranges::any_of(listed, [](const Record& record) { return record.state_label() == "Disabled, demo"; }));
  CHECK(all.remove_demo(true) == demo_total);
  CHECK(all.count() == demo_total + 1);
  CHECK(all.remove_demo(false) == demo_total);
  CHECK(all.list(everything) == std::vector<Record>{own});
}

TEST_CASE("hot queries use an index") {
  const Fixture fixture("records-plans");
  std::vector<notify::HotQuery> queries = records::hot_queries();
  for (const notify::HotQuery& query : notify::hot_queries()) {
    queries.push_back(query);
  }
  CHECK(queries.size() == 7);
  for (const notify::HotQuery& query : queries) {
    const std::vector<std::string> plan = fixture.store.query_plan(query.sql, query.parameters);
    const auto indexed = [](const std::string& step) {
      return text::contains(step, "USING INDEX") || text::contains(step, "USING COVERING INDEX") ||
             text::contains(step, "USING INTEGER PRIMARY KEY");
    };
    const auto full_scan = [&indexed](const std::string& step) { return step.starts_with("SCAN") && !indexed(step); };
    CHECK_MESSAGE((std::ranges::any_of(plan, indexed) && std::ranges::none_of(plan, full_scan)), query.label, ": ",
                  text::join(plan, " | "));
  }
}

TEST_CASE("a batch is created completely or not at all") {
  const Fixture fixture("records-batch");
  const records::Records all = fixture.records();
  CHECK(
      all.create_all({input("one", Status::Open, Severity::Info), input("two", Status::Open, Severity::Info)}).size() ==
      2);
  CHECK_THROWS_AS(
      all.create_all({input("three", Status::Open, Severity::Info), input("one", Status::Open, Severity::Info)}),
      AppError);
  CHECK(all.count() == 2);
  CHECK_FALSE(all.by_name("three").has_value());
}

TEST_CASE("search expressions are always valid full text queries") {
  const Fixture fixture("records-search-noise");
  fixture.records().create(input("sample text", Status::Open, Severity::Info));
  CHECK(records::search_expression("edge  router!") == "\"edge\"* \"router\"*");
  CHECK(records::search_expression("caf\xC3\xA9\xE2\x80\x94"
                                   "bar") == "\"caf\xC3\xA9\"* \"bar\"*");
  CHECK(records::search_expression(" \t ").empty());
  testkit::Random random(41);
  for (int round = 0; round < 300; ++round) {
    const std::string noise =
        round % 2 == 0 ? random.bytes(80) : random.text(80, "ab \"*():-^+.,'\\AND OR NOT NEAR{}[]\xC3\xA9\xE2\x80\x94");
    const std::string expression = records::search_expression(noise);
    CHECK(text::split(expression, ' ').size() <= 16);
    CHECK(text::is_valid_utf8(expression));
    Query query;
    query.search = noise;
    CHECK_NOTHROW(static_cast<void>(fixture.records().list(query)));
  }
}

TEST_CASE("normalized input is stable and within limits") {
  testkit::Random random(7);
  int accepted = 0;
  for (int round = 0; round < 400; ++round) {
    Input candidate;
    candidate.name =
        random.pieces(220, {"a", "b", " ", "\t", "\n", "\xC3\xA9", "\xE3\x80\x80", "\xC2\xA0", "Z", "9", "-"});
    candidate.note = random.pieces(200, {"x", "y", " ", "\r\n", "\n", "\t", "\xE2\x82\xAC", "."});
    try {
      const Input once = records::normalize(candidate);
      ++accepted;
      CHECK_FALSE(once.name.empty());
      CHECK(text::length(once.name) <= records::max_name_length);
      CHECK(records::normalize(once) == once);
    } catch (const AppError& error) {
      CHECK(error.is_usage());
    }
  }
  CHECK(accepted > 50);
}
