#include <algorithm>
#include <string>
#include <string_view>
#include <vector>

#include "core/guide.hpp"
#include "support/fuzzing.hpp"

extern "C" int LLVMFuzzerTestOneInput(const std::uint8_t* data, std::size_t size) {
  const auto text = fuzzing::text(data, size);
  if (!text) {
    return 0;
  }
  const std::size_t split = text->find('\n');
  const std::string_view query = split == std::string_view::npos ? std::string_view() : text->substr(0, split);
  const std::string_view markdown = split == std::string_view::npos ? *text : text->substr(split + 1);
  const app::guide::Guide parsed = app::guide::parse(markdown);
  static_cast<void>(parsed.search(query));
  static_cast<void>(app::guide::plain(markdown));
  std::vector<std::string_view> anchors;
  anchors.reserve(parsed.sections.size());
  for (const app::guide::Section& section : parsed.sections) {
    anchors.emplace_back(section.anchor);
  }
  const std::size_t total = anchors.size();
  std::sort(anchors.begin(), anchors.end());
  anchors.erase(std::unique(anchors.begin(), anchors.end()), anchors.end());
  fuzzing::require(anchors.size() == total, "guide_parse_and_search", "two sections share an anchor");
  return 0;
}
