#pragma once

#include <cstddef>
#include <string>
#include <string_view>
#include <vector>

namespace app::csv {

using Row = std::vector<std::string>;

[[nodiscard]] std::vector<Row> parse(std::string_view text, std::size_t max_rows);
void write_row(std::string& output, const std::vector<std::string_view>& fields);
[[nodiscard]] std::string guard_cell(std::string_view value);
[[nodiscard]] std::string_view unguard_cell(std::string_view value);

}
