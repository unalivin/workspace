#include "gui/render.hpp"

#include <algorithm>
#include <array>
#include <cmath>
#include <cstdint>
#include <string>

namespace app::gui {

namespace {

struct Corner {
  float x = 0;
  float y = 0;
  float u = 0;
  float v = 0;
  std::array<float, 4> tint{};
};

struct Sampler {
  const std::uint8_t* pixels = nullptr;
  int width = 0;
  int height = 0;
  int bytes = 4;

  [[nodiscard]] std::array<float, 4> texel(int x, int y) const {
    x = std::clamp(x, 0, width - 1);
    y = std::clamp(y, 0, height - 1);
    const std::uint8_t* at = pixels + (static_cast<std::ptrdiff_t>(y) * width + x) * bytes;
    if (bytes == 1) {
      return {1.0F, 1.0F, 1.0F, static_cast<float>(at[0]) / 255.0F};
    }
    return {static_cast<float>(at[0]) / 255.0F, static_cast<float>(at[1]) / 255.0F, static_cast<float>(at[2]) / 255.0F,
            static_cast<float>(at[3]) / 255.0F};
  }

  [[nodiscard]] std::array<float, 4> sample(float u, float v) const {
    if (pixels == nullptr || width <= 0 || height <= 0) {
      return {1.0F, 1.0F, 1.0F, 1.0F};
    }
    const float x = u * static_cast<float>(width) - 0.5F;
    const float y = v * static_cast<float>(height) - 0.5F;
    const float left = std::floor(x);
    const float top = std::floor(y);
    const float fx = x - left;
    const float fy = y - top;
    const int ix = static_cast<int>(left);
    const int iy = static_cast<int>(top);
    const auto a = texel(ix, iy);
    const auto b = texel(ix + 1, iy);
    const auto c = texel(ix, iy + 1);
    const auto d = texel(ix + 1, iy + 1);
    std::array<float, 4> out{};
    for (std::size_t part = 0; part < 4; ++part) {
      const float upper = a[part] + (b[part] - a[part]) * fx;
      const float lower = c[part] + (d[part] - c[part]) * fx;
      out[part] = upper + (lower - upper) * fy;
    }
    return out;
  }
};

struct Canvas {
  Picture* picture = nullptr;
  int left = 0;
  int top = 0;
  int right = 0;
  int bottom = 0;

