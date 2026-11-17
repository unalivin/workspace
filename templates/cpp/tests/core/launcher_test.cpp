#include <doctest/doctest.h>

#include <filesystem>
#include <sstream>
#include <string>
#include <tuple>
#include <utility>
#include <vector>

#include "core/error.hpp"
#include "core/files.hpp"
#include "core/launcher.hpp"
#include "core/meta.hpp"
#include "core/notifier.hpp"
#include "core/notify.hpp"
#include "core/text.hpp"
#include "support/fixture.hpp"
#include "support/testkit.hpp"

using app::AppError;
using app::launcher::Action;
using app::launcher::Input;
using app::launcher::PromptResult;
using app::launcher::Session;
using app::notifier::Autostart;
using app::platform::Os;
namespace files = app::files;
namespace fs = std::filesystem;
namespace launcher = app::launcher;
namespace meta = app::meta;
namespace notifier = app::notifier;
namespace notify = app::notify;
namespace text = app::text;

namespace {

Input terminal() {
  Input input;
  input.stdin_interactive = true;
  input.terminal_output = true;
  input.gui_available = true;
  return input;
}

Input desktop() {
  Input input;
  input.desktop_launch = true;
  input.gui_available = true;
  return input;
}

Input pipe() {
  Input input;
  input.gui_available = true;
  return input;
}

template <typename Change>
Input with(Input base, Change change) {
  change(base);
  return base;
}

std::pair<PromptResult, std::string> ask(const std::string& typed, std::string_view default_mode) {
  std::istringstream input(typed);
  std::ostringstream output;
  PromptResult result = launcher::prompt_terminal(input, output, default_mode);
  return {std::move(result), output.str()};
}

Session session(Os os, std::vector<std::pair<std::string, std::string>> pairs, bool is_terminal, unsigned owners) {
  Session made;
  made.os = os;
  made.env = app::platform::Env::from_pairs(std::move(pairs));
  made.stdin_terminal = is_terminal;
  made.stdout_terminal = is_terminal;
  made.stderr_terminal = is_terminal;
  made.console_owners = owners;
  return made;
}

void urgent(const testkit::Fixture& fixture, std::string_view name) {
  fixture.records().create(testkit::input(name, app::records::Status::Open, app::theme::Severity::Critical));
}

Autostart autostart(Os os, std::string home, std::string executable) {
  Autostart entry;
  entry.os = os;
  entry.home_dir = std::move(home);
  entry.executable = std::move(executable);
  return entry;
}

bool ends_with_path(const fs::path& whole, const fs::path& tail) {
  return text::concat("/", whole.generic_string()).ends_with(text::concat("/", tail.generic_string()));
}

}

TEST_CASE("a subcommand always runs and never prompts") {
  for (Input base : {terminal(), desktop(), Input{}}) {
    base.has_subcommand = true;
    base.ui_flag = "nonsense";
    CHECK(launcher::decide(base).action == Action::RunCommand);
  }
}

