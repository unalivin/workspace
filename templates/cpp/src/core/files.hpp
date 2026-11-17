#pragma once

#include <cstdint>
#include <filesystem>
#include <string>
#include <string_view>

namespace app::files {

[[nodiscard]] std::string shown(const std::filesystem::path& path);
[[nodiscard]] std::filesystem::path from_utf8(std::string_view text);
void restrict(const std::filesystem::path& path, bool directory);
void create_private_dir(const std::filesystem::path& path);
void write_private(const std::filesystem::path& path, std::string_view content, bool overwrite);
[[nodiscard]] std::string read_limited(const std::filesystem::path& path, std::uint64_t limit);
[[nodiscard]] std::string read_all(const std::filesystem::path& path);
[[nodiscard]] bool is_directory(const std::filesystem::path& path);
[[nodiscard]] bool exists(const std::filesystem::path& path);
[[nodiscard]] std::uint32_t permissions(const std::filesystem::path& path);
[[nodiscard]] std::uint64_t process_id();

}