  void blend(int x, int y, const std::array<float, 4>& ink) const {
    if (ink[3] <= 0.0F) {
      return;
    }
    std::uint8_t* at =
        &picture->pixels[(static_cast<std::size_t>(y) * picture->width + static_cast<std::size_t>(x)) * 4];
    const float alpha = std::min(ink[3], 1.0F);
    for (std::size_t part = 0; part < 3; ++part) {
      const float under = static_cast<float>(at[part]) / 255.0F;
      const float mixed = ink[part] * alpha + under * (1.0F - alpha);
      at[part] = static_cast<std::uint8_t>(std::lround(std::clamp(mixed, 0.0F, 1.0F) * 255.0F));
    }
    at[3] = 255;
  }
};

Corner corner(const ImDrawVert& vertex, ImVec2 origin, ImVec2 scale) {
  Corner out;
  out.x = (vertex.pos.x - origin.x) * scale.x;
  out.y = (vertex.pos.y - origin.y) * scale.y;
  out.u = vertex.uv.x;
  out.v = vertex.uv.y;
  out.tint = {static_cast<float>((vertex.col >> IM_COL32_R_SHIFT) & 0xFFU) / 255.0F,
              static_cast<float>((vertex.col >> IM_COL32_G_SHIFT) & 0xFFU) / 255.0F,
              static_cast<float>((vertex.col >> IM_COL32_B_SHIFT) & 0xFFU) / 255.0F,
              static_cast<float>((vertex.col >> IM_COL32_A_SHIFT) & 0xFFU) / 255.0F};
  return out;
}

double edge(double ax, double ay, double bx, double by, double px, double py) {
  return (bx - ax) * (py - ay) - (by - ay) * (px - ax);
}

bool owns(double ax, double ay, double bx, double by) {
  const double dx = bx - ax;
  const double dy = by - ay;
  return (dy == 0.0 && dx > 0.0) || dy < 0.0;
}

void triangle(const Canvas& canvas, const Sampler& sampler, Corner a, Corner b, Corner c) {
  double area = edge(a.x, a.y, b.x, b.y, c.x, c.y);
  if (area == 0.0) {
    return;
  }
  if (area < 0.0) {
    std::swap(b, c);
    area = -area;
  }
  const int first_x = std::max(canvas.left, static_cast<int>(std::floor(std::min({a.x, b.x, c.x}))));
  const int last_x = std::min(canvas.right - 1, static_cast<int>(std::ceil(std::max({a.x, b.x, c.x}))));
  const int first_y = std::max(canvas.top, static_cast<int>(std::floor(std::min({a.y, b.y, c.y}))));
  const int last_y = std::min(canvas.bottom - 1, static_cast<int>(std::ceil(std::max({a.y, b.y, c.y}))));
  const bool own_ab = owns(a.x, a.y, b.x, b.y);
  const bool own_bc = owns(b.x, b.y, c.x, c.y);
  const bool own_ca = owns(c.x, c.y, a.x, a.y);
  const bool flat = a.u == b.u && b.u == c.u && a.v == b.v && b.v == c.v;
  const std::array<float, 4> solid = flat ? sampler.sample(a.u, a.v) : std::array<float, 4>{};
  for (int y = first_y; y <= last_y; ++y) {
    const double py = static_cast<double>(y) + 0.5;
    for (int x = first_x; x <= last_x; ++x) {
      const double px = static_cast<double>(x) + 0.5;
      const double wa = edge(b.x, b.y, c.x, c.y, px, py);
      const double wb = edge(c.x, c.y, a.x, a.y, px, py);
      const double wc = edge(a.x, a.y, b.x, b.y, px, py);
      if (wa < 0.0 || wb < 0.0 || wc < 0.0) {
        continue;
      }
      if ((wa == 0.0 && !own_bc) || (wb == 0.0 && !own_ca) || (wc == 0.0 && !own_ab)) {
        continue;
      }
      const auto fa = static_cast<float>(wa / area);
      const auto fb = static_cast<float>(wb / area);
      const auto fc = static_cast<float>(wc / area);
      const std::array<float, 4> texel =
          flat ? solid : sampler.sample(a.u * fa + b.u * fb + c.u * fc, a.v * fa + b.v * fb + c.v * fc);
      std::array<float, 4> ink{};
      for (std::size_t part = 0; part < 4; ++part) {
        ink[part] = (a.tint[part] * fa + b.tint[part] * fb + c.tint[part] * fc) * texel[part];
      }
      canvas.blend(x, y, ink);
    }
  }
}

const char* read_clipboard(ImGuiContext*) {
  auto* text = static_cast<std::string*>(ImGui::GetPlatformIO().Platform_ClipboardUserData);
  return text == nullptr ? "" : text->c_str();
}

void write_clipboard(ImGuiContext*, const char* value) {
  auto* text = static_cast<std::string*>(ImGui::GetPlatformIO().Platform_ClipboardUserData);
  if (text != nullptr) {
    *text = value == nullptr ? "" : value;
  }
}

}

std::array<std::uint8_t, 4> Picture::at(std::size_t x, std::size_t y) const {
  const std::size_t start = (y * width + x) * 4;
  return {pixels[start], pixels[start + 1], pixels[start + 2], pixels[start + 3]};
}

void serve_textures(ImDrawData* data) {
  if (data == nullptr || data->Textures == nullptr) {
    return;
  }
  for (ImTextureData* texture : *data->Textures) {
    if (texture->Status == ImTextureStatus_WantCreate || texture->Status == ImTextureStatus_WantUpdates) {
      texture->SetTexID(static_cast<ImTextureID>(reinterpret_cast<std::uintptr_t>(texture)));
      texture->SetStatus(ImTextureStatus_OK);
    } else if (texture->Status == ImTextureStatus_WantDestroy && texture->UnusedFrames > 0) {
      texture->SetTexID(ImTextureID_Invalid);
      texture->SetStatus(ImTextureStatus_Destroyed);
    }
  }
}

Picture rasterize(const ImDrawData* data, ImVec4 clear) {
  Picture picture;
  const ImVec2 scale = data->FramebufferScale;
  picture.width = static_cast<std::size_t>(std::max(std::lround(data->DisplaySize.x * scale.x), 0L));
  picture.height = static_cast<std::size_t>(std::max(std::lround(data->DisplaySize.y * scale.y), 0L));
  picture.pixels.resize(picture.width * picture.height * 4);
  const std::array<std::uint8_t, 4> paper = {
      static_cast<std::uint8_t>(std::lround(std::clamp(clear.x, 0.0F, 1.0F) * 255.0F)),
      static_cast<std::uint8_t>(std::lround(std::clamp(clear.y, 0.0F, 1.0F) * 255.0F)),
      static_cast<std::uint8_t>(std::lround(std::clamp(clear.z, 0.0F, 1.0F) * 255.0F)), 255};
  for (std::size_t index = 0; index < picture.pixels.size(); index += 4) {
    std::copy(paper.begin(), paper.end(), picture.pixels.begin() + static_cast<std::ptrdiff_t>(index));
  }
  const ImVec2 origin = data->DisplayPos;
  for (const ImDrawList* list : data->CmdLists) {
    for (const ImDrawCmd& command : list->CmdBuffer) {
      if (command.UserCallback != nullptr) {
        continue;
      }
      Canvas canvas;
      canvas.picture = &picture;
      canvas.left = std::max(static_cast<int>(std::floor((command.ClipRect.x - origin.x) * scale.x)), 0);
      canvas.top = std::max(static_cast<int>(std::floor((command.ClipRect.y - origin.y) * scale.y)), 0);
      canvas.right = std::min(static_cast<int>(std::ceil((command.ClipRect.z - origin.x) * scale.x)),
                              static_cast<int>(picture.width));
      canvas.bottom = std::min(static_cast<int>(std::ceil((command.ClipRect.w - origin.y) * scale.y)),
                               static_cast<int>(picture.height));
      if (canvas.right <= canvas.left || canvas.bottom <= canvas.top) {
        continue;
      }
      Sampler sampler;
      const ImTextureID id = command.GetTexID();
      if (id != ImTextureID_Invalid) {
        auto* texture = reinterpret_cast<ImTextureData*>(static_cast<std::uintptr_t>(id));
        sampler.pixels = texture->Pixels;
        sampler.width = texture->Width;
        sampler.height = texture->Height;
        sampler.bytes = texture->BytesPerPixel;
      }
      const ImDrawIdx* indices = list->IdxBuffer.Data + command.IdxOffset;
      const ImDrawVert* vertices = list->VtxBuffer.Data + command.VtxOffset;
      for (unsigned int index = 0; index + 2 < command.ElemCount; index += 3) {
        triangle(canvas, sampler, corner(vertices[indices[index]], origin, scale),
                 corner(vertices[indices[index + 1]], origin, scale),
                 corner(vertices[indices[index + 2]], origin, scale));
      }
    }
  }
  return picture;
}

Stage::Stage(float width, float height, float scale) {
  previous_ = ImGui::GetCurrentContext();
  context_ = ImGui::CreateContext();
  ImGui::SetCurrentContext(context_);
  ImGuiIO& io = ImGui::GetIO();
  io.IniFilename = nullptr;
  io.LogFilename = nullptr;
  io.BackendFlags |= ImGuiBackendFlags_RendererHasTextures;
  io.DisplaySize = ImVec2(width, height);
  io.DisplayFramebufferScale = ImVec2(scale, scale);
  io.DeltaTime = static_cast<float>(step_);
  io.ConfigFlags |= ImGuiConfigFlags_NavEnableKeyboard;
  ImGuiPlatformIO& platform = ImGui::GetPlatformIO();
  platform.Platform_ClipboardUserData = &clipboard_;
  platform.Platform_GetClipboardTextFn = read_clipboard;
  platform.Platform_SetClipboardTextFn = write_clipboard;
}

Stage::~Stage() {
  ImGui::SetCurrentContext(context_);
  ImDrawData* data = ImGui::GetDrawData();
  if (data != nullptr && data->Textures != nullptr) {
    for (ImTextureData* texture : *data->Textures) {
      texture->SetTexID(ImTextureID_Invalid);
      texture->SetStatus(ImTextureStatus_Destroyed);
    }
  }
  for (ImTextureData* texture : ImGui::GetPlatformIO().Textures) {
    texture->SetTexID(ImTextureID_Invalid);
    texture->SetStatus(ImTextureStatus_Destroyed);
  }
  ImGui::DestroyContext(context_);
  ImGui::SetCurrentContext(previous_);
}

void Stage::resize(float width, float height) {
  ImGui::SetCurrentContext(context_);
  ImGui::GetIO().DisplaySize = ImVec2(width, height);
}

void Stage::advance(double seconds) {
  step_ = seconds;
}

void Stage::run(const std::function<void()>& frame) {
  ImGui::SetCurrentContext(context_);
  ImGui::GetIO().DeltaTime = static_cast<float>(step_);
  ImGui::NewFrame();
  frame();
  ImGui::Render();
  serve_textures(ImGui::GetDrawData());
}

Picture Stage::settle(std::size_t frames, const std::function<void()>& frame) {
  for (std::size_t index = 0; index < std::max<std::size_t>(frames, 1); ++index) {
    run(frame);
  }
  return picture();
}

Picture Stage::picture() const {
  ImGui::SetCurrentContext(context_);
  return rasterize(ImGui::GetDrawData(), clear());
}

void Stage::move(ImVec2 point) {
  ImGui::SetCurrentContext(context_);
  ImGui::GetIO().AddMousePosEvent(point.x, point.y);
}

void Stage::press(bool down) {
  ImGui::SetCurrentContext(context_);
  ImGui::GetIO().AddMouseButtonEvent(0, down);
}

void Stage::wheel(float sideways, float upright) {
  ImGui::SetCurrentContext(context_);
  ImGui::GetIO().AddMouseWheelEvent(sideways, upright);
}

void Stage::key(ImGuiKey key, bool down) {
  ImGui::SetCurrentContext(context_);
  ImGui::GetIO().AddKeyEvent(key, down);
}

void Stage::type(const char* text) {
  ImGui::SetCurrentContext(context_);
  ImGui::GetIO().AddInputCharactersUTF8(text);
}

const std::string& Stage::clipboard() const {
  return clipboard_;
}

ImVec4 Stage::clear() const {
  ImGui::SetCurrentContext(context_);
  return ImGui::GetStyle().Colors[ImGuiCol_WindowBg];
}

}
