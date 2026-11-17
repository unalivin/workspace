#include "core/csv.hpp"
#include "core/error.hpp"
#include "support/fuzzing.hpp"

extern "C" int LLVMFuzzerTestOneInput(const std::uint8_t* data, std::size_t size) {
  const auto text = fuzzing::text(data, size);
  if (!text) {
    return 0;
  }
  try {
    const std::vector<app::csv::Row> rows = app::csv::parse(*text, 64);
    fuzzing::require(rows.size() <= 64, "csv_parse", "more rows than the limit were returned");
  } catch (const app::AppError&) {
    return 0;
  }
  return 0;
}