TEST_CASE("the decision table holds") {
  const std::vector<std::tuple<std::string, Input, Action, std::string>> cases = {
      {"terminal first run", terminal(), Action::PromptTerminal, "gui"},
      {"terminal remembered tui", with(terminal(), [](Input& in) { in.remembered = "tui"; }), Action::OpenTui, "tui"},
      {"terminal remembered gui", with(terminal(), [](Input& in) { in.remembered = "gui"; }), Action::OpenGui, "gui"},
      {"terminal ask again", with(terminal(), [](Input& in) { in.remembered = "tui", in.ask = true; }),
       Action::PromptTerminal, "tui"},
      {"terminal no prompt", with(terminal(), [](Input& in) { in.no_prompt = true; }), Action::OpenGui, "gui"},
      {"terminal without dashboard", with(terminal(), [](Input& in) { in.gui_available = false; }), Action::OpenTui,
       "tui"},
      {"headless terminal", with(terminal(), [](Input& in) { in.headless = true; }), Action::OpenTui, "tui"},
      {"headless remembered gui", with(terminal(), [](Input& in) { in.headless = true, in.remembered = "gui"; }),
       Action::OpenTui, "tui"},
      {"headless pipe", with(pipe(), [](Input& in) { in.headless = true; }), Action::PrintHelp, "tui"},
      {"automation", with(terminal(), [](Input& in) { in.ci = true; }), Action::PrintHelp, "gui"},
      {"automation desktop", with(desktop(), [](Input& in) { in.ci = true; }), Action::OpenGui, "gui"},
      {"pipe", with(pipe(), [](Input& in) { in.terminal_output = true; }), Action::PrintHelp, "gui"},
      {"desktop first run", desktop(), Action::ShowChooser, "gui"},
      {"desktop remembered gui", with(desktop(), [](Input& in) { in.remembered = "gui"; }), Action::OpenGui, "gui"},
      {"desktop remembered tui", with(desktop(), [](Input& in) { in.remembered = "tui"; }), Action::OpenTerminalWindow,
       "tui"},
      {"desktop ask again", with(desktop(), [](Input& in) { in.remembered = "tui", in.ask = true; }),
       Action::ShowChooser, "tui"},
      {"desktop no prompt", with(desktop(), [](Input& in) { in.no_prompt = true; }), Action::OpenGui, "gui"},
      {"desktop without dashboard", with(desktop(), [](Input& in) { in.gui_available = false; }),
       Action::OpenTerminalWindow, "tui"},
      {"flag gui", with(terminal(), [](Input& in) { in.ui_flag = "gui"; }), Action::OpenGui, ""},
      {"flag tui", with(terminal(), [](Input& in) { in.ui_flag = " TUI "; }), Action::OpenTui, ""},
      {"flag tui from desktop", with(desktop(), [](Input& in) { in.ui_flag = "tui"; }), Action::OpenTerminalWindow, ""},
      {"flag tui in a pipe", with(pipe(), [](Input& in) { in.ui_flag = "tui"; }), Action::Fail, ""},
      {"flag gui headless", with(terminal(), [](Input& in) { in.ui_flag = "gui", in.headless = true; }), Action::Fail,
       ""},
      {"flag gui without dashboard", with(terminal(), [](Input& in) { in.ui_flag = "gui", in.gui_available = false; }),
       Action::Fail, ""},
      {"flag invalid", with(terminal(), [](Input& in) { in.ui_flag = "web"; }), Action::Fail, ""},
      {"env tui", with(terminal(), [](Input& in) { in.env_ui = "tui"; }), Action::OpenTui, ""},
      {"env invalid", with(terminal(), [](Input& in) { in.env_ui = "web"; }), Action::Fail, ""},
      {"flag beats env", with(terminal(), [](Input& in) { in.ui_flag = "gui", in.env_ui = "tui"; }), Action::OpenGui,
       ""},
      {"flag beats invalid env", with(terminal(), [](Input& in) { in.ui_flag = "tui", in.env_ui = "web"; }),
       Action::OpenTui, ""},
  };
  for (const auto& [name, input, action, preferred] : cases) {
    const launcher::Decision made = launcher::decide(input);
    CHECK_MESSAGE(made.action == action, name, ": ", made.reason);
    CHECK_MESSAGE(made.preferred == preferred, name, ": ", made.reason);
    CHECK_MESSAGE(!made.reason.empty(), name);
  }
  CHECK(launcher::label(Action::ShowChooser) == "show chooser");
  CHECK(text::contains(launcher::decide(with(terminal(), [](Input& in) { in.env_ui = "web"; })).reason,
                       meta::env_name("ui")));
}

TEST_CASE("the terminal prompt accepts keys words and enter") {
  CHECK(ask("\n", "tui").first == PromptResult{"tui", false, false});
  CHECK(ask("1\n", "tui").first.mode == "gui");
  CHECK(ask("Terminal\n", "gui").first.mode == "tui");
  CHECK(ask("r\n2\n", "gui").first == PromptResult{"tui", true, false});
  CHECK(ask("r\nr\n\n", "gui").first == PromptResult{"gui", false, false});
  CHECK(ask("q\n", "gui").first.quit);
  CHECK(ask("", "gui").first.quit);
  CHECK(ask("2", "gui").first.mode == "tui");
  CHECK(ask("r", "gui").first.quit);
  CHECK(ask(std::string(10000, 'x'), "gui").first.quit);
  const auto [result, shown] = ask("x\ny\nz\nw\nv\n", "gui");
  CHECK(result.quit);
  CHECK(text::contains(shown, "Enter 1, 2, r or q."));
  CHECK(text::contains(shown, "No valid choice"));
  const std::string greeting = ask("\n", "anything").second;
  CHECK_MESSAGE(text::contains(greeting, "Dashboard window (default)"), greeting);
  CHECK_MESSAGE(text::contains(greeting, "remember: off"), greeting);
}

