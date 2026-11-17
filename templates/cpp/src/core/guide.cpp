#include "core/guide.hpp"

#include <algorithm>
#include <map>
#include <utility>

#include "core/text.hpp"

namespace app::guide {

namespace {

using Taken = std::map<std::string, std::size_t, std::less<>>;

std::string_view strip(std::string_view value, std::string_view letters) {
  const std::size_t first = value.find_first_not_of(letters);
  if (first == std::string_view::npos) {
    return {};
  }
  return value.substr(first, value.find_last_not_of(letters) - first + 1);
}

std::string anchor(std::string_view title, Taken& taken) {
  std::string collected;
  for (const char32_t letter : text::decode(text::lower(title))) {
    if (text::is_word(letter)) {
      text::append(collected, letter);
    } else if (letter == U' ' || letter == U'-' || letter == U'_') {
      collected.push_back('-');
    }
  }
  const std::string_view trimmed = strip(collected, "-");
  const std::string base = trimmed.empty() ? std::string("section") : std::string(trimmed);
  std::string candidate = base;
  const auto known = taken.find(base);
  std::size_t number = known == taken.end() ? 0 : known->second;
  while (taken.contains(candidate)) {
    ++number;
    candidate = text::concat(base, "-", number);
  }
  taken[base] = number;
  taken.try_emplace(candidate, 0);
  return candidate;
}

struct Heading {
  std::size_t level = 0;
  std::string title;
};

std::optional<Heading> heading(std::string_view line) {
  std::size_t level = 0;
  while (level < line.size() && line[level] == '#') {
    ++level;
  }
  if (level == 0 || level > 6 || level >= line.size() || line[level] != ' ') {
    return std::nullopt;
  }
  std::string_view title = text::trim_whitespace(line.substr(level));
  while (!title.empty() && title.back() == '#') {
    title.remove_suffix(1);
  }
  title = text::trim_whitespace(title);
  if (title.empty()) {
    return std::nullopt;
  }
  return Heading{level, std::string(title)};
}

std::string excerpt(std::string_view body, std::size_t position, std::size_t length) {
  const std::u32string letters = text::decode(body);
  const std::size_t before = text::length(body.substr(0, position));
  const std::size_t start = before > 40 ? before - 40 : 0;
  const std::size_t end = std::min(start + 80 + length, letters.size());
  std::string out = text::collapse_whitespace(text::encode(std::u32string_view(letters).substr(start, end - start)));
  if (start > 0) {
    out.insert(0, "\xE2\x80\xA6");
  }
  if (end < letters.size()) {
    out.append("\xE2\x80\xA6");
  }
  return out;
}

std::string without_marks(std::string_view line) {
  return text::replace_all(text::replace_all(line, "**", ""), "`", "");
}

}

Guide parse(std::string_view markdown) {
  Guide guide;
  Taken taken;
  std::vector<std::string> body;
  std::optional<Heading> current;
  bool fenced = false;
  const auto flush = [&] {
    std::string content(text::trim_whitespace(text::join(body, "\n")));
    body.clear();
    if (current) {
      guide.sections.push_back({current->level, current->title, anchor(current->title, taken), std::move(content)});
    } else if (!content.empty()) {
      guide.sections.push_back({1, "Introduction", anchor("Introduction", taken), std::move(content)});
    }
  };
  for (std::string& line : text::split(text::replace_all(markdown, "\r\n", "\n"), '\n')) {
    if (text::trim_whitespace(line).starts_with("```")) {
      fenced = !fenced;
    }
    std::optional<Heading> found = fenced ? std::nullopt : heading(line);
    if (found) {
      flush();
      current = std::move(found);
    } else {
      body.push_back(std::move(line));
    }
  }
  flush();
  return guide;
}

std::optional<std::size_t> Guide::find(std::string_view anchor) const {
  for (std::size_t index = 0; index < sections.size(); ++index) {
    if (sections[index].anchor == anchor) {
      return index;
    }
  }
  return std::nullopt;
}

std::vector<Hit> Guide::search(std::string_view query) const {
  const std::vector<std::string> terms = text::words(text::lower(query));
  if (terms.empty()) {
    return {};
  }
  std::vector<Hit> hits;
  for (std::size_t index = 0; index < sections.size(); ++index) {
    const Section& section = sections[index];
    const std::string haystack = text::lower(text::concat(section.title, "\n", section.body));
    if (!std::ranges::all_of(terms, [&haystack](const std::string& term) { return text::contains(haystack, term); })) {
      continue;
    }
    const std::size_t position = text::lower(section.body).find(terms.front());
    hits.push_back(
        {index, section.title,
         position == std::string::npos ? section.title : excerpt(section.body, position, text::length(terms.front()))});
  }
  return hits;
}

std::string plain(std::string_view markdown) {
  std::vector<std::string> lines;
  bool fenced = false;
  for (const std::string& line : text::split(text::replace_all(markdown, "\r\n", "\n"), '\n')) {
    const std::string_view trimmed = text::trim_whitespace(line);
    if (trimmed.starts_with("```")) {
      fenced = !fenced;
      continue;
    }
    if (fenced) {
      lines.push_back(text::concat("    ", line));
      continue;
    }
    if (trimmed.starts_with('|')) {
      if (strip(trimmed, "|-: ").empty()) {
        continue;
      }
      std::vector<std::string> cells;
      for (const std::string& cell : text::split(strip(trimmed, "|"), '|')) {
        cells.emplace_back(text::trim_whitespace(cell));
      }
      lines.push_back(without_marks(text::join(cells, "  \xC2\xB7  ")));
      continue;
    }
    lines.push_back(without_marks(line));
  }
  return std::string(text::trim_whitespace(text::join(lines, "\n")));
}

}
