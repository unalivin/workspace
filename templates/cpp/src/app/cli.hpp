#pragma once

#include <cstdint>
#include <functional>
#include <iosfwd>
#include <string>
#include <string_view>
#include <vector>

#include "core/launcher.hpp"
#include "core/notifier.hpp"
#include "core/service.hpp"

namespace app::cli {

inline constexpr int exit_ok = 0;
inline constexpr int exit_failure = 1;
inline constexpr int exit_usage = 2;

using FrontEnd = std::function<std::string(service::App&)>;
using Chooser = std::function<launcher::PromptResult(std::string_view)>;
using WindowOpener = std::function<void(const std::string&, const std::vector<std::string>&)>;

struct Hooks {
  FrontEnd open_gui;
  FrontEnd open_tui;
  Chooser show_chooser;
  WindowOpener open_terminal_window;
  notifier::Deliver deliver;
  std::function<notifier::Autostart()> autostart;
  std::function<bool(std::int64_t)> wait;
};

struct Env {
  std::vector<std::string> args;
  std::istream* input = nullptr;
  std::ostream* output = nullptr;
  std::ostream* errors = nullptr;
  service::Options options;
  launcher::Session session;
  std::string executable;
  Hooks hooks;
};

[[nodiscard]] int run(Env& env);
[[nodiscard]] std::vector<std::string> command_paths();
[[nodiscard]] std::string reference();
[[nodiscard]] std::string commands_document(std::string_view current);

}