TEST_CASE("sessions are classified per platform") {
  const Session linux_terminal = session(Os::Linux, {{"DISPLAY", ":0"}, {"TERM", "xterm"}}, true, 0);
  CHECK_FALSE(linux_terminal.headless());
  CHECK_FALSE(linux_terminal.desktop_launch());
  CHECK_FALSE(linux_terminal.ci());
  const Session linux_menu = session(Os::Linux, {{"WAYLAND_DISPLAY", "wayland-0"}}, false, 0);
  CHECK(linux_menu.desktop_launch());
  const Session linux_server = session(Os::Linux, {{"TERM", "xterm"}}, true, 0);
  CHECK(linux_server.headless());
  CHECK_FALSE(linux_server.desktop_launch());
  CHECK_FALSE(session(Os::Linux, {{"DISPLAY", ":0"}, {"TERM", "xterm"}}, false, 0).desktop_launch());
  CHECK(session(Os::Macos, {{"SSH_TTY", "/dev/ttys001"}}, true, 0).headless());
  CHECK(session(Os::Macos, {}, false, 0).desktop_launch());
  const Session windows_explorer = session(Os::Windows, {}, false, 1);
  CHECK(windows_explorer.desktop_launch());
  CHECK_FALSE(windows_explorer.headless());
  CHECK_FALSE(session(Os::Windows, {}, true, 2).desktop_launch());
  const Session automated = session(Os::Linux, {{"DISPLAY", ":0"}, {"CI", "true"}}, false, 0);
  CHECK(automated.ci());
  CHECK_FALSE(automated.desktop_launch());
  CHECK_FALSE(session(Os::Linux, {{"CI", "false"}}, false, 0).ci());
  const Input filled = linux_menu.input(pipe());
  CHECK(filled.desktop_launch);
  CHECK(filled.gui_available);
  CHECK_FALSE(filled.stdin_interactive);
  CHECK(Session::current().console_owners >= 1);
}

TEST_CASE("the terminal request is read from the arguments") {
  CHECK(launcher::requests_terminal({"--ui", "tui"}));
  CHECK(launcher::requests_terminal({"--db", "x", "--ui=tui"}));
  CHECK_FALSE(launcher::requests_terminal({"--ui", "gui"}));
  CHECK_FALSE(launcher::requests_terminal({"--", "--ui", "tui"}));
  CHECK_FALSE(launcher::requests_terminal({"--ui"}));
}

TEST_CASE("the first tick only records the position then new items are announced once") {
  const testkit::Fixture fixture("notifier");
  urgent(fixture, "before the notifier started");
  std::vector<std::pair<std::string, std::string>> shown;
  notifier::Runner runner;
  runner.database = fixture.directory.path() / "app.db";
  runner.state_dir = fixture.directory.path() / "state";
  runner.clock = fixture.clock;
  runner.deliver = [&shown](std::string_view title, std::string_view body) { shown.emplace_back(title, body); };
  CHECK(runner.tick() == 0);
  CHECK(runner.tick() == 0);
  CHECK(shown.empty());
  urgent(fixture, "second");
  urgent(fixture, "third");
  CHECK(runner.tick() == 2);
  CHECK(runner.tick() == 0);
  CHECK(shown ==
        std::vector<std::pair<std::string, std::string>>{{std::string(meta::app_name), "2 new notifications"}});
  urgent(fixture, "fourth");
  fixture.notifications().mark_all_read();
  CHECK(runner.tick() == 0);
  CHECK(shown.size() == 1);
  if (app::platform::current_os() != Os::Windows) {
    CHECK(files::permissions(runner.state_dir / files::from_utf8(notifier::state_file_name)) == 0600U);
  }
}

