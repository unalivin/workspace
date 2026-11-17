#pragma once

#include <cstdint>
#include <string>
#include <string_view>
#include <vector>

namespace testkit {

class Random {
 public:
  explicit Random(std::uint64_t seed) : state_(seed == 0 ? 0x9E3779B97F4A7C15ULL : seed) {}

  [[nodiscard]] std::uint64_t next() {
    state_ ^= state_ << 13U;
    state_ ^= state_ >> 7U;
    state_ ^= state_ << 17U;
    return state_;
  }

  [[nodiscard]] std::int64_t between(std::int64_t least, std::int64_t most) {
    const auto span = static_cast<std::uint64_t>(most - least) + 1U;
    return least + static_cast<std::int64_t>(next() % span);
  }

  [[nodiscard]] std::string text(std::size_t longest, std::string_view alphabet) {
    const auto size = static_cast<std::size_t>(between(0, static_cast<std::int64_t>(longest)));
    std::string out;
    for (std::size_t index = 0; index < size; ++index) {
      out.push_back(alphabet[static_cast<std::size_t>(next() % alphabet.size())]);
    }
    return out;
  }

  [[nodiscard]] std::string pieces(std::size_t longest, const std::vector<std::string_view>& alphabet) {
    const auto size = static_cast<std::size_t>(between(0, static_cast<std::int64_t>(longest)));
    std::string out;
    for (std::size_t index = 0; index < size; ++index) {
      out.append(alphabet[static_cast<std::size_t>(next() % alphabet.size())]);
    }
    return out;
  }

  [[nodiscard]] std::string bytes(std::size_t longest) {
    const auto size = static_cast<std::size_t>(between(0, static_cast<std::int64_t>(longest)));
    std::string out;
    for (std::size_t index = 0; index < size; ++index) {
      out.push_back(static_cast<char>(next() % 256U));
    }
    return out;
  }

 private:
  std::uint64_t state_;
};

}
