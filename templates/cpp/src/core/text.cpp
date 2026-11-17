#include "core/text.hpp"

#include <algorithm>
#include <charconv>
#include <utility>

namespace app::text {

namespace {

bool is_space(char letter) {
  return letter == ' ' || letter == '\t' || letter == '\n' || letter == '\r' || letter == '\f' || letter == '\v';
}

char lower_letter(char letter) {
  return letter >= 'A' && letter <= 'Z' ? static_cast<char>(letter - 'A' + 'a') : letter;
}

char upper_letter(char letter) {
  return letter >= 'a' && letter <= 'z' ? static_cast<char>(letter - 'a' + 'A') : letter;
}

std::size_t sequence_length(unsigned char lead) {
  if (lead < 0x80) {
    return 1;
  }
  if (lead >= 0xC2 && lead <= 0xDF) {
    return 2;
  }
  if (lead >= 0xE0 && lead <= 0xEF) {
    return 3;
  }
  if (lead >= 0xF0 && lead <= 0xF4) {
    return 4;
  }
  return 0;
}

}

std::string_view trim(std::string_view value) {
  while (!value.empty() && is_space(value.front())) {
    value.remove_prefix(1);
  }
  while (!value.empty() && is_space(value.back())) {
    value.remove_suffix(1);
  }
  return value;
}

std::string lower(std::string_view value) {
  std::string out(value);
  std::transform(out.begin(), out.end(), out.begin(), lower_letter);
  return out;
}

std::string upper(std::string_view value) {
  std::string out(value);
  std::transform(out.begin(), out.end(), out.begin(), upper_letter);
  return out;
}

bool equals_ignoring_case(std::string_view left, std::string_view right) {
  return left.size() == right.size() && std::equal(left.begin(), left.end(), right.begin(), [](char one, char other) {
           return lower_letter(one) == lower_letter(other);
         });
}

bool contains(std::string_view value, std::string_view part) {
  return value.find(part) != std::string_view::npos;
}

std::vector<std::string> split(std::string_view value, char separator) {
  std::vector<std::string> parts;
  std::size_t start = 0;
  while (true) {
    const std::size_t end = value.find(separator, start);
    if (end == std::string_view::npos) {
      parts.emplace_back(value.substr(start));
      return parts;
    }
    parts.emplace_back(value.substr(start, end - start));
    start = end + 1;
  }
}

std::vector<std::string> lines(std::string_view value) {
  std::vector<std::string> out;
  if (value.empty()) {
    return out;
  }
  if (value.back() == '\n') {
    value.remove_suffix(1);
  }
  for (std::string& line : split(value, '\n')) {
    if (!line.empty() && line.back() == '\r') {
      line.pop_back();
    }
    out.push_back(std::move(line));
  }
  return out;
}

std::string join(const std::vector<std::string>& parts, std::string_view separator) {
  std::string out;
  for (std::size_t index = 0; index < parts.size(); ++index) {
    if (index > 0) {
      out.append(separator);
    }
    out.append(parts[index]);
  }
  return out;
}

std::string replace_all(std::string_view value, std::string_view from, std::string_view to) {
  if (from.empty()) {
    return std::string(value);
  }
  std::string out;
  std::size_t start = 0;
  while (true) {
    const std::size_t found = value.find(from, start);
    if (found == std::string_view::npos) {
      out.append(value.substr(start));
      return out;
    }
    out.append(value.substr(start, found - start));
    out.append(to);
    start = found + from.size();
  }
}

std::string padded(std::int64_t number, int width) {
  std::string digits = std::to_string(number < 0 ? -number : number);
  const auto wanted = static_cast<std::size_t>(std::max(width, 0));
  if (digits.size() < wanted) {
    digits.insert(0, wanted - digits.size(), '0');
  }
  return number < 0 ? "-" + digits : digits;
}

std::optional<std::int64_t> parse_integer(std::string_view value) {
  std::int64_t number = 0;
  const char* first = value.data();
  const char* last = value.data() + value.size();
  if (first == last || *first == '+') {
    return std::nullopt;
  }
  const auto result = std::from_chars(first, last, number);
  if (result.ec != std::errc() || result.ptr != last) {
    return std::nullopt;
  }
  return number;
}

bool is_digits(std::string_view value) {
  return !value.empty() &&
         std::all_of(value.begin(), value.end(), [](char letter) { return letter >= '0' && letter <= '9'; });
}

bool is_valid_utf8(std::string_view value) {
  std::size_t index = 0;
  while (index < value.size()) {
    const auto lead = static_cast<unsigned char>(value[index]);
    const std::size_t size = sequence_length(lead);
    if (size == 0 || index + size > value.size()) {
      return false;
    }
    for (std::size_t offset = 1; offset < size; ++offset) {
      if ((static_cast<unsigned char>(value[index + offset]) & 0xC0U) != 0x80U) {
        return false;
      }
    }
    if (size >= 3) {
      const auto second = static_cast<unsigned char>(value[index + 1]);
      const bool overlong = (lead == 0xE0 && second < 0xA0) || (lead == 0xF0 && second < 0x90);
      const bool surrogate = lead == 0xED && second >= 0xA0;
      const bool beyond = lead == 0xF4 && second >= 0x90;
      if (overlong || surrogate || beyond) {
        return false;
      }
    }
    index += size;
  }
  return true;
}

std::size_t length(std::string_view utf8) {
  return static_cast<std::size_t>(std::count_if(
      utf8.begin(), utf8.end(), [](char letter) { return (static_cast<unsigned char>(letter) & 0xC0U) != 0x80U; }));
}

std::string truncated(std::string_view utf8, std::size_t limit) {
  std::size_t seen = 0;
  std::size_t index = 0;
  while (index < utf8.size()) {
    if ((static_cast<unsigned char>(utf8[index]) & 0xC0U) != 0x80U) {
      if (seen == limit) {
        break;
      }
      ++seen;
    }
    ++index;
  }
  return std::string(utf8.substr(0, index));
}

std::string plural(std::int64_t count, std::string_view one, std::string_view many) {
  return std::to_string(count) + " " + std::string(count == 1 ? one : many);
}

std::u32string decode(std::string_view utf8) {
  std::u32string out;
  out.reserve(utf8.size());
  std::size_t index = 0;
  while (index < utf8.size()) {
    const auto lead = static_cast<unsigned char>(utf8[index]);
    const std::size_t size = sequence_length(lead);
    if (size == 0 || index + size > utf8.size() || !is_valid_utf8(utf8.substr(index, size))) {
      out.push_back(U'\uFFFD');
      ++index;
      continue;
    }
    char32_t letter = size == 1 ? lead : static_cast<char32_t>(lead & (0xFFU >> (size + 1)));
    for (std::size_t offset = 1; offset < size; ++offset) {
      letter = (letter << 6U) | (static_cast<unsigned char>(utf8[index + offset]) & 0x3FU);
    }
    out.push_back(letter);
    index += size;
  }
  return out;
}

void append(std::string& out, char32_t letter) {
  const auto value = static_cast<std::uint32_t>(letter);
  if (value < 0x80U) {
    out.push_back(static_cast<char>(value));
  } else if (value < 0x800U) {
    out.push_back(static_cast<char>(0xC0U | (value >> 6U)));
    out.push_back(static_cast<char>(0x80U | (value & 0x3FU)));
  } else if (value < 0x10000U) {
    out.push_back(static_cast<char>(0xE0U | (value >> 12U)));
    out.push_back(static_cast<char>(0x80U | ((value >> 6U) & 0x3FU)));
    out.push_back(static_cast<char>(0x80U | (value & 0x3FU)));
  } else {
    out.push_back(static_cast<char>(0xF0U | (value >> 18U)));
    out.push_back(static_cast<char>(0x80U | ((value >> 12U) & 0x3FU)));
    out.push_back(static_cast<char>(0x80U | ((value >> 6U) & 0x3FU)));
    out.push_back(static_cast<char>(0x80U | (value & 0x3FU)));
  }
}

std::string encode(std::u32string_view letters) {
  std::string out;
  out.reserve(letters.size());
  for (const char32_t letter : letters) {
    append(out, letter);
  }
  return out;
}

bool is_control(char32_t letter) {
  return letter < 0x20 || (letter >= 0x7F && letter < 0xA0);
}

bool is_whitespace(char32_t letter) {
  return (letter >= 0x09 && letter <= 0x0D) || letter == 0x20 || letter == 0x85 || letter == 0xA0 || letter == 0x1680 ||
         (letter >= 0x2000 && letter <= 0x200A) || letter == 0x2028 || letter == 0x2029 || letter == 0x202F ||
         letter == 0x205F || letter == 0x3000;
}

bool is_word(char32_t letter) {
  if (letter < 0x80) {
    return (letter >= U'0' && letter <= U'9') || (letter >= U'a' && letter <= U'z') ||
           (letter >= U'A' && letter <= U'Z');
  }
  if (is_control(letter) || is_whitespace(letter) || letter == 0xFFFD) {
    return false;
  }
  if (letter <= 0xBF) {
    return letter == 0xAA || letter == 0xB2 || letter == 0xB3 || letter == 0xB5 || letter == 0xB9 || letter == 0xBA ||
           (letter >= 0xBC && letter <= 0xBE);
  }
  const bool symbol = letter == 0xD7 || letter == 0xF7 || (letter >= 0x2000 && letter <= 0x206F) ||
                      (letter >= 0x20A0 && letter <= 0x20CF) || (letter >= 0x2190 && letter <= 0x245F) ||
                      (letter >= 0x2500 && letter <= 0x2BFF) || (letter >= 0x2E00 && letter <= 0x2E7F) ||
                      (letter >= 0x3000 && letter <= 0x3004) || (letter >= 0x3008 && letter <= 0x3020) ||
                      (letter >= 0xE000 && letter <= 0xF8FF) || (letter >= 0xFE10 && letter <= 0xFE6F) ||
                      (letter >= 0xFF01 && letter <= 0xFF0F) || (letter >= 0xFF1A && letter <= 0xFF20) ||
                      (letter >= 0xFF3B && letter <= 0xFF40) || (letter >= 0xFF5B && letter <= 0xFF65) ||
                      (letter >= 0x1F000 && letter <= 0x1FAFF);
  return !symbol;
}

std::vector<std::string> words(std::string_view utf8) {
  std::vector<std::string> out;
  std::string current;
  for (const char32_t letter : decode(utf8)) {
    if (is_whitespace(letter)) {
      if (!current.empty()) {
        out.push_back(std::exchange(current, {}));
      }
    } else {
      append(current, letter);
    }
  }
  if (!current.empty()) {
    out.push_back(std::move(current));
  }
  return out;
}

std::string collapse_whitespace(std::string_view utf8) {
  return join(words(utf8), " ");
}

std::string_view trim_whitespace(std::string_view utf8) {
  const auto leading = [](std::string_view value) -> std::size_t {
    for (const std::string_view space : {"\xC2\x85", "\xC2\xA0", "\xE1\x9A\x80", "\xE2\x80\xA8", "\xE2\x80\xA9",
                                         "\xE2\x80\xAF", "\xE2\x81\x9F", "\xE3\x80\x80"}) {
      if (value.starts_with(space)) {
        return space.size();
      }
    }
    if (value.size() >= 3 && value[0] == '\xE2' && value[1] == '\x80' && static_cast<unsigned char>(value[2]) >= 0x80 &&
        static_cast<unsigned char>(value[2]) <= 0x8A) {
      return 3;
    }
    if (!value.empty() && ((value[0] >= '\x09' && value[0] <= '\x0D') || value[0] == ' ')) {
      return 1;
    }
    return 0;
  };
  while (true) {
    const std::size_t size = leading(utf8);
    if (size == 0) {
      break;
    }
    utf8.remove_prefix(size);
  }
  while (!utf8.empty()) {
    std::size_t start = utf8.size() - 1;
    while (start > 0 && (static_cast<unsigned char>(utf8[start]) & 0xC0U) == 0x80U) {
      --start;
    }
    if (leading(utf8.substr(start)) != utf8.size() - start) {
      break;
    }
    utf8.remove_suffix(utf8.size() - start);
  }
  return utf8;
}

}
