#include "gui/icons.hpp"

#include <algorithm>
#include <cmath>
#include <numbers>
#include <optional>
#include <set>

#include "core/brand.hpp"
#include "core/error.hpp"
#include "core/text.hpp"

namespace app::gui {

namespace {

constexpr std::size_t samples = 4;
constexpr float arc_step = 0.2F;
constexpr std::string_view logo_name = "#logo";

struct Token {
  char command = 0;
  float number = 0;
};

bool is_letter(char letter) {
  return (letter >= 'a' && letter <= 'z') || (letter >= 'A' && letter <= 'Z');
}

bool is_digit(char letter) {
  return letter >= '0' && letter <= '9';
}

char upper(char letter) {
  return letter >= 'a' && letter <= 'z' ? static_cast<char>(letter - 'a' + 'A') : letter;
}

float number_from(std::string_view digits) {
  float sign = 1.0F;
  std::size_t at = 0;
  if (at < digits.size() && (digits[at] == '-' || digits[at] == '+')) {
    sign = digits[at] == '-' ? -1.0F : 1.0F;
    ++at;
  }
  float whole = 0;
  bool seen = false;
  while (at < digits.size() && is_digit(digits[at])) {
    whole = whole * 10.0F + static_cast<float>(digits[at] - '0');
    seen = true;
    ++at;
  }
  if (at < digits.size() && digits[at] == '.') {
    ++at;
    float place = 0.1F;
    while (at < digits.size() && is_digit(digits[at])) {
      whole += static_cast<float>(digits[at] - '0') * place;
      place *= 0.1F;
      seen = true;
      ++at;
    }
  }
  if (!seen) {
    throw AppError::usage(text::concat("\"", digits, "\" is not a number"));
  }
  return sign * whole;
}

std::vector<Token> tokens(std::string_view data) {
  std::vector<Token> found;
  std::size_t at = 0;
  while (at < data.size()) {
    const char letter = data[at];
    if (letter == ' ' || letter == ',' || letter == '\n' || letter == '\t' || letter == '\r') {
      ++at;
    } else if (is_letter(letter)) {
      found.push_back({letter, 0});
      ++at;
    } else if (letter == '-' || letter == '+' || letter == '.' || is_digit(letter)) {
      const std::size_t start = at;
      bool dot = letter == '.';
      ++at;
      while (at < data.size() && (is_digit(data[at]) || (data[at] == '.' && !dot))) {
        dot = dot || data[at] == '.';
        ++at;
      }
      found.push_back({0, number_from(data.substr(start, at - start))});
    } else {
      throw AppError::usage(text::concat("unexpected \"", std::string(1, letter), "\" in the path"));
    }
  }
  return found;
}

float angle(float ux, float uy, float vx, float vy) {
  return std::atan2(ux * vy - uy * vx, ux * vx + uy * vy);
}

void arc(Point from, Point radii, float rotation, bool large, bool sweep, Point to, Outline& out) {
  float rx = std::abs(radii.x);
  float ry = std::abs(radii.y);
  if (rx == 0.0F || ry == 0.0F || from == to) {
    out.push_back(to);
    return;
  }
  const float turn = rotation * std::numbers::pi_v<float> / 180.0F;
  const float sin = std::sin(turn);
  const float cos = std::cos(turn);
  const float dx = (from.x - to.x) / 2.0F;
  const float dy = (from.y - to.y) / 2.0F;
  const float x1 = cos * dx + sin * dy;
  const float y1 = -sin * dx + cos * dy;
  const float stretch = (x1 * x1) / (rx * rx) + (y1 * y1) / (ry * ry);
  if (stretch > 1.0F) {
    rx *= std::sqrt(stretch);
    ry *= std::sqrt(stretch);
  }
  const float top = std::max(rx * rx * ry * ry - rx * rx * y1 * y1 - ry * ry * x1 * x1, 0.0F);
  const float bottom = rx * rx * y1 * y1 + ry * ry * x1 * x1;
  const float sign = large == sweep ? -1.0F : 1.0F;
  const float factor = bottom == 0.0F ? 0.0F : sign * std::sqrt(top / bottom);
  const float cx1 = factor * rx * y1 / ry;
  const float cy1 = -factor * ry * x1 / rx;
  const float cx = cos * cx1 - sin * cy1 + (from.x + to.x) / 2.0F;
  const float cy = sin * cx1 + cos * cy1 + (from.y + to.y) / 2.0F;
  const float start = angle(1.0F, 0.0F, (x1 - cx1) / rx, (y1 - cy1) / ry);
  float span = angle((x1 - cx1) / rx, (y1 - cy1) / ry, (-x1 - cx1) / rx, (-y1 - cy1) / ry);
  const float full = 2.0F * std::numbers::pi_v<float>;
  if (!sweep && span > 0.0F) {
    span -= full;
  } else if (sweep && span < 0.0F) {
    span += full;
  }
  const auto steps = std::max<std::size_t>(static_cast<std::size_t>(std::ceil(std::abs(span) / arc_step)), 1);
  for (std::size_t step = 1; step < steps; ++step) {
    const float at = start + span * static_cast<float>(step) / static_cast<float>(steps);
    const float at_sin = std::sin(at);
    const float at_cos = std::cos(at);
    out.push_back({cx + cos * rx * at_cos - sin * ry * at_sin, cy + sin * rx * at_cos + cos * ry * at_sin});
  }
  out.push_back(to);
}

std::size_t arity(char command) {
  switch (upper(command)) {
    case 'M':
    case 'L':
      return 2;
    case 'H':
    case 'V':
      return 1;
    case 'A':
      return 7;
    default:
      return 0;
  }
}

int winding(const std::vector<Outline>& shapes, float x, float y) {
  int turns = 0;
  for (const Outline& shape : shapes) {
    for (std::size_t index = 0; index < shape.size(); ++index) {
      const Point from = shape[index];
      const Point to = shape[(index + 1) % shape.size()];
      const float side = (to.x - from.x) * (y - from.y) - (x - from.x) * (to.y - from.y);
      if (from.y <= y && to.y > y && side > 0.0F) {
        ++turns;
      } else if (from.y > y && to.y <= y && side < 0.0F) {
        --turns;
      }
    }
  }
  return turns;
}

}

bool known_icon(std::string_view name) {
  return std::ranges::find(icon_paths, name, &IconPath::name) != icon_paths.end();
}

std::string_view icon_path(std::string_view name) {
  auto found = std::ranges::find(icon_paths, name, &IconPath::name);
  if (found == icon_paths.end()) {
    found = std::ranges::find(icon_paths, fallback_icon, &IconPath::name);
  }
  return found == icon_paths.end() ? std::string_view() : found->data;
}

std::vector<Outline> outlines(std::string_view data) {
  const std::vector<Token> list = tokens(data);
  std::vector<Outline> shapes;
  Outline current;
  Point pen;
  Point start;
  char command = ' ';
  std::size_t at = 0;
  const auto flush = [&shapes, &current] {
    if (current.size() > 2) {
      shapes.push_back(std::move(current));
    }
    current.clear();
  };
  while (at < list.size()) {
    if (list[at].command != 0) {
      command = list[at].command;
      ++at;
      if (std::string_view("MLHVAZmlhvaz").find(command) == std::string_view::npos) {
        throw AppError::usage(text::concat("the path command \"", std::string(1, command), "\" is not supported"));
      }
      if (upper(command) == 'Z') {
        flush();
        pen = start;
      }
      continue;
    }
    const std::size_t count = arity(command);
    if (count == 0) {
      throw AppError::usage("a number appears where a path command is expected");
    }
    std::vector<float> numbers;
    for (std::size_t index = at; index < list.size() && numbers.size() < count; ++index) {
      if (list[index].command != 0) {
        break;
      }
      numbers.push_back(list[index].number);
    }
    if (numbers.size() != count) {
      throw AppError::usage(
          text::concat("the path command \"", std::string(1, command), "\" needs ", count, " numbers"));
    }
    at += count;
    const bool relative = command >= 'a' && command <= 'z';
    const auto offset = [relative, &pen](float x, float y) {
      return relative ? Point{pen.x + x, pen.y + y} : Point{x, y};
    };
    switch (upper(command)) {
      case 'M':
        flush();
        pen = offset(numbers[0], numbers[1]);
        start = pen;
        current.push_back(pen);
        command = relative ? 'l' : 'L';
        break;
      case 'L':
        pen = offset(numbers[0], numbers[1]);
        current.push_back(pen);
        break;
      case 'H':
        pen = {relative ? pen.x + numbers[0] : numbers[0], pen.y};
        current.push_back(pen);
        break;
      case 'V':
        pen = {pen.x, relative ? pen.y + numbers[0] : numbers[0]};
        current.push_back(pen);
        break;
      default: {
        const Point to = offset(numbers[5], numbers[6]);
        if (current.empty()) {
          current.push_back(pen);
        }
        arc(pen, {numbers[0], numbers[1]}, numbers[2], numbers[3] != 0.0F, numbers[4] != 0.0F, to, current);
        pen = to;
        break;
      }
    }
  }
  flush();
  return shapes;
}

std::vector<std::uint8_t> mask(const std::vector<Outline>& shapes, std::size_t size) {
  const float scale = icon_view / static_cast<float>(std::max<std::size_t>(size, 1));
  std::vector<std::uint8_t> coverage(size * size, 0);
  for (std::size_t py = 0; py < size; ++py) {
    for (std::size_t px = 0; px < size; ++px) {
      std::size_t inside = 0;
      for (std::size_t sy = 0; sy < samples; ++sy) {
        for (std::size_t sx = 0; sx < samples; ++sx) {
          const float x =
              (static_cast<float>(px) + (static_cast<float>(sx) + 0.5F) / static_cast<float>(samples)) * scale;
          const float y =
              (static_cast<float>(py) + (static_cast<float>(sy) + 0.5F) / static_cast<float>(samples)) * scale;
          if (winding(shapes, x, y) != 0) {
            ++inside;
          }
        }
      }
      coverage[py * size + px] = static_cast<std::uint8_t>(inside * 255 / (samples * samples));
    }
  }
  return coverage;
}

ImFontAtlasRectId Icons::slot(const std::string& name, std::size_t pixels) {
  ImFontAtlas* atlas = ImGui::GetIO().Fonts;
  if (atlas != atlas_) {
    slots_.clear();
    atlas_ = atlas;
  }
  const auto key = std::make_pair(name, pixels);
  if (const auto found = slots_.find(key); found != slots_.end()) {
    return found->second;
  }
  const int side = static_cast<int>(pixels);
  ImFontAtlasRect place;
  const ImFontAtlasRectId id = atlas->AddCustomRect(side, side, &place);
  if (id == ImFontAtlasRectId_Invalid) {
    return id;
  }
  std::vector<std::uint8_t> rgba(pixels * pixels * 4, 0);
  if (name == logo_name) {
    rgba = brand::icon(pixels);
  } else {
    std::vector<Outline> shapes;
    try {
      shapes = outlines(icon_path(name));
    } catch (const AppError&) {
      shapes.clear();
    }
    const std::vector<std::uint8_t> coverage = mask(shapes, pixels);
    for (std::size_t index = 0; index < coverage.size(); ++index) {
      rgba[index * 4] = 255;
      rgba[index * 4 + 1] = 255;
      rgba[index * 4 + 2] = 255;
      rgba[index * 4 + 3] = coverage[index];
    }
  }
  ImTextureData* texture = atlas->TexData;
  for (std::size_t row = 0; row < pixels; ++row) {
    auto* target = static_cast<std::uint8_t*>(texture->GetPixelsAt(place.x, place.y + static_cast<int>(row)));
    for (std::size_t column = 0; column < pixels; ++column) {
      const std::uint8_t* source = &rgba[(row * pixels + column) * 4];
      if (texture->Format == ImTextureFormat_RGBA32) {
        std::copy_n(source, 4, target + column * 4);
      } else {
        target[column] = source[3];
      }
    }
  }
  slots_.emplace(key, id);
  return id;
}

void Icons::draw(ImDrawList* canvas, std::string_view name, ImVec2 corner, float side, ImU32 tint) {
  const float scale = ImGui::GetIO().DisplayFramebufferScale.x;
  const auto pixels = static_cast<std::size_t>(std::clamp(std::round(side * std::max(scale, 1.0F)), 8.0F, 256.0F));
  const ImFontAtlasRectId id = slot(std::string(name), pixels);
  ImFontAtlasRect place;
  if (id == ImFontAtlasRectId_Invalid || !atlas_->GetCustomRect(id, &place)) {
    return;
  }
  const ImVec2 origin(std::round(corner.x), std::round(corner.y));
  canvas->AddImage(atlas_->TexRef, origin, ImVec2(origin.x + side, origin.y + side), place.uv0, place.uv1, tint);
}

void Icons::logo(ImDrawList* canvas, ImVec2 corner, float side) {
  const float scale = ImGui::GetIO().DisplayFramebufferScale.x;
  const auto pixels = static_cast<std::size_t>(std::clamp(std::round(side * std::max(scale, 1.0F)), 16.0F, 512.0F));
  const ImFontAtlasRectId id = slot(std::string(logo_name), pixels);
  ImFontAtlasRect place;
  if (id == ImFontAtlasRectId_Invalid || !atlas_->GetCustomRect(id, &place)) {
    return;
  }
  const ImVec2 origin(std::round(corner.x), std::round(corner.y));
  canvas->AddImage(atlas_->TexRef, origin, ImVec2(origin.x + side, origin.y + side), place.uv0, place.uv1,
                   IM_COL32_WHITE);
}

std::vector<std::string> Icons::drawn() const {
  std::set<std::string> names;
  for (const auto& [key, id] : slots_) {
    if (key.first != logo_name) {
      names.insert(key.first);
    }
  }
  return {names.begin(), names.end()};
}

}
