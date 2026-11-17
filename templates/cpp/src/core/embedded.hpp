#pragma once

#include <cstddef>
#include <string_view>
#include <vector>

namespace app::embedded {

struct File {
  std::string_view name;
  std::string_view content;
};

[[nodiscard]] std::string_view license();
[[nodiscard]] std::string_view notices();
[[nodiscard]] std::string_view usage();
[[nodiscard]] std::vector<File> migrations();

}
