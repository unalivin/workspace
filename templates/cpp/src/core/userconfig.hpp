#pragma once

#include <cstdint>
#include <filesystem>
#include <string>
#include <string_view>

namespace app::userconfig {

inline constexpr std::string_view front_end_gui = "gui";
inline constexpr std::string_view front_end_tui = "tui";
inline constexpr std::uint64_t max_file_size = 64 * 1024;

struct Config {
  std::string database_path;
  std::string front_end;

  void validate() const;
  bool operator==(const Config&) const = default;
};

[[nodiscard]] std::filesystem::path file(const std::filesystem::path& directory);
[[nodiscard]] Config load(const std::filesystem::path& directory);
void save(const std::filesystem::path& directory, const Config& config);

}
