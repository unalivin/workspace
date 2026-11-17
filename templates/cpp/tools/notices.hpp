#pragma once

#include <filesystem>
#include <string>
#include <string_view>
#include <vector>

namespace tools::notices {

struct Component {
  std::string name;
  std::string version;
  std::string license;
  std::string source;
  std::string part_of;
  std::string use;
  std::string license_file;
  std::string port;
  std::string commit;
};

inline constexpr std::string_view address_omitted = "(address omitted)";
inline constexpr std::string_view components_path = "third_party/components.tsv";
inline constexpr std::string_view licenses_directory = "third_party/licenses";
inline constexpr std::string_view font_directory = "src/gui/fonts";
inline constexpr std::string_view notices_path = "THIRD_PARTY_NOTICES.md";

[[nodiscard]] std::vector<Component> parse_components(std::string_view table);
[[nodiscard]] std::vector<Component> load_components(const std::filesystem::path& root);
[[nodiscard]] std::string omit_addresses(std::string_view line);
[[nodiscard]] const std::vector<std::string_view>& allowed_licenses();
[[nodiscard]] std::string render(const std::filesystem::path& root);
[[nodiscard]] std::string read_text(const std::filesystem::path& path);
void write_text(const std::filesystem::path& path, std::string_view content);

}
