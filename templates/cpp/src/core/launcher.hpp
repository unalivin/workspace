#pragma once

#include <iosfwd>
#include <string>
#include <string_view>
#include <vector>

#include "core/platform.hpp"

namespace app::launcher {

inline constexpr std::string_view mode_gui = "gui";
inline constexpr std::string_view mode_tui = "tui";

enum class Action {
  RunCommand,
  OpenGui,
  OpenTui,
  PromptTerminal,
  ShowChooser,
  OpenTerminalWindow,
  PrintHelp,
  Fail,
};

[[nodiscard]] std::string_view label(Action action);

struct Input {
  std::string ui_flag;
  bool ask = false;
  bool no_prompt = false;
  bool has_subcommand = false;
  std::string env_ui;
  std::string remembered;
  bool stdin_interactive = false;
  bool terminal_output = false;
  bool ci = false;
  bool headless = false;
  bool desktop_launch = false;
  bool gui_available = false;
  bool operator==(const Input&) const = default;
};

struct Decision {
  Action action = Action::Fail;
  std::string preferred;
  std::string reason;
  bool operator==(const Decision&) const = default;
};

[[nodiscard]] Decision decide(const Input& input);

struct PromptResult {
  std::string mode;
  bool remember = false;
  bool quit = false;
  bool operator==(const PromptResult&) const = default;
};

PromptResult prompt_terminal(std::istream& input, std::ostream& output, std::string_view default_mode);

struct Session {
  platform::Os os = platform::current_os();
  platform::Env env = platform::Env::system();
  bool stdin_terminal = false;
  bool stdout_terminal = false;
  bool stderr_terminal = false;
  unsigned console_owners = 0;

  [[nodiscard]] static Session current();
  [[nodiscard]] bool ci() const;
  [[nodiscard]] bool headless() const;
  [[nodiscard]] bool desktop_launch() const;
  [[nodiscard]] Input input(Input base) const;
};

[[nodiscard]] bool requests_terminal(const std::vector<std::string>& arguments);

}
