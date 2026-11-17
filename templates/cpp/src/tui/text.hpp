#pragma once

#include <cstddef>
#include <string>
#include <string_view>
#include <utility>
#include <vector>

#include "core/theme.hpp"

namespace app::tui {

enum class Look {
  Plain,
  Muted,
  Accent,
  Heading,
  Selected,
  Border,
  Status,
  Success,
  Warning,
  Failure,
  Info,
  Low,
  Medium,
  High,
  Critical
};

[[nodiscard]] Look look_for(theme::Severity level);

struct Span {
  std::string text;
  Look look = Look::Plain;
  bool operator==(const Span&) const = default;
};

[[nodiscard]] std::size_t width(std::string_view text);
[[nodiscard]] std::size_t letter_width(char32_t letter);
[[nodiscard]] std::string clean(std::string_view text);
[[nodiscard]] std::string clip(std::string_view text, std::size_t limit);
[[nodiscard]] std::string pad(std::string_view text, std::size_t size);
[[nodiscard]] std::string repeat(std::string_view text, std::size_t count);
[[nodiscard]] std::vector<std::string> wrap(std::string_view text, std::size_t limit);
[[nodiscard]] std::pair<std::size_t, std::size_t> window(std::size_t total, std::size_t cursor, std::size_t height);

struct Line {
  std::vector<Span> spans;

  [[nodiscard]] static Line plain(std::string text);
  [[nodiscard]] static Line styled(std::string text, Look look);
  Line& add(std::string text, Look look);
  [[nodiscard]] Line with(std::string text, Look look) &&;
  [[nodiscard]] std::size_t width() const;
  [[nodiscard]] std::string text() const;
  [[nodiscard]] Line recolor(Look look) &&;
  [[nodiscard]] Line fit(std::size_t size) const;
  bool operator==(const Line&) const = default;
};

using Lines = std::vector<Line>;

[[nodiscard]] Lines wrapped(std::string_view text, std::size_t limit, Look look);
void extend(Lines& into, Lines more);

}
