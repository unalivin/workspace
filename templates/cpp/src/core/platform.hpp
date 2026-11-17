#pragma once

#include <filesystem>
#include <functional>
#include <map>
#include <memory>
#include <optional>
#include <string>
#include <string_view>
#include <utility>
#include <vector>

namespace app::platform {

class Env {
 public:
  using Lookup = std::function<std::optional<std::string>(std::string_view)>;

  [[nodiscard]] static Env system();
  [[nodiscard]] static Env from_pairs(std::vector<std::pair<std::string, std::string>> pairs);

  [[nodiscard]] std::optional<std::string> get(std::string_view name) const;

 private:
  explicit Env(Lookup lookup);
  std::shared_ptr<const Lookup> lookup_;
};

enum class Os { Linux, Macos, Windows };

[[nodiscard]] Os current_os();
[[nodiscard]] std::string_view label(Os os);

using Runner = std::function<void(const std::string&, const std::vector<std::string>&)>;

[[nodiscard]] std::filesystem::path data_dir(Os os, const Env& env);
[[nodiscard]] std::filesystem::path config_dir(Os os, const Env& env);
[[nodiscard]] std::filesystem::path default_database_path(Os os, const Env& env);
[[nodiscard]] std::pair<std::string, std::vector<std::string>> browser_command(Os os, std::string_view url);
void open_repository_url(Os os, std::string_view url, const Runner& run);
[[nodiscard]] std::vector<std::vector<std::string>> terminal_commands(Os os, std::string_view executable,
                                                                      const std::vector<std::string>& arguments);
void open_terminal_window(Os os, std::string_view executable, const std::vector<std::string>& arguments,
                          const Runner& run);
void spawn_detached(const std::string& program, const std::vector<std::string>& arguments);
[[nodiscard]] int run_quiet(const std::string& program, const std::vector<std::string>& arguments,
                            const std::vector<std::pair<std::string, std::string>>& environment);
[[nodiscard]] std::string windows_command_line(const std::string& program, const std::vector<std::string>& arguments);
[[nodiscard]] std::filesystem::path current_executable();
[[nodiscard]] bool stdin_is_terminal();
[[nodiscard]] bool stdout_is_terminal();
[[nodiscard]] bool stderr_is_terminal();
[[nodiscard]] unsigned console_owners();
[[nodiscard]] std::vector<std::string> process_arguments(int argc, char** argv);
void prepare_console();
void release_console();

}
