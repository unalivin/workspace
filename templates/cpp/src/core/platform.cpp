#include "core/platform.hpp"

#include <array>
#include <cerrno>
#include <cstdint>
#include <cstdio>
#include <cstdlib>
#include <system_error>

#include "core/error.hpp"
#include "core/files.hpp"
#include "core/meta.hpp"
#include "core/text.hpp"

#if defined(_WIN32)
#include <windows.h>

#include <io.h>
#include <shellapi.h>
#else
#include <fcntl.h>
#include <spawn.h>
#include <sys/wait.h>
#include <unistd.h>
#if defined(__APPLE__)
#include <mach-o/dyld.h>
#endif
extern char** environ;
#endif

namespace app::platform {

namespace fs = std::filesystem;

namespace {

#if defined(_WIN32)
std::wstring widen(std::string_view value) {
  if (value.empty()) {
    return {};
  }
  const int size = MultiByteToWideChar(CP_UTF8, 0, value.data(), static_cast<int>(value.size()), nullptr, 0);
  std::wstring wide(static_cast<std::size_t>(size), L'\0');
  MultiByteToWideChar(CP_UTF8, 0, value.data(), static_cast<int>(value.size()), wide.data(), size);
  return wide;
}

std::string narrow(std::wstring_view value) {
  if (value.empty()) {
    return {};
  }
  const int size =
      WideCharToMultiByte(CP_UTF8, 0, value.data(), static_cast<int>(value.size()), nullptr, 0, nullptr, nullptr);
  std::string out(static_cast<std::size_t>(size), '\0');
  WideCharToMultiByte(CP_UTF8, 0, value.data(), static_cast<int>(value.size()), out.data(), size, nullptr, nullptr);
  return out;
}
#endif

std::optional<std::string> system_variable(std::string_view name) {
#if defined(_WIN32)
  const std::wstring wide = widen(name);
  const DWORD size = GetEnvironmentVariableW(wide.c_str(), nullptr, 0);
  if (size == 0) {
    return std::nullopt;
  }
  std::wstring value(size, L'\0');
  const DWORD written = GetEnvironmentVariableW(wide.c_str(), value.data(), size);
  value.resize(written);
  return narrow(value);
#else
  const std::string owned(name);
  const char* value = std::getenv(owned.c_str());
  if (value == nullptr) {
    return std::nullopt;
  }
  return std::string(value);
#endif
}

fs::path home(const Env& env) {
  if (auto value = env.get("HOME")) {
    return files::from_utf8(*value);
  }
  if (auto value = env.get("USERPROFILE")) {
    return files::from_utf8(*value);
  }
  throw AppError::io("the home directory is unknown: set HOME, or pass --db <file>");
}

fs::path named(std::string_view name) {
  return files::from_utf8(name);
}

std::string shell_quote(std::string_view word) {
  return text::concat("'", text::replace_all(word, "'", "'\\''"), "'");
}

std::string windows_quote(std::string_view word) {
  if (!word.empty() && word.find_first_of(" \t\n\v\"") == std::string_view::npos) {
    return std::string(word);
  }
  std::string quoted = "\"";
  std::size_t slashes = 0;
  for (const char letter : word) {
    if (letter == '\\') {
      ++slashes;
      continue;
    }
    if (letter == '"') {
      quoted.append(slashes * 2 + 1, '\\');
    } else {
      quoted.append(slashes, '\\');
    }
    slashes = 0;
    quoted.push_back(letter);
  }
  quoted.append(slashes * 2, '\\');
  quoted.push_back('"');
  return quoted;
}

}

Env::Env(Lookup lookup) : lookup_(std::make_shared<const Lookup>(std::move(lookup))) {}

Env Env::system() {
  return Env(system_variable);
}

Env Env::from_pairs(std::vector<std::pair<std::string, std::string>> pairs) {
  std::map<std::string, std::string, std::less<>> values;
  for (auto& [name, value] : pairs) {
    values.insert_or_assign(std::move(name), std::move(value));
  }
  return Env([values = std::move(values)](std::string_view name) -> std::optional<std::string> {
    const auto found = values.find(name);
    if (found == values.end()) {
      return std::nullopt;
    }
    return found->second;
  });
}

std::optional<std::string> Env::get(std::string_view name) const {
  std::optional<std::string> value = (*lookup_)(name);
  if (!value || text::trim(*value).empty()) {
    return std::nullopt;
  }
  return value;
}

Os current_os() {
#if defined(_WIN32)
  return Os::Windows;
#elif defined(__APPLE__)
  return Os::Macos;
#else
  return Os::Linux;
#endif
}

std::string_view label(Os os) {
  switch (os) {
    case Os::Linux:
      return "linux";
    case Os::Macos:
      return "macos";
    case Os::Windows:
      return "windows";
  }
  return "linux";
}

fs::path data_dir(Os os, const Env& env) {
  switch (os) {
    case Os::Linux:
      if (auto base = env.get("XDG_DATA_HOME")) {
        return files::from_utf8(*base) / named(meta::slug);
      }
      return home(env) / ".local" / "share" / named(meta::slug);
    case Os::Macos:
      return home(env) / "Library" / "Application Support" / named(meta::app_name);
    case Os::Windows:
      if (auto base = env.get("LOCALAPPDATA")) {
        return files::from_utf8(*base) / named(meta::app_name);
      }
      return home(env) / "AppData" / "Local" / named(meta::app_name);
  }
  return home(env);
}

fs::path config_dir(Os os, const Env& env) {
  if (auto chosen = env.get(meta::env_name("config_dir"))) {
    return files::from_utf8(*chosen);
  }
  switch (os) {
    case Os::Linux:
      if (auto base = env.get("XDG_CONFIG_HOME")) {
        return files::from_utf8(*base) / named(meta::slug);
      }
      return home(env) / ".config" / named(meta::slug);
    case Os::Macos:
      return home(env) / "Library" / "Application Support" / named(meta::app_name);
    case Os::Windows:
      if (auto base = env.get("APPDATA")) {
        return files::from_utf8(*base) / named(meta::app_name);
      }
      return home(env) / "AppData" / "Roaming" / named(meta::app_name);
  }
  return home(env);
}

fs::path default_database_path(Os os, const Env& env) {
  return data_dir(os, env) / named(text::concat(meta::slug, ".db"));
}

std::pair<std::string, std::vector<std::string>> browser_command(Os os, std::string_view url) {
  switch (os) {
    case Os::Windows:
      return {"rundll32", {"url.dll,FileProtocolHandler", std::string(url)}};
    case Os::Macos:
      return {"open", {std::string(url)}};
    case Os::Linux:
      return {"xdg-open", {std::string(url)}};
  }
  return {"xdg-open", {std::string(url)}};
}

void open_repository_url(Os os, std::string_view url, const Runner& run) {
  if (!meta::is_repository_url(url)) {
    throw AppError::usage(
        text::concat("only links under ", meta::repository_url, " can be opened from the application"));
  }
  const auto [program, arguments] = browser_command(os, url);
  try {
    run(program, arguments);
  } catch (const AppError& error) {
    throw AppError::io(text::concat("the browser could not be started, copy the link instead: ", error.message()));
  }
}

std::vector<std::vector<std::string>> terminal_commands(Os os, std::string_view executable,
                                                        const std::vector<std::string>& arguments) {
  const std::string program(executable);
  if (os == Os::Windows) {
    std::vector<std::string> command = {"cmd", "/c", "start", "", program};
    command.insert(command.end(), arguments.begin(), arguments.end());
    return {command};
  }
  if (os == Os::Macos) {
    std::vector<std::string> words = {shell_quote(program)};
    for (const std::string& argument : arguments) {
      words.push_back(shell_quote(argument));
    }
    const std::string line = text::replace_all(text::replace_all(text::join(words, " "), "\\", "\\\\"), "\"", "\\\"");
    return {{"osascript", "-e", text::concat("tell application \"Terminal\" to do script \"", line, "\""), "-e",
             "tell application \"Terminal\" to activate"}};
  }
  static const std::array<std::vector<std::string>, 8> terminals = {{{"x-terminal-emulator", "-e"},
                                                                     {"gnome-terminal", "--"},
                                                                     {"konsole", "-e"},
                                                                     {"xfce4-terminal", "-x"},
                                                                     {"alacritty", "-e"},
                                                                     {"kitty"},
                                                                     {"foot"},
                                                                     {"xterm", "-e"}}};
  std::vector<std::vector<std::string>> commands;
  for (const std::vector<std::string>& terminal : terminals) {
    std::vector<std::string> command = terminal;
    command.push_back(program);
    command.insert(command.end(), arguments.begin(), arguments.end());
    commands.push_back(std::move(command));
  }
  return commands;
}

void open_terminal_window(Os os, std::string_view executable, const std::vector<std::string>& arguments,
                          const Runner& run) {
  std::vector<std::string> failures;
  for (const std::vector<std::string>& command : terminal_commands(os, executable, arguments)) {
    try {
      run(command.front(), {command.begin() + 1, command.end()});
      return;
    } catch (const AppError& error) {
      failures.push_back(error.message());
    }
  }
  throw AppError::io(
      text::concat("no terminal window could be opened, start the application from a terminal with --ui tui: ",
                   text::join(failures, "; ")));
}

std::string windows_command_line(const std::string& program, const std::vector<std::string>& arguments) {
  std::string line = windows_quote(program);
  for (const std::string& argument : arguments) {
    line.push_back(' ');
    line.append(windows_quote(argument));
  }
  return line;
}

void spawn_detached(const std::string& program, const std::vector<std::string>& arguments) {
#if defined(_WIN32)
  std::wstring line = widen(windows_command_line(program, arguments));
  STARTUPINFOW startup{};
  startup.cb = sizeof(startup);
  PROCESS_INFORMATION process{};
  const BOOL started =
      CreateProcessW(nullptr, line.data(), nullptr, nullptr, FALSE, DETACHED_PROCESS | CREATE_NEW_PROCESS_GROUP,
                     nullptr, nullptr, &startup, &process);
  if (started == 0) {
    throw AppError::io(text::concat(
        program, " could not be started: ", std::system_category().message(static_cast<int>(GetLastError()))));
  }
  CloseHandle(process.hThread);
  CloseHandle(process.hProcess);
#else
  std::vector<std::string> words = {program};
  words.insert(words.end(), arguments.begin(), arguments.end());
  std::vector<char*> pointers;
  pointers.reserve(words.size() + 1);
  for (std::string& word : words) {
    pointers.push_back(word.data());
  }
  pointers.push_back(nullptr);
  posix_spawn_file_actions_t actions;
  posix_spawn_file_actions_init(&actions);
  posix_spawn_file_actions_addopen(&actions, 0, "/dev/null", O_RDONLY, 0);
  posix_spawn_file_actions_addopen(&actions, 1, "/dev/null", O_WRONLY, 0);
  posix_spawn_file_actions_addopen(&actions, 2, "/dev/null", O_WRONLY, 0);
  pid_t child = 0;
  const int outcome = posix_spawnp(&child, program.c_str(), &actions, nullptr, pointers.data(), environ);
  posix_spawn_file_actions_destroy(&actions);
  if (outcome != 0) {
    throw AppError::io(text::concat(program, " could not be started: ", std::generic_category().message(outcome)));
  }
#endif
}

int run_quiet(const std::string& program, const std::vector<std::string>& arguments,
              const std::vector<std::pair<std::string, std::string>>& environment) {
#if defined(_WIN32)
  std::wstring block;
  LPWCH inherited = GetEnvironmentStringsW();
  if (inherited != nullptr) {
    for (const wchar_t* entry = inherited; *entry != L'\0';) {
      const std::wstring_view current(entry);
      const std::size_t split = current.find(L'=', 1);
      const std::wstring_view name = current.substr(0, split);
      bool replaced = false;
      for (const auto& [key, value] : environment) {
        replaced = replaced || CompareStringOrdinal(name.data(), static_cast<int>(name.size()), widen(key).c_str(), -1,
                                                    TRUE) == CSTR_EQUAL;
      }
      if (!replaced) {
        block.append(current);
        block.push_back(L'\0');
      }
      entry += current.size() + 1;
    }
    FreeEnvironmentStringsW(inherited);
  }
  for (const auto& [key, value] : environment) {
    block.append(widen(text::concat(key, "=", value)));
    block.push_back(L'\0');
  }
  block.push_back(L'\0');
  std::wstring line = widen(windows_command_line(program, arguments));
  STARTUPINFOW startup{};
  startup.cb = sizeof(startup);
  PROCESS_INFORMATION process{};
  const BOOL started =
      CreateProcessW(nullptr, line.data(), nullptr, nullptr, FALSE, CREATE_NO_WINDOW | CREATE_UNICODE_ENVIRONMENT,
                     block.data(), nullptr, &startup, &process);
  if (started == 0) {
    throw AppError::io(text::concat(
        program, " could not be started: ", std::system_category().message(static_cast<int>(GetLastError()))));
  }
  WaitForSingleObject(process.hProcess, INFINITE);
  DWORD code = 1;
  GetExitCodeProcess(process.hProcess, &code);
  CloseHandle(process.hThread);
  CloseHandle(process.hProcess);
  return static_cast<int>(code);
#else
  std::vector<std::string> words = {program};
  words.insert(words.end(), arguments.begin(), arguments.end());
  std::vector<char*> pointers;
  pointers.reserve(words.size() + 1);
  for (std::string& word : words) {
    pointers.push_back(word.data());
  }
  pointers.push_back(nullptr);
  std::vector<std::string> variables;
  for (char** entry = environ; entry != nullptr && *entry != nullptr; ++entry) {
    const std::string_view current(*entry);
    const std::string_view name = current.substr(0, current.find('='));
    bool replaced = false;
    for (const auto& [key, value] : environment) {
      replaced = replaced || name == key;
    }
    if (!replaced) {
      variables.emplace_back(current);
    }
  }
  for (const auto& [key, value] : environment) {
    variables.push_back(text::concat(key, "=", value));
  }
  std::vector<char*> block;
  block.reserve(variables.size() + 1);
  for (std::string& variable : variables) {
    block.push_back(variable.data());
  }
  block.push_back(nullptr);
  posix_spawn_file_actions_t actions;
  posix_spawn_file_actions_init(&actions);
  posix_spawn_file_actions_addopen(&actions, 0, "/dev/null", O_RDONLY, 0);
  posix_spawn_file_actions_addopen(&actions, 1, "/dev/null", O_WRONLY, 0);
  posix_spawn_file_actions_addopen(&actions, 2, "/dev/null", O_WRONLY, 0);
  pid_t child = 0;
  const int outcome = posix_spawnp(&child, program.c_str(), &actions, nullptr, pointers.data(), block.data());
  posix_spawn_file_actions_destroy(&actions);
  if (outcome != 0) {
    throw AppError::io(text::concat(program, " could not be started: ", std::generic_category().message(outcome)));
  }
  int status = 0;
  while (waitpid(child, &status, 0) < 0) {
    if (errno != EINTR) {
      throw AppError::io(text::concat(program, " could not be waited for: ", std::generic_category().message(errno)));
    }
  }
  if (WIFEXITED(status)) {
    return WEXITSTATUS(status);
  }
  return 128 + (WIFSIGNALED(status) ? WTERMSIG(status) : 0);
#endif
}

fs::path current_executable() {
#if defined(_WIN32)
  std::wstring buffer(32768, L'\0');
  const DWORD size = GetModuleFileNameW(nullptr, buffer.data(), static_cast<DWORD>(buffer.size()));
  buffer.resize(size);
  return {buffer};
#elif defined(__APPLE__)
  std::uint32_t size = 0;
  _NSGetExecutablePath(nullptr, &size);
  std::string buffer(size, '\0');
  if (_NSGetExecutablePath(buffer.data(), &size) != 0) {
    throw AppError::io("the path of the running program is unknown");
  }
  buffer.resize(std::char_traits<char>::length(buffer.c_str()));
  std::error_code problem;
  fs::path resolved = fs::canonical(buffer, problem);
  return problem ? fs::path(buffer) : resolved;
#else
  std::error_code problem;
  fs::path resolved = fs::read_symlink("/proc/self/exe", problem);
  if (problem) {
    throw AppError::io(text::concat("the path of the running program is unknown: ", problem.message()));
  }
  return resolved;
#endif
}

bool stdin_is_terminal() {
#if defined(_WIN32)
  return _isatty(_fileno(stdin)) != 0;
#else
  return ::isatty(STDIN_FILENO) != 0;
#endif
}

bool stdout_is_terminal() {
#if defined(_WIN32)
  return _isatty(_fileno(stdout)) != 0;
#else
  return ::isatty(STDOUT_FILENO) != 0;
#endif
}

bool stderr_is_terminal() {
#if defined(_WIN32)
  return _isatty(_fileno(stderr)) != 0;
#else
  return ::isatty(STDERR_FILENO) != 0;
#endif
}

unsigned console_owners() {
#if defined(_WIN32)
  std::array<DWORD, 4> processes{};
  return static_cast<unsigned>(GetConsoleProcessList(processes.data(), static_cast<DWORD>(processes.size())));
#else
  return stdin_is_terminal() || stdout_is_terminal() || stderr_is_terminal() ? 2U : 1U;
#endif
}

std::vector<std::string> process_arguments(int argc, char** argv) {
  std::vector<std::string> arguments;
#if defined(_WIN32)
  static_cast<void>(argv);
  int count = 0;
  LPWSTR* wide = CommandLineToArgvW(GetCommandLineW(), &count);
  if (wide != nullptr) {
    for (int index = 1; index < count; ++index) {
      arguments.push_back(narrow(wide[index]));
    }
    LocalFree(wide);
    return arguments;
  }
  static_cast<void>(argc);
#else
  for (int index = 1; index < argc; ++index) {
    arguments.emplace_back(argv[index]);
  }
#endif
  return arguments;
}

void prepare_console() {
#if defined(_WIN32)
  SetConsoleOutputCP(CP_UTF8);
  SetConsoleCP(CP_UTF8);
#endif
}

void release_console() {
#if defined(_WIN32)
  if (console_owners() <= 1) {
    FreeConsole();
  }
#endif
}

}
