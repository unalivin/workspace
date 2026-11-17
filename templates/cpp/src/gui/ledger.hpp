#pragma once

#include <cstdint>
#include <string>
#include <vector>

#include "core/records.hpp"
#include "core/theme.hpp"
#include "gui/ui.hpp"

namespace app::gui::ledger {

[[nodiscard]] std::int64_t squares(std::int64_t total, std::int64_t unit);
[[nodiscard]] std::int64_t unit(std::int64_t largest, std::int64_t capacity);
[[nodiscard]] std::string caption(std::int64_t unit);
[[nodiscard]] std::int64_t count(const std::vector<records::Breakdown>& rows, records::Status status,
                                 theme::Severity level);
[[nodiscard]] std::int64_t largest(const std::vector<records::Breakdown>& rows);
[[nodiscard]] float height(const theme::Tokens& tokens);
std::int64_t show(const Ui& ui, const std::vector<records::Breakdown>& rows);

}
