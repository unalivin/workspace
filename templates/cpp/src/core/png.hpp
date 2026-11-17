#pragma once

#include <cstddef>
#include <cstdint>
#include <span>
#include <string_view>
#include <vector>

namespace app::png {

using Bytes = std::vector<std::uint8_t>;

inline constexpr std::size_t max_dimension = 16384;

struct Picture {
  std::size_t width = 0;
  std::size_t height = 0;
  Bytes rgba;
  bool operator==(const Picture&) const = default;
};

[[nodiscard]] std::uint32_t crc32(std::span<const std::uint8_t> first, std::span<const std::uint8_t> second = {});
[[nodiscard]] std::uint32_t adler32(std::span<const std::uint8_t> data);
[[nodiscard]] Bytes deflate(std::span<const std::uint8_t> data);
[[nodiscard]] Bytes inflate(std::span<const std::uint8_t> data, std::size_t limit);
[[nodiscard]] Bytes encode(std::size_t width, std::size_t height, std::span<const std::uint8_t> rgba);
[[nodiscard]] Picture decode(std::span<const std::uint8_t> data);
[[nodiscard]] Bytes bytes_of(std::string_view text);

}
