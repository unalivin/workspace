#include "core/csv.hpp"

#include <utility>

#include "core/error.hpp"
#include "core/text.hpp"

namespace app::csv {

namespace {

constexpr std::string_view guarded = "=+-@\t\r'";

bool starts_guarded(std::string_view value) {
  return !value.empty() && guarded.find(value.front()) != std::string_view::npos;
}

AppError unreadable(std::size_t row, std::string_view reason) {
  return AppError::usage(text::concat("the CSV cannot be read near row ", row, ": ", reason));
}

void finish_row(std::vector<Row>& rows, Row& row, bool quoted, std::size_t max_rows) {
  if (row.size() == 1 && row.front().empty() && !quoted) {
    row.clear();
    return;
  }
  if (rows.size() == max_rows) {
    throw AppError::usage(text::concat("the import has more than ", max_rows == 0 ? 0 : max_rows - 1,
                                       " rows: split it into smaller files"));
  }
  rows.push_back(std::exchange(row, {}));
}

}

std::vector<Row> parse(std::string_view text, std::size_t max_rows) {
  std::vector<Row> rows;
  Row row;
  std::string field;
  bool quoted = false;
  bool was_quoted = false;
  bool row_quoted = false;
  for (std::size_t index = 0; index < text.size(); ++index) {
    const char letter = text[index];
    const bool more = index + 1 < text.size();
    if (quoted) {
      if (letter != '"') {
        field.push_back(letter);
      } else if (more && text[index + 1] == '"') {
        ++index;
        field.push_back('"');
      } else {
        quoted = false;
      }
      continue;
    }
    if (letter == '"') {
      if (!field.empty() || was_quoted) {
        throw unreadable(rows.size() + 1,
                         "a quote appears inside a field, write it as two quotes inside a quoted field");
      }
      quoted = true;
      was_quoted = true;
      row_quoted = true;
    } else if (letter == ',') {
      row.push_back(std::exchange(field, {}));
      was_quoted = false;
    } else if (letter == '\r' && more && text[index + 1] == '\n') {
      continue;
    } else if (letter == '\n') {
      row.push_back(std::exchange(field, {}));
      was_quoted = false;
      finish_row(rows, row, row_quoted, max_rows);
      row_quoted = false;
    } else if (was_quoted) {
      throw unreadable(rows.size() + 1, "text follows a closing quote");
    } else {
      field.push_back(letter);
    }
  }
  if (quoted) {
    throw unreadable(rows.size() + 1, "a quoted field is never closed");
  }
  if (!field.empty() || was_quoted || !row.empty()) {
    row.push_back(std::move(field));
    finish_row(rows, row, row_quoted, max_rows);
  }
  return rows;
}

void write_row(std::string& output, const std::vector<std::string_view>& fields) {
  for (std::size_t index = 0; index < fields.size(); ++index) {
    const std::string_view field = fields[index];
    if (index > 0) {
      output.push_back(',');
    }
    const bool needs_quotes = (field.empty() && fields.size() == 1) ||
                              field.find_first_of(",\"\n\r") != std::string_view::npos ||
                              (!field.empty() && (field.front() == ' ' || field.back() == ' '));
    if (needs_quotes) {
      output.push_back('"');
      output.append(text::replace_all(field, "\"", "\"\""));
      output.push_back('"');
    } else {
      output.append(field);
    }
  }
  output.push_back('\n');
}

std::string guard_cell(std::string_view value) {
  return starts_guarded(value) ? text::concat("'", value) : std::string(value);
}

std::string_view unguard_cell(std::string_view value) {
  if (!value.empty() && value.front() == '\'' && starts_guarded(value.substr(1))) {
    return value.substr(1);
  }
  return value;
}

}
