#pragma once

#include <imgui.h>

#include <array>
#include <cstddef>
#include <cstdint>
#include <functional>
#include <string>
#include <vector>

namespace app::gui {

struct Picture {
  std::size_t width = 0;
  std::size_t height = 0;
  std::vector<std::uint8_t> pixels;

  [[nodiscard]] std::array<std::uint8_t, 4> at(std::size_t x, std::size_t y) const;
};

void serve_textures(ImDrawData* data);
[[nodiscard]] Picture rasterize(const ImDrawData* data, ImVec4 clear);

class Stage {
 public:
  Stage(float width, float height, float scale);
  ~Stage();
  Stage(const Stage&) = delete;
  Stage& operator=(const Stage&) = delete;

  void resize(float width, float height);
  void advance(double seconds);
  void run(const std::function<void()>& frame);
  [[nodiscard]] Picture settle(std::size_t frames, const std::function<void()>& frame);
  [[nodiscard]] Picture picture() const;
  void move(ImVec2 point);
  void press(bool down);
  void wheel(float sideways, float upright);
  void key(ImGuiKey key, bool down);
  void type(const char* text);
  [[nodiscard]] const std::string& clipboard() const;
  [[nodiscard]] ImVec4 clear() const;

 private:
  ImGuiContext* context_ = nullptr;
  ImGuiContext* previous_ = nullptr;
  double step_ = 1.0 / 60.0;
  std::string clipboard_;
};

}
