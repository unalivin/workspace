#pragma once

#include <string>

#include "core/platform.hpp"
#include "core/service.hpp"
#include "tui/model.hpp"

namespace app::tui {

[[nodiscard]] bool dark_background(const platform::Env& env);
[[nodiscard]] std::string run(service::App& app, Options options);

}
