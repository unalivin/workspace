#include "core/launcher.hpp"

#include <algorithm>
#include <array>
#include <istream>
#include <ostream>
#include <utility>

#include "core/meta.hpp"
#include "core/text.hpp"

namespace app::launcher {

namespace {

constexpr std::array<std::string_view, 7> automation_variables = {
    "CI", "GITHUB_ACTIONS", "GITLAB_CI", "JENKINS_URL", "BUILDKITE", "TF_BUILD", "TEAMCITY_VERSION"};
constexpr std::size_t longest_answer = 4096;
constexpr int attempts = 5;

bool valid_mode(std::string_view value) {
  return value == mode_gui || value == mode_tui;
}

Decision decision(Action action, std::string_view preferred, std::string reason) {
  return {action, std::string(preferred), std::move(reason)};
}

Decision open(const Input& input, std::string_view mode, std::string_view reason) {
  if (mode == mode_gui) {
    return decision(Action::OpenGui, mode, std::string(reason));
  }
  if (input.stdin_interactive && input.terminal_output) {
    return decision(Action::OpenTui, mode, std::string(reason));
  }
  if (input.desktop_launch && !input.headless) {
    return decision(Action::OpenTerminalWindow, mode, std::string(reason));
  }
  return decision(Action::PrintHelp, mode,
                  "this session is not interactive, so nothing was opened: run a subcommand or pass --ui");
}

struct Answer {
  std::string text;
  bool ended = false;
};

Answer read_answer(std::istream& input) {
  Answer answer;
  std::size_t taken = 0;
  while (taken < longest_answer) {
    const int next = input.get();
    if (next == std::istream::traits_type::eof()) {
      answer.ended = true;
      break;
    }
    ++taken;
    if (next == '\n') {
      break;
    }
    answer.text.push_back(static_cast<char>(next));
  }
  if (taken == longest_answer) {
    answer.ended = true;
  }
  answer.text = text::lower(text::trim_whitespace(answer.text));
  return answer;
}

}

std::string_view label(Action action) {
  switch (action) {
    case Action::RunCommand:
      return "run command";
    case Action::OpenGui:
      return "open dashboard";
    case Action::OpenTui:
      return "open terminal UI";
    case Action::PromptTerminal:
      return "prompt in terminal";
    case Action::ShowChooser:
      return "show chooser";
    case Action::OpenTerminalWindow:
      return "open terminal window";
    case Action::PrintHelp:
      return "print help";
    case Action::Fail:
      return "fail";
  }
  return "fail";
}

Decision decide(const Input& input) {
  if (input.has_subcommand) {
    return decision(Action::RunCommand, "", "an explicit subcommand never opens the chooser");
  }
  const std::string flag = text::lower(text::trim_whitespace(input.ui_flag));
  const std::string env = text::lower(text::trim_whitespace(input.env_ui));
  if (!flag.empty() && !valid_mode(flag)) {
    return decision(Action::Fail, "",
                    text::concat("--ui \"", input.ui_flag, "\" is not valid: use --ui gui or --ui tui"));
  }
  if (flag.empty() && !env.empty() && !valid_mode(env)) {
    return decision(Action::Fail, "",
                    text::concat(meta::env_name("ui"), "=\"", input.env_ui, "\" is not valid: use gui or tui"));
  }
  const bool terminal = input.stdin_interactive && input.terminal_output;
  const std::string& chosen = flag.empty() ? env : flag;
  const std::string origin = flag.empty() ? meta::env_name("ui") : std::string("--ui");
  if (chosen == mode_gui) {
    if (!input.gui_available) {
      return decision(Action::Fail, "",
                      text::concat(origin,
                                   " asks for the dashboard but this build has no dashboard: use --ui tui or "
                                   "a build with the GUI"));
    }
    if (input.headless) {
      return decision(Action::Fail, "",
                      text::concat(origin, " asks for the dashboard but no display is available: use --ui tui"));
    }
    return decision(Action::OpenGui, "", text::concat("requested with ", origin));
  }
  if (chosen == mode_tui) {
    if (terminal) {
      return decision(Action::OpenTui, "", text::concat("requested with ", origin));
    }
    if (input.desktop_launch && !input.headless) {
      return decision(Action::OpenTerminalWindow, "",
                      text::concat("requested with ", origin, " from a desktop launch"));
    }
    return decision(Action::Fail, "",
                    text::concat(origin,
                                 " asks for the terminal UI but this session is not an interactive terminal: "
                                 "run a subcommand instead"));
  }
  const std::string_view fallback = input.gui_available && !input.headless ? mode_gui : mode_tui;
  const bool remembered = valid_mode(input.remembered);
  const std::string_view preferred = remembered && (input.remembered == mode_tui || fallback == mode_gui)
                                         ? std::string_view(input.remembered)
                                         : fallback;
  if (input.headless) {
    return open(input, mode_tui, "no display is available, opening the terminal UI");
  }
  if (input.ci) {
    if (input.desktop_launch) {
      return open(input, preferred, "automation environment, not prompting");
    }
    return decision(Action::PrintHelp, preferred,
                    "automation environment detected, so nothing was opened: run a subcommand or pass --ui");
  }
  if (input.desktop_launch) {
    if (!input.gui_available) {
      return open(input, mode_tui, "this build has no dashboard");
    }
    if (remembered && !input.ask) {
      return open(input, preferred, "remembered choice");
    }
    if (input.no_prompt) {
      return open(input, preferred, "--no-prompt given");
    }
    return decision(Action::ShowChooser, preferred, "desktop launch");
  }
  if (!terminal) {
    return decision(Action::PrintHelp, preferred,
                    "standard input is not interactive, so nothing was opened: run a subcommand or pass --ui");
  }
  if (input.no_prompt) {
    return open(input, preferred, "--no-prompt given");
  }
  if (remembered && !input.ask) {
    return open(input, preferred, "remembered choice");
  }
  if (!input.gui_available) {
    return open(input, mode_tui, "this build has no dashboard");
  }
  return decision(Action::PromptTerminal, preferred, "interactive terminal");
}

PromptResult prompt_terminal(std::istream& input, std::ostream& output, std::string_view default_mode) {
  const std::string_view fallback = valid_mode(default_mode) ? default_mode : mode_gui;
  const auto marker = [fallback](std::string_view mode) { return mode == fallback ? " (default)" : ""; };
  output << "How do you want to open " << meta::app_name << "?\n";
  output << "  1  Dashboard window" << marker(mode_gui) << "\n";
  output << "  2  Terminal UI" << marker(mode_tui) << "\n";
  output << "  r  Toggle \"remember my choice\"\n";
  output << "  q  Quit\n";
  PromptResult quit{"", false, true};
  bool remember = false;
  for (int attempt = 0; attempt < attempts; ++attempt) {
    output << "Choice [Enter = " << (fallback == mode_gui ? "1" : "2") << ", remember: " << (remember ? "on" : "off")
           << "]: ";
    output.flush();
    const Answer answer = read_answer(input);
    const std::string& typed = answer.text;
    if (answer.ended && typed.empty()) {
      output << "\n";
      return quit;
    }
    if (typed.empty()) {
      return {std::string(fallback), remember, false};
    }
    if (typed == "1" || typed == "d" || typed == "dashboard" || typed == mode_gui) {
      return {std::string(mode_gui), remember, false};
    }
    if (typed == "2" || typed == "t" || typed == "terminal" || typed == mode_tui) {
      return {std::string(mode_tui), remember, false};
    }
    if (typed == "q" || typed == "quit" || typed == "exit") {
      return quit;
    }
    if (typed == "r" || typed == "remember") {
      remember = !remember;
    } else {
      output << "Enter 1, 2, r or q.\n";
    }
    if (answer.ended) {
      return quit;
    }
  }
  output << "No valid choice after several attempts.\n";
  return quit;
}

Session Session::current() {
  Session session;
  session.stdin_terminal = platform::stdin_is_terminal();
  session.stdout_terminal = platform::stdout_is_terminal();
  session.stderr_terminal = platform::stderr_is_terminal();
  session.console_owners = platform::console_owners();
  return session;
}

bool Session::ci() const {
  return std::ranges::any_of(automation_variables, [this](std::string_view name) {
    const std::optional<std::string> value = env.get(name);
    if (!value) {
      return false;
    }
    const std::string lowered = text::lower(*value);
    return lowered != "false" && lowered != "0";
  });
}

bool Session::headless() const {
  switch (os) {
    case platform::Os::Windows:
      return false;
    case platform::Os::Macos:
      return env.get("SSH_CONNECTION").has_value() || env.get("SSH_TTY").has_value();
    case platform::Os::Linux:
      return !env.get("DISPLAY").has_value() && !env.get("WAYLAND_DISPLAY").has_value();
  }
  return false;
}

bool Session::desktop_launch() const {
  if (headless() || ci()) {
    return false;
  }
  if (os == platform::Os::Windows) {
    return console_owners <= 1;
  }
  if (stdin_terminal || stdout_terminal || stderr_terminal) {
    return false;
  }
  const std::optional<std::string> term = env.get("TERM");
  return !term || *term == "dumb";
}

Input Session::input(Input base) const {
  base.stdin_interactive = stdin_terminal;
  base.terminal_output = stdout_terminal;
  base.ci = ci();
  base.headless = headless();
  base.desktop_launch = desktop_launch();
  return base;
}

bool requests_terminal(const std::vector<std::string>& arguments) {
  for (std::size_t index = 0; index < arguments.size(); ++index) {
    const std::string& argument = arguments[index];
    if (argument == "--") {
      return false;
    }
    if (argument == "--ui=tui" ||
        (argument == "--ui" && index + 1 < arguments.size() && arguments[index + 1] == mode_tui)) {
      return true;
    }
  }
  return false;
}

}
