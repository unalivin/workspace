#include <sqlite3.h>

#include <string>

#include "core/records.hpp"
#include "core/text.hpp"
#include "support/fuzzing.hpp"

namespace {

constexpr const char* target = "records_search_expression";
constexpr const char* sql_probe =
    "CREATE VIRTUAL TABLE probe USING fts5 (body); INSERT INTO probe (body) VALUES ('sample text');";
constexpr const char* sql_match = "SELECT count(*) FROM probe WHERE probe MATCH ?1";

class Probe {
 public:
  Probe() {
    fuzzing::require(sqlite3_open(":memory:", &database_) == SQLITE_OK, target, "an in-memory database opens");
    fuzzing::require(sqlite3_exec(database_, sql_probe, nullptr, nullptr, nullptr) == SQLITE_OK, target,
                     "the probe table is created");
    fuzzing::require(sqlite3_prepare_v2(database_, sql_match, -1, &statement_, nullptr) == SQLITE_OK, target,
                     "the probe statement is prepared");
  }
  Probe(const Probe&) = delete;
  Probe& operator=(const Probe&) = delete;
  ~Probe() {
    sqlite3_finalize(statement_);
    sqlite3_close(database_);
  }

  void match(const std::string& expression) const {
    sqlite3_reset(statement_);
    sqlite3_bind_text(statement_, 1, expression.data(), static_cast<int>(expression.size()), SQLITE_TRANSIENT);
    const int outcome = sqlite3_step(statement_);
    if (outcome != SQLITE_ROW) {
      fuzzing::fail(target, expression, sqlite3_errmsg(database_));
    }
    sqlite3_reset(statement_);
  }

 private:
  sqlite3* database_ = nullptr;
  sqlite3_stmt* statement_ = nullptr;
};

}

extern "C" int LLVMFuzzerTestOneInput(const std::uint8_t* data, std::size_t size) {
  const auto text = fuzzing::text(data, size);
  if (!text) {
    return 0;
  }
  const std::string expression = app::records::search_expression(*text);
  std::size_t terms = 0;
  for (const std::string& term : app::text::split(expression, ' ')) {
    terms += term.empty() ? 0U : 1U;
  }
  fuzzing::require(terms <= 16, target, "more than 16 terms", expression);
  if (expression.empty()) {
    return 0;
  }
  static const Probe probe;
  probe.match(expression);
  return 0;
}
