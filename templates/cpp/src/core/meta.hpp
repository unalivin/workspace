#pragma once

#include <string>
#include <string_view>

namespace app::meta {

inline constexpr std::string_view app_name = "Template App";
inline constexpr std::string_view slug = "templateapp";
inline constexpr std::string_view app_id = "io.github.unalivin.templateapp";
inline constexpr std::string_view env_prefix = "TEMPLATEAPP";
inline constexpr std::string_view tagline = "TEMPLATE_TAGLINE";
inline constexpr std::string_view description = "TEMPLATE_DESCRIPTION";
inline constexpr std::string_view icon_concept = "TEMPLATE_ICON_CONCEPT";
inline constexpr std::string_view repository_url = "https://github.com/unalivin/workspace";
inline constexpr std::string_view license = "MIT";
inline constexpr std::string_view publisher = "Unalivin";

struct BuildInfo {
  std::string version;
  std::string os;
  std::string arch;
  std::string profile;
  std::string compiler;
  bool dashboard = false;

  bool operator==(const BuildInfo&) const = default;
};

[[nodiscard]] std::string_view version();
[[nodiscard]] std::string_view license_text();
[[nodiscard]] std::string_view notices_text();
[[nodiscard]] std::string_view usage_text();
[[nodiscard]] BuildInfo build(bool dashboard);
[[nodiscard]] std::string env_name(std::string_view suffix);
[[nodiscard]] std::string source_url();
[[nodiscard]] std::string advisory_url();
[[nodiscard]] std::string encode_query(std::string_view text);
[[nodiscard]] std::string new_issue_url(std::string_view title, std::string_view body);
[[nodiscard]] bool is_repository_url(std::string_view candidate);

}
