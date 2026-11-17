#pragma once

#include <imgui.h>

#include <array>
#include <cstddef>
#include <cstdint>
#include <map>
#include <string>
#include <string_view>
#include <utility>
#include <vector>

namespace app::gui {

inline constexpr float icon_view = 24.0F;
inline constexpr std::string_view fallback_icon = "info";

struct IconPath {
  std::string_view name;
  std::string_view data;
};

inline constexpr std::array<IconPath, 27> icon_paths = {{
    {"layout-dashboard", "M3 3h8v10H3z M13 3h8v6h-8z M13 11h8v10h-8z M3 15h8v6H3z"},
    {"list", "M3 5h2v2H3z M7 5h14v2H7z M3 11h2v2H3z M7 11h14v2H7z M3 17h2v2H3z M7 17h14v2H7z"},
    {"bell",
     "M12 2a2 2 0 0 1 2 2v.3A7 7 0 0 1 19 11v4l2 3v1H3v-1l2-3v-4a7 7 0 0 1 5-6.7V4a2 2 0 0 1 2-2z M9.5 20h5a2.5 "
     "2.5 0 0 1-5 0z"},
    {"settings",
     "M3 5h9v2H3z M16 5h5v2h-5z M12 3h2v6h-2z M3 11h3v2H3z M10 11h11v2H10z M6 9h2v6H6z M3 17h11v2H3z M18 "
     "17h3v2h-3z M14 15h2v6h-2z"},
    {"info",
     "M12 2a10 10 0 1 0 0 20a10 10 0 1 0 0-20z M12 4a8 8 0 1 1 0 16a8 8 0 1 1 0-16z M11 7h2v2h-2z M11 "
     "11h2v6h-2z"},
    {"octagon-alert", "M8 2h8l6 6v8l-6 6H8l-6-6V8z M11 6v8h2V6z M11 16v2h2v-2z"},
    {"triangle-alert", "M12 2L23 21H1z M11 9v6h2V9z M11 17v2h2v-2z"},
    {"circle-alert", "M12 2a10 10 0 1 1 0 20a10 10 0 1 1 0-20z M11 6v8h2V6z M11 16v2h2v-2z"},
    {"circle-minus", "M12 2a10 10 0 1 1 0 20a10 10 0 1 1 0-20z M7 11v2h10v-2z"},
    {"status-open", "M12 3a9 9 0 1 0 0 18a9 9 0 1 0 0-18z M12 5a7 7 0 1 1 0 14a7 7 0 1 1 0-14z"},
    {"status-progress",
     "M12 3a9 9 0 1 0 0 18a9 9 0 1 0 0-18z M12 5a7 7 0 1 1 0 14a7 7 0 1 1 0-14z M12 5a7 7 0 0 1 0 14z"},
    {"status-done", "M12 3a9 9 0 1 0 0 18a9 9 0 1 0 0-18z M8.5 11L10.5 13L16 7.5L17.5 9L10.5 16L7 12.5z"},
    {"sidebar", "M3 4h18v16H3z M5 6v12h4V6z"},
    {"terminal", "M2 4h20v16H2z M4 6v12h16V6z M6 9l1.4-1.4L11.8 12l-4.4 4.4L6 15l3-3z M12 15h6v2h-6z"},
    {"x", "M6.4 5L12 10.6L17.6 5L19 6.4L13.4 12L19 17.6L17.6 19L12 13.4L6.4 19L5 17.6L10.6 12L5 6.4z"},
    {"check", "M9 16.2L4.8 12L3.4 13.4L9 19L21 7L19.6 5.6z"},
    {"plus", "M11 5h2v6h6v2h-6v6h-2v-6H5v-2h6z"},
    {"copy", "M8 3h12v14H8z M10 5v10h8V5z M4 7h2v12h10v2H4z"},
    {"columns", "M3 4h18v16H3z M5 6v12h3V6z M10 6v12h4V6z M16 6v12h3V6z"},
    {"trash", "M9 3h6v2h5v2H4V5h5z M6 9h12v12H6z M8 11v8h8v-8z"},
    {"pencil", "M16.5 3L21 7.5L8.5 20H4v-4.5z M6 16.3V18h1.7L18.2 7.5L16.5 5.8z"},
    {"download", "M11 3h2v9.2l3.6-3.6L18 10l-6 6l-6-6l1.4-1.4L11 12.2z M4 19h16v2H4z"},
    {"upload", "M11 16h2V6.8l3.6 3.6L18 9l-6-6l-6 6l1.4 1.4L11 6.8z M4 19h16v2H4z"},
    {"arrow-right", "M4 11h12.2l-4.6-4.6L13 5l7 7l-7 7l-1.4-1.4l4.6-4.6H4z"},
    {"palette", "M3 3h8v8H3z M13 3h8v8h-8z M3 13h8v8H3z M13 13h8v8h-8z M15 15v4h4v-4z"},
    {"monitor", "M2 4h20v13H2z M4 6v9h16V6z M8 19h8v2H8z"},
    {"message", "M2 4h20v14H9l-5 4v-4H2z M4 6v10h2v2l2.5-2H20V6z"},
}};

struct Point {
  float x = 0;
  float y = 0;
  bool operator==(const Point&) const = default;
};

using Outline = std::vector<Point>;

[[nodiscard]] bool known_icon(std::string_view name);
[[nodiscard]] std::string_view icon_path(std::string_view name);
[[nodiscard]] std::vector<Outline> outlines(std::string_view data);
[[nodiscard]] std::vector<std::uint8_t> mask(const std::vector<Outline>& shapes, std::size_t size);

class Icons {
 public:
  void draw(ImDrawList* canvas, std::string_view name, ImVec2 corner, float side, ImU32 tint);
  void logo(ImDrawList* canvas, ImVec2 corner, float side);
  [[nodiscard]] std::vector<std::string> drawn() const;

 private:
  [[nodiscard]] ImFontAtlasRectId slot(const std::string& name, std::size_t pixels);

  std::map<std::pair<std::string, std::size_t>, ImFontAtlasRectId> slots_;
  ImFontAtlas* atlas_ = nullptr;
};

}