TEST_CASE("a failed delivery is retried and a broken state file starts over") {
  const testkit::Fixture fixture("notifier-retry");
  notifier::Runner runner;
  runner.database = fixture.directory.path() / "app.db";
  runner.state_dir = fixture.directory.path() / "state";
  runner.show_titles = true;
  runner.clock = fixture.clock;
  runner.deliver = [](std::string_view, std::string_view) { throw AppError::io("no notification service"); };
  CHECK(runner.tick() == 0);
  urgent(fixture, "needs delivery");
  const std::string error = testkit::failure_of([&] { static_cast<void>(runner.tick()); });
  CHECK_MESSAGE(text::contains(error, "cannot show the system notification"), error);
  std::vector<std::string> delivered;
  runner.deliver = [&delivered](std::string_view, std::string_view body) { delivered.emplace_back(body); };
  CHECK(runner.tick() == 1);
  CHECK(delivered == std::vector<std::string>{"A record needs attention"});
  const fs::path state = runner.state_dir / files::from_utf8(notifier::state_file_name);
  for (const std::string& broken :
       {std::string("{broken"), std::string("[1]"), std::string("{\"last_id\":-4,\"initialized\":true}"),
        std::string("{\"last_id\":\"7\",\"initialized\":true}"),
        std::string("{\"last_id\":18446744073709551615,\"initialized\":true}"), std::string(5000, ' ')}) {
    urgent(fixture, text::concat("while broken ", broken.size()));
    files::write_private(state, broken, true);
    CHECK(runner.tick() == 0);
    CHECK(delivered.size() == 1);
  }
  runner.database = fixture.directory.path() / "missing.db";
  CHECK(text::contains(testkit::failure_of([&] { static_cast<void>(runner.tick()); }), "read-only"));
}

TEST_CASE("the loop clamps the interval logs errors and stops when told") {
  const testkit::TempDir directory("notifier-loop");
  notifier::Runner runner;
  runner.database = directory.path() / "missing.db";
  runner.state_dir = directory.path();
  runner.clock = app::clock::Clock::fixed(testkit::fixed_time());
  runner.deliver = [](std::string_view, std::string_view) {};
  std::vector<std::int64_t> waits;
  std::vector<std::string> logged;
  const auto log = [&logged](std::string_view line) { logged.emplace_back(line); };
  runner.run(
      1, 3,
      [&waits](std::int64_t seconds) {
        waits.push_back(seconds);
        return true;
      },
      log);
  CHECK(waits == std::vector<std::int64_t>{15, 15, 15});
  CHECK(logged.size() == 3);
  runner.run(99999, 0, [](std::int64_t) { return false; }, log);
  CHECK(logged.size() == 4);
  CHECK(notifier::clamp_interval(5) == 15);
  CHECK(notifier::clamp_interval(60) == 60);
  CHECK(notifier::clamp_interval(99999) == 3600);
}

TEST_CASE("delivery commands pass text as data never as code") {
  const std::string hostile = "\"; rm -rf ~ #$(id)`id`";
  const notifier::DeliveryCommand on_linux = notifier::delivery_command(Os::Linux, "Title", hostile);
  CHECK(on_linux.program == "notify-send");
  CHECK(on_linux.arguments.at(2) == "--");
  CHECK(on_linux.arguments.at(4) == notify::safe_title(hostile));
  const notifier::DeliveryCommand on_mac = notifier::delivery_command(Os::Macos, "Title", hostile);
  for (std::size_t index = 0; index < 6; ++index) {
    CHECK_FALSE(text::contains(on_mac.arguments.at(index), "rm -rf"));
  }
  CHECK(on_mac.arguments.at(6) == "--");
  const notifier::DeliveryCommand on_windows = notifier::delivery_command(Os::Windows, "Title", hostile);
  for (const std::string& argument : on_windows.arguments) {
    CHECK_FALSE(text::contains(argument, "rm -rf"));
  }
  CHECK(on_windows.environment.at(1) ==
        std::pair<std::string, std::string>{"APP_NOTIFY_BODY", notify::safe_title(hostile)});
}

