#include <doctest/doctest.h>

#include <filesystem>
#include <fstream>
#include <string>
#include <vector>

#include "core/error.hpp"
#include "core/files.hpp"
#include "core/meta.hpp"
#include "core/platform.hpp"
#include "core/text.hpp"
#include "support/testkit.hpp"

using app::AppError;
using app::platform::Env;
using app::platform::Os;
namespace files = app::files;
namespace fs = std::filesystem;
namespace meta = app::meta;
namespace platform = app::platform;
namespace text = app::text;

namespace {
fs::path named(std::string_view name) {
  return files::from_utf8(name);
}
}

TEST_CASE("linux follows the xdg variables and falls back to home") {
  Env env = Env::from_pairs({{"HOME", "/home/ada"}});
  CHECK(platform::data_dir(Os::Linux, env) == fs::path("/home/ada/.local/share") / named(meta::slug));
  CHECK(platform::config_dir(Os::Linux, env) == fs::path("/home/ada/.config") / named(meta::slug));
  env = Env::from_pairs({{"HOME", "/home/ada"}, {"XDG_DATA_HOME", "/data"}, {"XDG_CONFIG_HOME", "/config"}});
  CHECK(platform::data_dir(Os::Linux, env) == fs::path("/data") / named(meta::slug));
  CHECK(platform::config_dir(Os::Linux, env) == fs::path("/config") / named(meta::slug));
  CHECK(platform::default_database_path(Os::Linux, env) ==
        fs::path("/data") / named(meta::slug) / named(text::concat(meta::slug, ".db")));
}

TEST_CASE("macos and windows use their application folders") {
  Env env = Env::from_pairs({{"HOME", "/Users/ada"}});
  CHECK(platform::data_dir(Os::Macos, env) ==
        fs::path("/Users/ada") / "Library" / "Application Support" / named(meta::app_name));
  CHECK(platform::config_dir(Os::Macos, env) == platform::data_dir(Os::Macos, env));
  env = Env::from_pairs({{"USERPROFILE", "C:/Users/ada"}, {"LOCALAPPDATA", "C:/Local"}, {"APPDATA", "C:/Roaming"}});
  CHECK(platform::data_dir(Os::Windows, env) == fs::path("C:/Local") / named(meta::app_name));
  CHECK(platform::config_dir(Os::Windows, env) == fs::path("C:/Roaming") / named(meta::app_name));
  env = Env::from_pairs({{"USERPROFILE", "C:/Users/ada"}});
  CHECK(platform::data_dir(Os::Windows, env) == fs::path("C:/Users/ada") / "AppData" / "Local" / named(meta::app_name));
  CHECK(platform::config_dir(Os::Windows, env) ==
        fs::path("C:/Users/ada") / "AppData" / "Roaming" / named(meta::app_name));
}

TEST_CASE("the configuration directory can be overridden and empty values are ignored") {
  const std::string name = meta::env_name("config_dir");
  Env env = Env::from_pairs({{"HOME", "/home/ada"}, {name, "/elsewhere"}});
  CHECK(platform::config_dir(Os::Linux, env) == fs::path("/elsewhere"));
  env = Env::from_pairs({{"HOME", "/home/ada"}, {name, "  "}, {"XDG_CONFIG_HOME", ""}});
  CHECK(platform::config_dir(Os::Linux, env) == fs::path("/home/ada/.config") / named(meta::slug));
}

TEST_CASE("a missing home is reported with the way out") {
  const std::string error =
      testkit::failure_of([] { static_cast<void>(platform::data_dir(Os::Linux, Env::from_pairs({}))); });
  CHECK_MESSAGE(text::contains(error, "--db"), error);
}

TEST_CASE("the system environment is readable and names the current system") {
  const Env env = Env::system();
  CHECK((env.get("PATH").has_value() || env.get("Path").has_value()));
  CHECK_FALSE(env.get("TEMPLATEAPP_SURELY_NOT_SET_ANYWHERE").has_value());
  CHECK(platform::label(platform::current_os()) == meta::build(false).os);
  CHECK(platform::label(Os::Linux) == "linux");
  CHECK(platform::label(Os::Macos) == "macos");
  CHECK(platform::label(Os::Windows) == "windows");
  const fs::path self = platform::current_executable();
  CHECK(self.is_absolute());
  CHECK(files::exists(self));
}

TEST_CASE("only repository links reach the browser") {
  std::vector<std::string> started;
  const platform::Runner run = [&started](const std::string& program, const std::vector<std::string>& arguments) {
    started.push_back(text::concat(program, " ", text::join(arguments, " ")));
  };
  platform::open_repository_url(Os::Linux, meta::advisory_url(), run);
  platform::open_repository_url(Os::Macos, meta::repository_url, run);
  platform::open_repository_url(Os::Windows, meta::repository_url, run);
  CHECK_THROWS_AS(platform::open_repository_url(Os::Linux, "https://example.test/x", run), AppError);
  REQUIRE(started.size() == 3);
  CHECK(started[0].starts_with("xdg-open https://"));
  CHECK(started[1].starts_with("open https://"));
  CHECK(started[2].starts_with("rundll32 url.dll,FileProtocolHandler https://"));
}

