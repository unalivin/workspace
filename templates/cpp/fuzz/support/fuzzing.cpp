#include "support/fuzzing.hpp"

#include <cstdio>
#include <cstdlib>
#include <string>

#include "core/text.hpp"

namespace fuzzing {

void fail(std::string_view target, std::string_view message, std::string_view detail) {
  const std::string line = app::text::concat(target, ": ", message, detail.empty() ? "" : ": ", detail, "\n");
  std::fwrite(line.data(), 1, line.size(), stderr);
  std::fflush(stderr);
  std::abort();
}

void require(bool holds, std::string_view target, std::string_view message, std::string_view detail) {
  if (!holds) {
    fail(target, message, detail);
  }
}

std::string_view bytes(const std::uint8_t* data, std::size_t size) {
  if (data == nullptr || size == 0) {
    return {};
  }
  return {reinterpret_cast<const char*>(data), size};
}

std::optional<std::string_view> text(const std::uint8_t* data, std::size_t size) {
  const std::string_view raw = bytes(data, size);
  if (!app::text::is_valid_utf8(raw)) {
    return std::nullopt;
  }
  return raw;
}

}
