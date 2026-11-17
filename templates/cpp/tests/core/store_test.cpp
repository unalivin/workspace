#include <doctest/doctest.h>

#include <filesystem>
#include <fstream>
#include <string>
#include <vector>

#include "core/error.hpp"
#include "core/files.hpp"
#include "core/platform.hpp"
#include "core/store.hpp"
#include "core/text.hpp"
#include "support/testkit.hpp"

using app::AppError;
using app::store::Migration;
using app::store::Options;
using app::store::Statement;
using app::store::Store;
using app::store::Transaction;
namespace files = app::files;
namespace fs = std::filesystem;
namespace store = app::store;
namespace text = app::text;

namespace {

constexpr const char* insert_setting = "INSERT INTO settings (key, value, updated_at) VALUES (?1, ?2, ?3)";

Store ready(const testkit::TempDir& directory) {
  return Store::open_ready(directory.path() / "app.db");
}

void add_setting(const Store& target, std::string_view key) {
  target.prepare(insert_setting).bind(key).bind("b").bind("c").run();
}

}

TEST_CASE("a new database is created private migrated and configured") {
  const testkit::TempDir directory("store-new");
  const fs::path path = directory.path() / "nested" / "app.db";
  const Store database = Store::open(path);
  CHECK(database.schema_version() == 0);
  CHECK(database.migrate(store::embedded_migrations()) == 1);
  CHECK(database.migrate(store::embedded_migrations()) == 0);
  CHECK(database.schema_version() == 1);
  CHECK(text::lower(database.journal_mode()) == "wal");
  CHECK(database.foreign_keys_enabled());
  CHECK(database.busy_timeout() == store::default_busy_timeout);
  CHECK(database.has_full_text_search());
  CHECK(database.path() == path);
  CHECK_FALSE(database.read_only());
  database.integrity_check();
  database.foreign_key_check();
  CHECK(store::sqlite_version() == "3.54.1");
  if (app::platform::current_os() != app::platform::Os::Windows) {
    CHECK(files::permissions(path) == 0600U);
    CHECK(files::permissions(path.parent_path()) == 0700U);
  }
}

TEST_CASE("schema matches the committed snapshot") {
  const testkit::TempDir directory("store-snapshot");
  const std::string snapshot = ready(directory).schema_snapshot();
  const fs::path golden = testkit::source_root() / "testdata" / "schema.snapshot";
  if (app::platform::Env::system().get("UPDATE_SNAPSHOT")) {
    std::ofstream(golden, std::ios::binary | std::ios::trunc) << snapshot;
  }
  const std::string expected = text::replace_all(files::read_all(golden), "\r\n", "\n");
  CHECK_MESSAGE(snapshot == expected, "run UPDATE_SNAPSHOT=1 with the tests to accept the change");
}

TEST_CASE("files that are not databases are refused and left untouched") {
  const testkit::TempDir directory("store-garbage");
  const fs::path path = directory.path() / "notes.txt";
  files::write_private(path, "these are my notes, not a database", false);
  std::string error = testkit::failure_of([&] { static_cast<void>(Store::open(path)); });
  CHECK_MESSAGE(text::contains(error, "left untouched"), error);
  CHECK(files::read_all(path) == "these are my notes, not a database");
  const fs::path brief = directory.path() / "short.bin";
  files::write_private(brief, "SQLite", false);
  error = testkit::failure_of([&] { static_cast<void>(Store::open(brief)); });
  CHECK_MESSAGE(text::contains(error, "left untouched"), error);
  error = testkit::failure_of([&] { static_cast<void>(Store::open(directory.path())); });
  CHECK_MESSAGE(text::contains(error, "is a directory"), error);
  error = testkit::failure_of([] { static_cast<void>(Store::open("relative.db")); });
  CHECK_MESSAGE(text::contains(error, "must be absolute"), error);
}

TEST_CASE("read only stores reject writes and missing files") {
  const testkit::TempDir directory("store-readonly");
  const fs::path path = directory.path() / "app.db";
  const std::string missing = testkit::failure_of([&] { static_cast<void>(Store::open(path, Options{true, {}})); });
  CHECK_MESSAGE(text::contains(missing, "read-only"), missing);
  { const Store created = Store::open_ready(path); }
  const Store database = Store::open(path, Options{true, {}});
  CHECK(database.read_only());
  CHECK_THROWS_AS(add_setting(database, "a"), AppError);
  CHECK_THROWS_AS(database.migrate(store::embedded_migrations()), AppError);
}

TEST_CASE("a failing migration is rolled back completely") {
  const testkit::TempDir directory("store-rollback");
  const Store database = ready(directory);
  std::vector<Migration> migrations = store::embedded_migrations();
  migrations.push_back({2, "0002_broken.sql",
                        "CREATE TABLE half_done (id INTEGER PRIMARY KEY); INSERT INTO missing_table VALUES (1); PRAGMA "
                        "user_version = 2;"});
  const std::string error = testkit::failure_of([&] { database.migrate(migrations); });
  CHECK_MESSAGE(text::contains(error, "0002_broken.sql failed and was rolled back"), error);
  CHECK(database.schema_version() == 1);
  CHECK_FALSE(text::contains(database.schema_snapshot(), "half_done"));
}

