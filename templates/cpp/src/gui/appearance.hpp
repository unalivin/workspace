#pragma once

#include <optional>
#include <string_view>

#include "core/platform.hpp"

namespace app::gui {

[[nodiscard]] bool scheme_is_dark(std::string_view report);
[[nodiscard]] bool theme_is_dark(std::string_view name);
[[nodiscard]] std::optional<bool> system_dark(const platform::Env& env);

}
