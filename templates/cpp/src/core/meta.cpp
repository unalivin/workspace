#include "core/meta.hpp"

#include <algorithm>
#include <array>

#include "core/build_config.hpp"
#include "core/embedded.hpp"
#include "core/text.hpp"

namespace app::meta {

namespace {

std::string_view current_os() {
#if defined(_WIN32)
  return "windows";
#elif defined(__APPLE__)
  return "macos";
#else
  return "linux";
#endif
}

std::string_view current_arch() {
#if defined(__x86_64__) || defined(_M_X64)
  return "x86_64";
#elif defined(__aarch64__) || defined(_M_ARM64)
  return "aarch64";
#elif defined(__i386__) || defined(_M_IX86)
  return "x86";
#else
  return "unknown";
#endif
}

std::string_view current_profile() {
#if defined(NDEBUG)
  return "release";
#else
  return "debug";
#endif
}

}

std::string_view version() {
  return build_config::version;
}

std::string_view license_text() {
  return embedded::license();
}

std::string_view notices_text() {
  return embedded::notices();
}

std::string_view usage_text() {
  return embedded::usage();
}

BuildInfo build(bool dashboard) {
  return {std::string(version()),         std::string(current_os()),           std::string(current_arch()),
          std::string(current_profile()), std::string(build_config::compiler), dashboard};
}

std::string env_name(std::string_view suffix) {
  std::string converted = text::upper(suffix);
  std::replace(converted.begin(), converted.end(), '.', '_');
  std::replace(converted.begin(), converted.end(), '-', '_');
  return text::concat(env_prefix, "_", converted);
}

std::string source_url() {
  return std::string(repository_url);
}

std::string advisory_url() {
  return text::concat(repository_url, "/security/advisories/new");
}

std::string encode_query(std::string_view text) {
  static constexpr std::string_view digits = "0123456789ABCDEF";
  std::string encoded;
  encoded.reserve(text.size());
  for (const char letter : text) {
    const auto byte = static_cast<unsigned char>(letter);
    const bool plain = (byte >= 'a' && byte <= 'z') || (byte >= 'A' && byte <= 'Z') || (byte >= '0' && byte <= '9') ||
                       byte == '-' || byte == '_' || byte == '.' || byte == '~';
    if (plain) {
      encoded.push_back(letter);
    } else {
      encoded.push_back('%');
      encoded.push_back(digits[byte >> 4U]);
      encoded.push_back(digits[byte & 0x0FU]);
    }
  }
  return encoded;
}

std::string new_issue_url(std::string_view title, std::string_view body) {
  return text::concat(repository_url, "/issues/new?title=", encode_query(title), "&body=", encode_query(body));
}

bool is_repository_url(std::string_view candidate) {
  if (!candidate.starts_with(repository_url)) {
    return false;
  }
  const std::string_view rest = candidate.substr(repository_url.size());
  if (!rest.empty() && rest.front() != '/' && rest.front() != '?') {
    return false;
  }
  const bool printable = std::all_of(candidate.begin(), candidate.end(), [](char letter) {
    const auto byte = static_cast<unsigned char>(letter);
    return byte > 0x20 && byte < 0x7F;
  });
  if (!printable) {
    return false;
  }
  const std::string_view path = rest.substr(0, rest.find_first_of("?#"));
  const std::string lowered = text::lower(path);
  static constexpr std::array<std::string_view, 3> plain = {"..", "//", "\\"};
  static constexpr std::array<std::string_view, 3> encoded = {"%2f", "%5c", "%2e"};
  const bool has_plain =
      std::any_of(plain.begin(), plain.end(), [&](std::string_view part) { return text::contains(path, part); });
  const bool has_encoded =
      std::any_of(encoded.begin(), encoded.end(), [&](std::string_view part) { return text::contains(lowered, part); });
  return !has_plain && !has_encoded;
}

}
