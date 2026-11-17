#include <iostream>
#include <string>
#include <string_view>
#include <vector>

#include "app/cli.hpp"
#include "core/error.hpp"
#include "core/files.hpp"
#include "core/launcher.hpp"
#include "core/meta.hpp"
#include "core/notifier.hpp"
#include "core/platform.hpp"
#include "core/service.hpp"
#include "tui/tui.hpp"

#if defined(PROJECT_HAS_GUI)
#include "gui/gui.hpp"
#endif

int main(int argc, char** argv) {
  namespace platform = app::platform;
  platform::prepare_console();
  app::cli::Env env;
  env.args = platform::process_arguments(argc, argv);
  env.input = &std::cin;
  env.output = &std::cout;
  env.errors = &std::cerr;
  env.options = app::service::Options::system();
  env.session = app::launcher::Session::current();
  const platform::Os os = platform::current_os();
  const platform::Env environment = env.options.env;
  std::filesystem::path executable;
  try {
    executable = platform::current_executable();
  } catch (const app::AppError&) {
    executable = app::files::from_utf8(app::meta::slug);
  }
  env.executable = app::files::shown(executable);
  const auto autostart = [os, environment, executable] {
    return app::notifier::Autostart::for_system(os, environment, executable);
  };
  const auto open_url = [os](std::string_view url) {
    platform::open_repository_url(os, url, platform::spawn_detached);
  };
  env.hooks.deliver = [os](std::string_view title, std::string_view body) {
    app::notifier::deliver_with_system(os, title, body);
  };
  env.hooks.autostart = autostart;
  env.hooks.open_terminal_window = [os](const std::string& target, const std::vector<std::string>& arguments) {
    platform::open_terminal_window(os, target, arguments, platform::spawn_detached);
  };
  env.hooks.open_tui = [&](app::service::App& opened) {
    app::tui::Options options;
    options.env = environment;
    options.system_dark = app::tui::dark_background(environment);
    options.autostart = autostart();
#if defined(PROJECT_HAS_GUI)
    options.has_dashboard = true;
#endif
    options.open_url = open_url;
    return app::tui::run(opened, std::move(options));
  };
#if defined(PROJECT_HAS_GUI)
  const bool in_terminal = env.session.stdin_terminal && env.session.stdout_terminal;
  env.hooks.open_gui = [&](app::service::App& opened) {
    if (!in_terminal) {
      platform::release_console();
    }
    app::gui::Options options;
    options.env = environment;
    options.autostart = autostart();
    options.in_terminal = in_terminal;
    options.open_terminal = [os, program = env.executable](const std::vector<std::string>& arguments) {
      platform::open_terminal_window(os, program, arguments, platform::spawn_detached);
    };
    options.open_url = open_url;
    return app::gui::run(opened, std::move(options));
  };
  env.hooks.show_chooser = app::gui::choose;
#endif
  const int code = app::cli::run(env);
  std::cout.flush();
  return code;
}
