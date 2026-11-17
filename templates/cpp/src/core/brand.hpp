#pragma once

#include <array>
#include <cstddef>
#include <filesystem>
#include <span>
#include <string>
#include <string_view>
#include <utility>
#include <vector>

#include "core/png.hpp"
#include "core/theme.hpp"

namespace app::brand {

inline constexpr theme::Color background{0x1F, 0x3A, 0x5F};
inline constexpr theme::Color foreground{0xF5, 0xF7, 0xFA};
inline constexpr theme::Color accent{0x4F, 0xC3, 0xA1};

struct Shape {
  double x = 0;
  double y = 0;
  double w = 0;
  double h = 0;
  double r = 0;
  theme::Color tint;
  bool operator==(const Shape&) const = default;
};

inline constexpr std::array<Shape, 6> motif = {{
    {0.22, 0.26, 0.10, 0.10, 0.05, accent},
    {0.40, 0.26, 0.38, 0.10, 0.05, foreground},
    {0.22, 0.45, 0.10, 0.10, 0.05, accent},
    {0.40, 0.45, 0.38, 0.10, 0.05, foreground},
    {0.22, 0.64, 0.10, 0.10, 0.05, accent},
    {0.40, 0.64, 0.24, 0.10, 0.05, foreground},
}};

inline constexpr std::array<Shape, 4> small_motif = {{
    {0.19, 0.25, 0.16, 0.19, 0.06, accent},
    {0.44, 0.25, 0.37, 0.19, 0.06, foreground},
    {0.19, 0.56, 0.16, 0.19, 0.06, accent},
    {0.44, 0.56, 0.37, 0.19, 0.06, foreground},
}};

inline constexpr Shape plate{0.04, 0.04, 0.92, 0.92, 0.20, background};

inline constexpr std::size_t small_limit = 24;
inline constexpr std::array<std::size_t, 9> sizes = {16, 24, 32, 48, 64, 128, 256, 512, 1024};
inline constexpr std::array<std::size_t, 4> ico_sizes = {16, 32, 48, 256};

struct IcnsEntry {
  std::string_view kind;
  std::size_t size = 0;
};

inline constexpr std::array<IcnsEntry, 7> icns_entries = {
    {{"icp4", 16}, {"icp5", 32}, {"icp6", 64}, {"ic07", 128}, {"ic08", 256}, {"ic09", 512}, {"ic10", 1024}}};

struct File {
  std::string name;
  png::Bytes content;
  bool operator==(const File&) const = default;
};

[[nodiscard]] std::span<const Shape> parts(std::size_t size);
[[nodiscard]] png::Bytes icon(std::size_t size);
[[nodiscard]] std::string svg(std::string_view title);
[[nodiscard]] std::string small_svg(std::string_view title);
[[nodiscard]] std::string desktop_entry(std::string_view name, std::string_view comment, std::string_view slug);
[[nodiscard]] png::Bytes picture(std::size_t size);
[[nodiscard]] png::Bytes ico();
[[nodiscard]] png::Bytes icns();
[[nodiscard]] std::vector<std::pair<std::string, std::string>> text_files(std::string_view name,
                                                                          std::string_view comment,
                                                                          std::string_view slug);
[[nodiscard]] std::vector<File> files(std::string_view name, std::string_view comment, std::string_view slug);
std::vector<std::string> write_files(const std::filesystem::path& directory, const std::vector<File>& list, bool check);

}
