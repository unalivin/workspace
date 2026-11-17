#pragma once

#include <string>
#include <string_view>

#include "core/launcher.hpp"
#include "core/service.hpp"
#include "gui/model.hpp"

namespace app::gui {

inline constexpr float min_width = 720.0F;
inline constexpr float min_height = 480.0F;

[[nodiscard]] std::string run(service::App& app, Options options);
[[nodiscard]] launcher::PromptResult choose(std::string_view default_mode);

}
