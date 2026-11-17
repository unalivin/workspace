#pragma once

#include <cstddef>
#include <cstdint>
#include <filesystem>
#include <functional>
#include <string>
#include <string_view>
#include <utility>
#include <vector>

#include "core/clock.hpp"
#include "core/platform.hpp"

namespace app::notifier {

inline constexpr std::int64_t min_interval_seconds = 15;
inline constexpr std::int64_t max_interval_seconds = 3600;
inline constexpr std::string_view state_file_name = "notifier-state.json";

[[nodiscard]] std::int64_t clamp_interval(std::int64_t seconds);

using Deliver = std::function<void(std::string_view title, std::string_view body)>;

struct Runner {
  std::filesystem::path database;
  std::filesystem::path state_dir;
  bool show_titles = false;
  clock::Clock clock = clock::Clock::system();
  Deliver deliver;

  [[nodiscard]] std::size_t tick() const;
  void run(std::int64_t interval_seconds, std::size_t max_ticks, const std::function<bool(std::int64_t)>& wait,
           const std::function<void(std::string_view)>& log) const;
};

struct DeliveryCommand {
  std::string program;
  std::vector<std::string> arguments;
  std::vector<std::pair<std::string, std::string>> environment;
  bool operator==(const DeliveryCommand&) const = default;
};

[[nodiscard]] DeliveryCommand delivery_command(platform::Os os, std::string_view title, std::string_view body);
void deliver_with_system(platform::Os os, std::string_view title, std::string_view body);

struct Placement {
  bool present = false;
  std::filesystem::path path;
  bool operator==(const Placement&) const = default;
};

struct Autostart {
  platform::Os os = platform::current_os();
  std::string home_dir;
  std::string config_home;
  std::string app_data;
  std::string executable;

  [[nodiscard]] static Autostart for_system(platform::Os os, const platform::Env& env,
                                            const std::filesystem::path& executable);
  [[nodiscard]] std::filesystem::path path() const;
  [[nodiscard]] std::string content() const;
  std::filesystem::path install(bool dry_run) const;
  Placement uninstall(bool dry_run) const;
  [[nodiscard]] Placement status() const;
  bool operator==(const Autostart&) const = default;
};

}