TEST_CASE("terminal windows are tried in order and arguments are quoted") {
  const std::vector<std::string> arguments = {"--ui", "tui"};
  const auto windows = platform::terminal_commands(Os::Windows, "C:/Apps/app.exe", arguments);
  CHECK(windows == std::vector<std::vector<std::string>>{{"cmd", "/c", "start", "", "C:/Apps/app.exe", "--ui", "tui"}});
  const auto mac = platform::terminal_commands(Os::Macos, "/Applications/It's \"here\"/app", arguments);
  CHECK(mac[0][0] == "osascript");
  CHECK_MESSAGE(text::contains(mac[0][2], "'/Applications/It'\\\\''s \\\"here\\\"/app' '--ui' 'tui'"), mac[0][2]);
  const auto linux_commands = platform::terminal_commands(Os::Linux, "/usr/bin/app", arguments);
  REQUIRE(linux_commands.size() == 8);
  CHECK(linux_commands[0] == std::vector<std::string>{"x-terminal-emulator", "-e", "/usr/bin/app", "--ui", "tui"});
  CHECK(linux_commands[5] == std::vector<std::string>{"kitty", "/usr/bin/app", "--ui", "tui"});
  std::vector<std::string> tried;
  const platform::Runner third_works = [&tried](const std::string& program, const std::vector<std::string>&) {
    tried.push_back(program);
    if (program != "konsole") {
      throw AppError::io(program + " missing");
    }
  };
  platform::open_terminal_window(Os::Linux, "/usr/bin/app", arguments, third_works);
  CHECK(tried == std::vector<std::string>{"x-terminal-emulator", "gnome-terminal", "konsole"});
  const platform::Runner none_work = [](const std::string& program, const std::vector<std::string>&) {
    throw AppError::io(program + " missing");
  };
  const std::string error =
      testkit::failure_of([&] { platform::open_terminal_window(Os::Linux, "/usr/bin/app", arguments, none_work); });
  CHECK_MESSAGE((text::contains(error, "--ui tui") && text::contains(error, "xterm missing")), error);
}

TEST_CASE("a browser that does not start tells the user to copy the link") {
  const platform::Runner run = [](const std::string&, const std::vector<std::string>&) {
    throw AppError::io("not found");
  };
  const std::string error =
      testkit::failure_of([&] { platform::open_repository_url(Os::Linux, meta::repository_url, run); });
  CHECK_MESSAGE(text::contains(error, "copy the link instead"), error);
}

TEST_CASE("windows command lines quote spaces quotes and trailing backslashes") {
  CHECK(platform::windows_command_line("app.exe", {"--ui", "tui"}) == "app.exe --ui tui");
  CHECK(platform::windows_command_line("C:\\Program Files\\app.exe", {"a b", "", "say \"hi\"", "dir\\"}) ==
        "\"C:\\Program Files\\app.exe\" \"a b\" \"\" \"say \\\"hi\\\"\" dir\\");
  CHECK(platform::windows_command_line("app", {"ends with slash \\"}) == "app \"ends with slash \\\\\"");
}

TEST_CASE("a program that does not exist is reported when it is started") {
  const std::string error =
      testkit::failure_of([] { platform::spawn_detached("templateapp-no-such-program-anywhere", {"--version"}); });
  CHECK_MESSAGE(text::contains(error, "could not be started"), error);
}

TEST_CASE("files are written whole private and never overwritten silently") {
  const testkit::TempDir directory("files");
  const fs::path path = directory.path() / "nested" / "export.json";
  files::write_private(path, "first", false);
  CHECK(files::read_all(path) == "first");
  const std::string refused = testkit::failure_of([&] { files::write_private(path, "second", false); });
  CHECK_MESSAGE(text::contains(refused, "already exists"), refused);
  CHECK(files::read_all(path) == "first");
  files::write_private(path, "second", true);
  CHECK(files::read_all(path) == "second");
  std::vector<std::string> names;
  for (const fs::directory_entry& entry : fs::directory_iterator(path.parent_path())) {
    names.push_back(files::shown(entry.path().filename()));
  }
  CHECK(names == std::vector<std::string>{"export.json"});
  if (platform::current_os() != Os::Windows) {
    CHECK(files::permissions(path) == 0600U);
    CHECK(files::permissions(path.parent_path()) == 0700U);
  }
  CHECK_THROWS_AS(files::write_private(directory.path(), "x", true), AppError);
}

TEST_CASE("oversized and missing files are not read") {
  const testkit::TempDir directory("files-limit");
  const fs::path path = directory.path() / "big.txt";
  {
    std::ofstream file(path, std::ios::binary);
    file << std::string(100, 'a');
  }
  CHECK(files::read_limited(path, 100).size() == 100);
  const std::string error = testkit::failure_of([&] { static_cast<void>(files::read_limited(path, 99)); });
  CHECK_MESSAGE(text::contains(error, "larger than 99 bytes"), error);
  CHECK_THROWS_AS(static_cast<void>(files::read_limited(directory.path() / "missing", 10)), AppError);
}

TEST_CASE("paths with characters outside ascii keep their bytes") {
  const std::string name = "caf\xC3\xA9 \xE2\x82\xAC.txt";
  const testkit::TempDir directory("files-utf8");
  const fs::path path = directory.path() / files::from_utf8(name);
  files::write_private(path, "ok", false);
  CHECK(files::shown(path.filename()) == name);
  CHECK(files::read_all(path) == "ok");
}

TEST_CASE("temporary directories are unique and removed") {
  fs::path kept;
  const testkit::TempDir second("kit");
  {
    const testkit::TempDir first("kit");
    CHECK(first.path() != second.path());
    kept = first.path();
    files::write_private(kept / "file", "x", false);
  }
  CHECK_FALSE(files::exists(kept));
  CHECK(files::is_directory(second.path()));
}
