#include "support/harness.hpp"

#include <doctest/doctest.h>

#include <algorithm>
#include <sstream>
#include <utility>

#include "app/cli.hpp"
#include "core/error.hpp"
#include "core/files.hpp"
#include "core/meta.hpp"
#include "core/text.hpp"

namespace testkit {

namespace files = app::files;
namespace text = app::text;
using app::platform::Os;

Harness::Harness(std::string_view label) : home(label) {
  variables["HOME"] = files::shown(home.path());
}

void Harness::set(std::string_view name, std::string_view value) {
  variables[std::string(name)] = std::string(value);
}

void Harness::unset(std::string_view name) {
  const auto found = variables.find(name);
  if (found != variables.end()) {
    variables.erase(found);
  }
}

app::platform::Env Harness::environment() const {
  return app::platform::Env::from_pairs({variables.begin(), variables.end()});
}

app::service::Options Harness::options() const {
  app::service::Options made;
  made.os = Os::Linux;
  made.env = environment();
  made.clock = app::clock::Clock::fixed(fixed_time());
  made.zone = app::clock::Zone::utc();
  return made;
}

std::string Harness::path(std::string_view name) const {
  return files::shown(home.path() / files::from_utf8(name));
}

std::string Harness::visit(std::string_view name, const std::vector<std::string>& script) {
  const auto visits = static_cast<std::size_t>(std::ranges::count(opened_, name));
  opened_.emplace_back(name);
  return visits < script.size() ? script[visits] : std::string();
}

Outcome Harness::run(const std::vector<std::string>& arguments) {
  std::istringstream input(standard_input);
  std::ostringstream output;
  std::ostringstream errors;
  app::cli::Env env;
  env.args = arguments;
  env.input = &input;
  env.output = &output;
  env.errors = &errors;
  env.options = options();
  env.session.os = Os::Linux;
  env.session.env = environment();
  env.session.stdin_terminal = terminal;
  env.session.stdout_terminal = terminal;
  env.session.stderr_terminal = terminal;
  env.session.console_owners = 0;
  env.executable = files::shown(home.path() / "bin" / files::from_utf8(app::meta::slug));
  if (gui) {
    env.hooks.open_gui = [this](app::service::App&) { return visit("gui", *gui); };
  }
  if (tui) {
    env.hooks.open_tui = [this](app::service::App&) { return visit("tui", *tui); };
  }
  env.hooks.show_chooser = [this](std::string_view preferred) {
    opened_.push_back(text::concat("chooser default ", preferred));
    if (!chooser) {
      throw app::AppError::io("no chooser answer");
    }
    return *chooser;
  };
  if (window) {
    env.hooks.open_terminal_window = [this](const std::string& executable, const std::vector<std::string>& words) {
      opened_.push_back(text::concat("window ", executable, " ", text::join(words, " ")));
    };
  }
  env.hooks.deliver = [this](std::string_view title, std::string_view body) {
    delivered.push_back(text::concat(title, " | ", body));
  };
  const std::string home_dir = files::shown(home.path());
  env.hooks.autostart = [home_dir] {
    app::notifier::Autostart entry;
    entry.os = Os::Linux;
    entry.home_dir = home_dir;
    entry.executable = "/opt/app/templateapp";
    return entry;
  };
  env.hooks.wait = [](std::int64_t) { return false; };
  const int code = app::cli::run(env);
  return {output.str(), errors.str(), code};
}

std::string Harness::ok(const std::vector<std::string>& arguments) {
  Outcome outcome = run(arguments);
  INFO(text::join(arguments, " "), "\nstdout:\n", outcome.output, "\nstderr:\n", outcome.errors);
  CHECK(outcome.code == app::cli::exit_ok);
  return std::move(outcome.output);
}

std::string Harness::fails(const std::vector<std::string>& arguments, int code, std::string_view fragment) {
  Outcome outcome = run(arguments);
  INFO(text::join(arguments, " "), " should mention ", std::string(fragment), "\nstdout:\n", outcome.output,
       "\nstderr:\n", outcome.errors);
  CHECK(outcome.code == code);
  CHECK(text::contains(outcome.errors, fragment));
  return std::move(outcome.errors);
}

std::vector<std::string> Harness::opened() {
  return std::exchange(opened_, {});
}

}
