#pragma once

#include <cstddef>
#include <string>
#include <string_view>
#include <vector>

#include "core/records.hpp"

namespace app::transfer {

inline constexpr std::size_t max_import_bytes = std::size_t{5} << 20U;
inline constexpr std::size_t max_import_rows = 10000;

enum class Format { Csv, Json };

[[nodiscard]] Format parse_format(std::string_view value);
[[nodiscard]] Format format_for_file(std::string_view name, std::string_view explicit_format);
[[nodiscard]] std::string_view label(Format format);
[[nodiscard]] std::string export_text(Format format, const std::vector<records::Record>& list);

struct RowError {
  std::size_t row = 0;
  std::string message;

  [[nodiscard]] std::string display() const;
  bool operator==(const RowError&) const = default;
};

struct Parsed {
  std::vector<records::Input> inputs;
  std::vector<RowError> problems;
};

[[nodiscard]] Parsed parse(std::string_view raw, Format format);

struct ImportResult {
  std::size_t created = 0;
  std::size_t updated = 0;
  std::size_t skipped = 0;
  std::vector<RowError> problems;
  bool dry_run = false;

  [[nodiscard]] std::string summary() const;
  bool operator==(const ImportResult&) const = default;
};

ImportResult import_records(const records::Records& records, std::string_view raw, Format format, bool dry_run);

}