TEST_CASE("a database from a newer build is refused") {
  const testkit::TempDir directory("store-newer");
  const Store database = ready(directory);
  database.execute_batch("PRAGMA user_version = 99");
  const std::string error = testkit::failure_of([&] { database.migrate(store::embedded_migrations()); });
  CHECK_MESSAGE(text::contains(error, "use a newer build"), error);
}

TEST_CASE("migration files must be named numbered and terminated") {
  CHECK_THROWS_AS(static_cast<void>(store::load_migrations({})), AppError);
  CHECK_THROWS_AS(static_cast<void>(store::load_migrations({{"init.sql", "PRAGMA user_version = 1;"}})), AppError);
  CHECK_THROWS_AS(static_cast<void>(store::load_migrations({{"0001_init.sql", "CREATE TABLE a (b);"}})), AppError);
  CHECK_THROWS_AS(static_cast<void>(store::load_migrations({{"0002_second.sql", "PRAGMA user_version = 2;"}})),
                  AppError);
  CHECK_THROWS_AS(static_cast<void>(store::load_migrations({{"0001_Init.sql", "PRAGMA user_version = 1;"}})), AppError);
  CHECK_THROWS_AS(static_cast<void>(store::load_migrations({{"001_init.sql", "PRAGMA user_version = 1;"}})), AppError);
  const std::vector<Migration> loaded = store::load_migrations(
      {{"0002_b.sql", "PRAGMA user_version = 2;"}, {"0001_a.sql", "PRAGMA user_version = 1;\n"}});
  REQUIRE(loaded.size() == 2);
  CHECK(loaded[0].number == 1);
  CHECK(loaded[1].number == 2);
  CHECK(loaded[0].sql == "PRAGMA user_version = 1;");
}

TEST_CASE("a second writer waits then reports a busy database") {
  const testkit::TempDir directory("store-busy");
  const fs::path path = directory.path() / "app.db";
  const Store first = Store::open_ready(path);
  const Store second = Store::open(path, Options{false, std::chrono::milliseconds(50)});
  {
    Transaction held(first);
    add_setting(first, "a");
    std::string error;
    try {
      const Transaction blocked(second);
    } catch (const AppError& busy) {
      error = busy.message();
      CHECK(busy.is_busy());
    }
    CHECK_MESSAGE(text::contains(error, "database is busy"), error);
    held.commit();
  }
  {
    Transaction free(second);
    free.commit();
  }
  const std::int64_t before = second.data_version();
  add_setting(first, "d");
  CHECK(second.data_version() != before);
}

TEST_CASE("a transaction that is not committed leaves no trace") {
  const testkit::TempDir directory("store-transaction");
  const Store database = ready(directory);
  {
    const Transaction abandoned(database);
    add_setting(database, "lost");
  }
  Statement count = database.prepare("SELECT count(*) FROM settings");
  REQUIRE(count.step());
  CHECK(count.integer(0) == 0);
  CHECK_THROWS_AS(add_setting(database, ""), store::SqliteError);
  try {
    add_setting(database, "dup");
    add_setting(database, "dup");
  } catch (const store::SqliteError& error) {
    CHECK(error.is_constraint());
    CHECK_FALSE(error.is_busy());
  }
}

TEST_CASE("statements bind text numbers and nothing and read them back") {
  const testkit::TempDir directory("store-statement");
  const Store database = ready(directory);
  Statement echo = database.prepare("SELECT ?1, ?2, ?3, ?4");
  CHECK(echo.parameters() == 4);
  echo.bind(std::int64_t{-7}).bind("h\xC3\xA9").bind_null().bind_optional(std::string("set"));
  REQUIRE(echo.step());
  CHECK(echo.integer(0) == -7);
  CHECK(echo.text(1) == "h\xC3\xA9");
  CHECK(echo.is_null(2));
  CHECK(echo.text(2).empty());
  CHECK(echo.text(3) == "set");
  CHECK_FALSE(echo.step());
  echo.reset();
  echo.bind(std::int64_t{1}).bind("").bind_optional(std::nullopt).bind(std::int64_t{2});
  REQUIRE(echo.step());
  CHECK(echo.integer(0) == 1);
  CHECK(echo.is_null(2));
  Statement blank = database.prepare("SELECT ?1 = '', typeof(?1)");
  blank.bind(std::string_view());
  REQUIRE(blank.step());
  CHECK(blank.integer(0) == 1);
  CHECK(blank.text(1) == "text");
  Statement moved = std::move(echo);
  CHECK(moved.integer(3) == 2);
  CHECK_THROWS_AS(static_cast<void>(database.prepare("SELECT FROM nothing at all")), AppError);
}
