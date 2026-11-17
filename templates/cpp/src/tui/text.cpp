#include "tui/text.hpp"

#include <ftxui/screen/string.hpp>

#include <algorithm>
#include <iterator>

#include "core/text.hpp"

namespace app::tui {

namespace {

constexpr std::size_t min_wrap_width = 8;

}

Look look_for(theme::Severity level) {
  switch (level) {
    case theme::Severity::Info:
      return Look::Info;
    case theme::Severity::Low:
      return Look::Low;
    case theme::Severity::Medium:
      return Look::Medium;
    case theme::Severity::High:
      return Look::High;
    case theme::Severity::Critical:
      return Look::Critical;
  }
  return Look::Info;
}

std::size_t width(std::string_view text) {
  return static_cast<std::size_t>(std::max(0, ftxui::string_width(text)));
}

std::size_t letter_width(char32_t letter) {
  if (letter >= 0x20 && letter < 0x7f) {
    return 1;
  }
  std::string one;
  text::append(one, letter);
  return width(one);
}

std::string clean(std::string_view text) {
  std::string out;
  out.reserve(text.size());
  for (const char32_t letter : text::decode(text)) {
    text::append(out, text::is_control(letter) ? U' ' : letter);
  }
  return out;
}

std::string clip(std::string_view text, std::size_t limit) {
  std::string cleaned = clean(text);
  if (width(cleaned) <= limit) {
    return cleaned;
  }
  if (limit == 0) {
    return {};
  }
  std::string out;
  std::size_t used = 0;
  for (const char32_t letter : text::decode(cleaned)) {
    const std::size_t size = letter_width(letter);
    if (used + size > limit - 1) {
      break;
    }
    text::append(out, letter);
    used += size;
  }
  out.append("…");
  return out;
}

std::string pad(std::string_view text, std::size_t size) {
  std::string out = clip(text, size);
  const std::size_t used = width(out);
  if (used < size) {
    out.append(size - used, ' ');
  }
  return out;
}

std::string repeat(std::string_view text, std::size_t count) {
  std::string out;
  out.reserve(text.size() * count);
  for (std::size_t index = 0; index < count; ++index) {
    out.append(text);
  }
  return out;
}

std::vector<std::string> wrap(std::string_view text, std::size_t limit) {
  limit = std::max(limit, min_wrap_width);
  std::vector<std::string> lines;
  for (const std::string& raw : text::split(text, '\n')) {
    const std::string paragraph = clean(raw);
    std::string current;
    for (std::string word : text::split(paragraph, ' ')) {
      while (width(word) > limit) {
        if (!current.empty()) {
          lines.push_back(std::move(current));
          current.clear();
        }
        std::string head;
        std::string rest;
        std::size_t used = 0;
        for (const char32_t letter : text::decode(word)) {
          const std::size_t size = letter_width(letter);
          if (rest.empty() && used + size <= limit) {
            text::append(head, letter);
            used += size;
          } else {
            text::append(rest, letter);
          }
        }
        lines.push_back(std::move(head));
        word = std::move(rest);
      }
      if (current.empty()) {
        current = std::move(word);
      } else if (width(current) + 1 + width(word) <= limit) {
        current.push_back(' ');
        current.append(word);
      } else {
        lines.push_back(std::move(current));
        current = std::move(word);
      }
    }
    lines.push_back(std::move(current));
  }
  return lines;
}

std::pair<std::size_t, std::size_t> window(std::size_t total, std::size_t cursor, std::size_t height) {
  if (height == 0 || total == 0) {
    return {0, 0};
  }
  if (total <= height) {
    return {0, total};
  }
  cursor = std::min(cursor, total - 1);
  const std::size_t half = height / 2;
  const std::size_t start = std::min(cursor > half ? cursor - half : 0, total - height);
  return {start, start + height};
}

Line Line::plain(std::string text) {
  return styled(std::move(text), Look::Plain);
}

Line Line::styled(std::string text, Look look) {
  Line line;
  line.spans.push_back({std::move(text), look});
  return line;
}

Line& Line::add(std::string text, Look look) {
  spans.push_back({std::move(text), look});
  return *this;
}

Line Line::with(std::string text, Look look) && {
  spans.push_back({std::move(text), look});
  return std::move(*this);
}

std::size_t Line::width() const {
  std::size_t total = 0;
  for (const Span& span : spans) {
    total += tui::width(span.text);
  }
  return total;
}

std::string Line::text() const {
  std::string out;
  for (const Span& span : spans) {
    out.append(span.text);
  }
  return out;
}

Line Line::recolor(Look look) && {
  for (Span& span : spans) {
    span.look = look;
  }
  return std::move(*this);
}

Line Line::fit(std::size_t size) const {
  Line out;
  std::size_t used = 0;
  for (const Span& span : spans) {
    if (used >= size) {
      break;
    }
    const std::size_t room = size - used;
    std::string shown = tui::width(span.text) > room ? clip(span.text, room) : clean(span.text);
    used += tui::width(shown);
    out.spans.push_back({std::move(shown), span.look});
  }
  if (used < size) {
    const bool selected = !out.spans.empty() && out.spans.back().look == Look::Selected;
    out.spans.push_back({std::string(size - used, ' '), selected ? Look::Selected : Look::Plain});
  }
  return out;
}

Lines wrapped(std::string_view text, std::size_t limit, Look look) {
  Lines lines;
  for (std::string& line : wrap(text, limit)) {
    lines.push_back(Line::styled(std::move(line), look));
  }
  return lines;
}

void extend(Lines& into, Lines more) {
  into.insert(into.end(), std::make_move_iterator(more.begin()), std::make_move_iterator(more.end()));
}

}
