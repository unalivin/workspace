#include "gui/appearance.hpp"

#include <array>
#include <string>
#include <vector>

#include "core/text.hpp"

#if defined(_WIN32)
#include <windows.h>
#else
#include <fcntl.h>
#include <spawn.h>
#include <sys/wait.h>
#include <unistd.h>

#include <cerrno>

extern char** environ;
#endif

namespace app::gui {

namespace {

#if !defined(_WIN32)
std::optional<std::string> answer_of(const std::vector<std::string>& command) {
  std::array<int, 2> ends{};
  if (pipe(ends.data()) != 0) {
    return std::nullopt;
  }
  std::vector<std::string> words = command;
  std::vector<char*> pointers;
  pointers.reserve(words.size() + 1);
  for (std::string& word : words) {
    pointers.push_back(word.data());
  }
  pointers.push_back(nullptr);
  posix_spawn_file_actions_t actions;
  posix_spawn_file_actions_init(&actions);
  posix_spawn_file_actions_addopen(&actions, 0, "/dev/null", O_RDONLY, 0);
  posix_spawn_file_actions_adddup2(&actions, ends[1], 1);
  posix_spawn_file_actions_addopen(&actions, 2, "/dev/null", O_WRONLY, 0);
  posix_spawn_file_actions_addclose(&actions, ends[0]);
  posix_spawn_file_actions_addclose(&actions, ends[1]);
  pid_t child = 0;
  const int outcome = posix_spawnp(&child, pointers.front(), &actions, nullptr, pointers.data(), environ);
  posix_spawn_file_actions_destroy(&actions);
  close(ends[1]);
  if (outcome != 0) {
    close(ends[0]);
    return std::nullopt;
  }
  std::string collected;
  std::array<char, 256> chunk{};
  while (collected.size() < 4096) {
    const ssize_t count = read(ends[0], chunk.data(), chunk.size());
    if (count < 0 && errno == EINTR) {
      continue;
    }
    if (count <= 0) {
      break;
    }
    collected.append(chunk.data(), static_cast<std::size_t>(count));
  }
  close(ends[0]);
  int status = 0;
  while (waitpid(child, &status, 0) < 0) {
    if (errno != EINTR) {
      return std::nullopt;
    }
  }
  if (!WIFEXITED(status) || WEXITSTATUS(status) != 0) {
    return std::nullopt;
  }
  return collected;
}
#endif

}

bool scheme_is_dark(std::string_view report) {
  return text::contains(text::lower(report), "dark");
}

bool theme_is_dark(std::string_view name) {
  const std::string lowered = text::lower(text::trim(name));
  return lowered.ends_with(":dark") || lowered.ends_with("-dark");
}

std::optional<bool> system_dark(const platform::Env& env) {
#if defined(_WIN32)
  static_cast<void>(env);
  DWORD light = 1;
  DWORD size = sizeof(light);
  const LSTATUS status =
      RegGetValueW(HKEY_CURRENT_USER, L"Software\\Microsoft\\Windows\\CurrentVersion\\Themes\\Personalize",
                   L"AppsUseLightTheme", RRF_RT_REG_DWORD, nullptr, &light, &size);
  if (status != ERROR_SUCCESS) {
    return std::nullopt;
  }
  return light == 0;
#elif defined(__APPLE__)
  static_cast<void>(env);
  const std::optional<std::string> style = answer_of({"defaults", "read", "-g", "AppleInterfaceStyle"});
  return style && scheme_is_dark(*style);
#else
  if (const std::optional<std::string> theme = env.get("GTK_THEME"); theme && theme_is_dark(*theme)) {
    return true;
  }
  const std::optional<std::string> scheme =
      answer_of({"gsettings", "get", "org.gnome.desktop.interface", "color-scheme"});
  if (!scheme) {
    return std::nullopt;
  }
  return scheme_is_dark(*scheme);
#endif
}

}
