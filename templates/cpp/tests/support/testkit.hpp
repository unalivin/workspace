#pragma once

#include <filesystem>
#include <functional>
#include <string>
#include <string_view>

#include "core/clock.hpp"

namespace testkit {

class TempDir {
 public:
  explicit TempDir(std::string_view label);
  TempDir(const TempDir&) = delete;
  TempDir& operator=(const TempDir&) = delete;
  ~TempDir();

  [[nodiscard]] const std::filesystem::path& path() const;

 private:
  std::filesystem::path path_;
};

[[nodiscard]] app::clock::Timestamp fixed_time();
[[nodiscard]] std::filesystem::path source_root();
[[nodiscard]] std::string failure_of(const std::function<void()>& action);

}