TEST_CASE("autostart entries are placed and written per platform") {
  const Autostart on_linux = autostart(Os::Linux, "/home/ada", "/opt/app/templateapp");
  CHECK(ends_with_path(on_linux.path(), fs::path(".config") / "autostart" /
                                            files::from_utf8(text::concat(meta::slug, "-notifier.desktop"))));
  CHECK(text::contains(on_linux.content(), "Exec=\"/opt/app/templateapp\" notifier run\n"));
  Autostart configured = on_linux;
  configured.config_home = "relative";
  CHECK(configured.path() == on_linux.path());
  const Autostart on_mac = autostart(Os::Macos, "/Users/ada", "/Applications/A&B/app");
  CHECK(ends_with_path(on_mac.path(), fs::path("Library") / "LaunchAgents" /
                                          files::from_utf8(text::concat(meta::app_id, ".notifier.plist"))));
  CHECK(text::contains(on_mac.content(), "<string>/Applications/A&amp;B/app</string>"));
  Autostart on_windows = autostart(Os::Windows, "", "C:\\Apps\\app.exe");
  CHECK_THROWS_AS(static_cast<void>(on_windows.path()), AppError);
  on_windows.app_data = "C:/Roaming";
  CHECK(ends_with_path(on_windows.path(),
                       fs::path("Startup") / files::from_utf8(text::concat(meta::slug, "-notifier.cmd"))));
  CHECK(on_windows.content() == "@echo off\r\nstart \"\" /min \"C:\\Apps\\app.exe\" notifier run\r\n");
  CHECK_THROWS_AS(static_cast<void>(autostart(Os::Linux, "", "/opt/app").path()), AppError);
  for (const std::string bad :
       {"", "relative/app", "/opt/a\"b", "/opt/$HOME/app", "/opt/a`b", "/opt/100%/app", "/opt/a\nb", "C:\\app.exe"}) {
    CHECK_THROWS_AS_MESSAGE(static_cast<void>(autostart(Os::Linux, "/home/ada", bad).content()), AppError, bad);
  }
  const app::platform::Env env = app::platform::Env::from_pairs({{"USERPROFILE", "C:/Users/ada"}, {"APPDATA", "C:/R"}});
  const Autostart found = Autostart::for_system(Os::Windows, env, "C:/Apps/app.exe");
  CHECK(found.home_dir == "C:/Users/ada");
  CHECK(found.app_data == "C:/R");
  CHECK(found.config_home.empty());
}

TEST_CASE("install status and uninstall follow the file") {
  const testkit::TempDir directory("autostart");
  const Autostart entry = autostart(Os::Linux, files::shown(directory.path()), "/opt/app/templateapp");
  CHECK_FALSE(entry.status().present);
  const fs::path planned = entry.install(true);
  CHECK_FALSE(files::exists(planned));
  const fs::path written = entry.install(false);
  CHECK(written == planned);
  CHECK(files::read_all(written) == entry.content());
  CHECK(entry.status() == notifier::Placement{true, written});
  CHECK(entry.uninstall(true) == notifier::Placement{true, written});
  CHECK(files::exists(written));
  CHECK(entry.uninstall(false) == notifier::Placement{true, written});
  CHECK(entry.uninstall(false) == notifier::Placement{false, written});
}

TEST_CASE("a quiet command reports its exit status and receives its environment") {
  if (app::platform::current_os() == Os::Windows) {
    CHECK(app::platform::run_quiet("cmd", {"/c", "if \"%APP_PROBE%\"==\"seen\" (exit 0) else (exit 3)"},
                                   {{"APP_PROBE", "seen"}}) == 0);
    CHECK(app::platform::run_quiet("cmd", {"/c", "exit 7"}, {}) == 7);
  } else {
    CHECK(app::platform::run_quiet("sh", {"-c", "test \"$APP_PROBE\" = seen"}, {{"APP_PROBE", "seen"}}) == 0);
    CHECK(app::platform::run_quiet("sh", {"-c", "test \"$APP_PROBE\" = seen"}, {}) == 1);
    CHECK(app::platform::run_quiet("sh", {"-c", "exit 7"}, {}) == 7);
  }
  const std::string missing = testkit::failure_of([] {
    const int code = app::platform::run_quiet("templateapp-no-such-program", {}, {});
    if (code == 127) {
      throw AppError::io("could not be started");
    }
  });
  CHECK(text::contains(missing, "could not be started"));
}
