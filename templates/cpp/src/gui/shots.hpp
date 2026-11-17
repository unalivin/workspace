#pragma once

#include <array>
#include <string>
#include <string_view>
#include <utility>
#include <vector>

#include "core/service.hpp"
#include "gui/model.hpp"
#include "gui/render.hpp"

namespace app::gui {

class View;

}

namespace app::gui::shots {

inline constexpr float width = 1180.0F;
inline constexpr float height = 760.0F;

inline constexpr std::array<std::string_view, 14> names = {
    "first-start",   "overview", "records", "columns",   "notifications", "rules",     "settings",
    "settings-dark", "about",    "guide",   "shortcuts", "diagnostics",   "collapsed", "chooser",
};

[[nodiscard]] Picture dashboard(Stage& stage, View& view);
[[nodiscard]] Picture chooser(bool dark);
[[nodiscard]] std::vector<std::pair<std::string_view, Picture>> screens(service::App& app, Options options);

}
