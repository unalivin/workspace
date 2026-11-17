#pragma once

#include <cstddef>
#include <optional>
#include <string>
#include <string_view>
#include <vector>

namespace app::guide {

struct Section {
  std::size_t level = 1;
  std::string title;
  std::string anchor;
  std::string body;
  bool operator==(const Section&) const = default;
};

struct Hit {
  std::size_t section = 0;
  std::string title;
  std::string excerpt;
  bool operator==(const Hit&) const = default;
};

struct Guide {
  std::vector<Section> sections;

  [[nodiscard]] std::optional<std::size_t> find(std::string_view anchor) const;
  [[nodiscard]] std::vector<Hit> search(std::string_view query) const;
  bool operator==(const Guide&) const = default;
};

[[nodiscard]] Guide parse(std::string_view markdown);
[[nodiscard]] std::string plain(std::string_view markdown);

}
