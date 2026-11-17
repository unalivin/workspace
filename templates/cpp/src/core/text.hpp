#pragma once

#include <cstdint>
#include <optional>
#include <string>
#include <string_view>
#include <type_traits>
#include <vector>

namespace app::text {

[[nodiscard]] std::string_view trim(std::string_view value);
[[nodiscard]] std::string lower(std::string_view value);
[[nodiscard]] std::string upper(std::string_view value);
[[nodiscard]] bool equals_ignoring_case(std::string_view left, std::string_view right);
[[nodiscard]] bool contains(std::string_view value, std::string_view part);
[[nodiscard]] std::vector<std::string> split(std::string_view value, char separator);
[[nodiscard]] std::vector<std::string> lines(std::string_view value);
[[nodiscard]] std::string join(const std::vector<std::string>& parts, std::string_view separator);
[[nodiscard]] std::string replace_all(std::string_view value, std::string_view from, std::string_view to);
[[nodiscard]] std::string padded(std::int64_t number, int width);
[[nodiscard]] std::optional<std::int64_t> parse_integer(std::string_view value);
[[nodiscard]] bool is_digits(std::string_view value);
[[nodiscard]] std::size_t length(std::string_view utf8);
[[nodiscard]] bool is_valid_utf8(std::string_view value);
[[nodiscard]] std::string truncated(std::string_view utf8, std::size_t limit);
[[nodiscard]] std::u32string decode(std::string_view utf8);
[[nodiscard]] std::string encode(std::u32string_view letters);
void append(std::string& out, char32_t letter);
[[nodiscard]] bool is_control(char32_t letter);
[[nodiscard]] bool is_whitespace(char32_t letter);
[[nodiscard]] bool is_word(char32_t letter);
[[nodiscard]] std::string_view trim_whitespace(std::string_view utf8);
[[nodiscard]] std::vector<std::string> words(std::string_view utf8);
[[nodiscard]] std::string collapse_whitespace(std::string_view utf8);
[[nodiscard]] std::string plural(std::int64_t count, std::string_view one, std::string_view many);

template <typename... Parts>
[[nodiscard]] std::string concat(const Parts&... parts) {
  std::string out;
  const auto add = [&out](const auto& part) {
    using Part = std::decay_t<decltype(part)>;
    if constexpr (std::is_same_v<Part, char>) {
      out.push_back(part);
    } else if constexpr (std::is_same_v<Part, bool>) {
      out.append(part ? "true" : "false");
    } else if constexpr (std::is_arithmetic_v<Part>) {
      out.append(std::to_string(part));
    } else {
      out.append(std::string_view(part));
    }
  };
  (add(parts), ...);
  return out;
}

}
