#include "core/brand.hpp"

#include <algorithm>
#include <cmath>
#include <cstdint>
#include <fstream>
#include <map>
#include <system_error>

#include "core/error.hpp"
#include "core/files.hpp"
#include "core/text.hpp"

namespace app::brand {

namespace fs = std::filesystem;

namespace {

constexpr int samples = 4;
constexpr double unit = 256.0;

Shape scaled(const Shape& part, double scale) {
  return {part.x * scale, part.y * scale, part.w * scale, part.h * scale, part.r * scale, part.tint};
}

double coverage(double px, double py, const Shape& part) {
  if (px + 1.0 < part.x || px > part.x + part.w || py + 1.0 < part.y || py > part.y + part.h) {
    return 0.0;
  }
  int inside = 0;
  for (int sy = 0; sy < samples; ++sy) {
    for (int sx = 0; sx < samples; ++sx) {
      const double x = px + (sx + 0.5) / samples;
      const double y = py + (sy + 0.5) / samples;
      const double dx = std::max({part.x + part.r - x, x - (part.x + part.w - part.r), 0.0});
      const double dy = std::max({part.y + part.r - y, y - (part.y + part.h - part.r), 0.0});
      const bool within = x >= part.x && x <= part.x + part.w && y >= part.y && y <= part.y + part.h;
      if (within && dx * dx + dy * dy <= part.r * part.r) {
        ++inside;
      }
    }
  }
  return static_cast<double>(inside) / (samples * samples);
}

std::uint8_t channel(double value) {
  return static_cast<std::uint8_t>(std::clamp(std::round(value), 0.0, 255.0));
}

theme::Color blend(theme::Color base, theme::Color top, double share) {
  const auto mix = [share](std::uint8_t from, std::uint8_t to) {
    return channel(static_cast<double>(from) * (1.0 - share) + static_cast<double>(to) * share);
  };
  return {mix(base.r, top.r), mix(base.g, top.g), mix(base.b, top.b)};
}

std::string number(double value) {
  const long long hundredths = std::llround(value * unit * 100.0);
  std::string out = std::to_string(hundredths / 100);
  const long long rest = hundredths % 100;
  if (rest != 0) {
    out.push_back('.');
    out.push_back(static_cast<char>('0' + rest / 10));
    if (rest % 10 != 0) {
      out.push_back(static_cast<char>('0' + rest % 10));
    }
  }
  return out;
}

void rect(std::string& out, const Shape& part) {
  out.append(text::concat("  <rect x=\"", number(part.x), "\" y=\"", number(part.y), "\" width=\"", number(part.w),
                          "\" height=\"", number(part.h), "\" rx=\"", number(part.r), "\" fill=\"", part.tint.hex(),
                          "\"/>\n"));
}

std::string drawing(std::string_view title, std::span<const Shape> shapes) {
  std::string out =
      "<svg xmlns=\"http://www.w3.org/2000/svg\" viewBox=\"0 0 256 256\" width=\"256\" height=\"256\" role=\"img\">\n";
  out.append(text::concat("  <title>", title, "</title>\n"));
  rect(out, plate);
  for (const Shape& part : shapes) {
    rect(out, part);
  }
  out.append("</svg>\n");
  return out;
}

class Pictures {
 public:
  const png::Bytes& at(std::size_t size) {
    const auto found = made_.find(size);
    if (found != made_.end()) {
      return found->second;
    }
    return made_.emplace(size, picture(size)).first->second;
  }

 private:
  std::map<std::size_t, png::Bytes> made_;
};

void little(png::Bytes& out, std::uint32_t value, int width) {
  for (int index = 0; index < width; ++index) {
    out.push_back(static_cast<std::uint8_t>((value >> (8U * static_cast<unsigned>(index))) & 0xFFU));
  }
}

void big(png::Bytes& out, std::uint32_t value) {
  for (int index = 3; index >= 0; --index) {
    out.push_back(static_cast<std::uint8_t>((value >> (8U * static_cast<unsigned>(index))) & 0xFFU));
  }
}

png::Bytes ico_from(Pictures& pictures) {
  png::Bytes out;
  little(out, 0, 2);
  little(out, 1, 2);
  little(out, static_cast<std::uint32_t>(ico_sizes.size()), 2);
  std::size_t offset = 6 + 16 * ico_sizes.size();
  for (const std::size_t size : ico_sizes) {
    const png::Bytes& one = pictures.at(size);
    const auto side = static_cast<std::uint8_t>(size % 256);
    out.insert(out.end(), {side, side, 0, 0});
    little(out, 1, 2);
    little(out, 32, 2);
    little(out, static_cast<std::uint32_t>(one.size()), 4);
    little(out, static_cast<std::uint32_t>(offset), 4);
    offset += one.size();
  }
  for (const std::size_t size : ico_sizes) {
    const png::Bytes& one = pictures.at(size);
    out.insert(out.end(), one.begin(), one.end());
  }
  return out;
}

png::Bytes icns_from(Pictures& pictures) {
  png::Bytes body;
  for (const IcnsEntry& entry : icns_entries) {
    const png::Bytes& one = pictures.at(entry.size);
    const png::Bytes kind = png::bytes_of(entry.kind);
    body.insert(body.end(), kind.begin(), kind.end());
    big(body, static_cast<std::uint32_t>(one.size() + 8));
    body.insert(body.end(), one.begin(), one.end());
  }
  png::Bytes out = png::bytes_of("icns");
  big(out, static_cast<std::uint32_t>(body.size() + 8));
  out.insert(out.end(), body.begin(), body.end());
  return out;
}

}

std::span<const Shape> parts(std::size_t size) {
  if (size <= small_limit) {
    return small_motif;
  }
  return motif;
}

png::Bytes icon(std::size_t size) {
  const auto scale = static_cast<double>(size);
  const Shape base = scaled(plate, scale);
  std::vector<Shape> shapes;
  for (const Shape& part : parts(size)) {
    shapes.push_back(scaled(part, scale));
  }
  png::Bytes pixels(size * size * 4, 0);
  for (std::size_t y = 0; y < size; ++y) {
    for (std::size_t x = 0; x < size; ++x) {
      const auto px = static_cast<double>(x);
      const auto py = static_cast<double>(y);
      const double share = coverage(px, py, base);
      if (share == 0.0) {
        continue;
      }
      theme::Color pixel = background;
      for (const Shape& part : shapes) {
        const double inside = coverage(px, py, part);
        if (inside > 0.0) {
          pixel = blend(pixel, part.tint, inside);
        }
      }
      const std::size_t at = (y * size + x) * 4;
      pixels[at] = pixel.r;
      pixels[at + 1] = pixel.g;
      pixels[at + 2] = pixel.b;
      pixels[at + 3] = channel(share * 255.0);
    }
  }
  return pixels;
}

std::string svg(std::string_view title) {
  return drawing(title, motif);
}

std::string small_svg(std::string_view title) {
  return drawing(title, small_motif);
}

std::string desktop_entry(std::string_view name, std::string_view comment, std::string_view slug) {
  return text::concat("[Desktop Entry]\nType=Application\nVersion=1.0\nName=", name, "\nComment=", comment,
                      "\nExec=", slug, "\nIcon=", slug, "\nTerminal=false\nCategories=Utility;\n");
}

png::Bytes picture(std::size_t size) {
  return png::encode(size, size, icon(size));
}

png::Bytes ico() {
  Pictures pictures;
  return ico_from(pictures);
}

png::Bytes icns() {
  Pictures pictures;
  return icns_from(pictures);
}

std::vector<std::pair<std::string, std::string>> text_files(std::string_view name, std::string_view comment,
                                                            std::string_view slug) {
  return {
      {"icon.svg", svg(text::concat(name, " icon"))},
      {"icon-16.svg", small_svg(text::concat(name, " icon, simplified for small sizes"))},
      {text::concat(slug, ".desktop"), desktop_entry(name, comment, slug)},
      {text::concat("linux/", slug, ".svg"), svg(text::concat(name, " icon"))},
  };
}

std::vector<File> files(std::string_view name, std::string_view comment, std::string_view slug) {
  std::vector<File> list;
  for (auto& [path, content] : text_files(name, comment, slug)) {
    list.push_back({std::move(path), png::bytes_of(content)});
  }
  Pictures pictures;
  list.push_back({"icon.ico", ico_from(pictures)});
  list.push_back({"icon.icns", icns_from(pictures)});
  list.push_back({"icon.png", pictures.at(512)});
  list.push_back({text::concat("linux/", slug, ".png"), pictures.at(256)});
  for (const std::size_t size : sizes) {
    list.push_back({text::concat("png/icon-", size, ".png"), pictures.at(size)});
  }
  std::ranges::sort(list, {}, &File::name);
  return list;
}

std::vector<std::string> write_files(const fs::path& directory, const std::vector<File>& list, bool check) {
  std::vector<std::string> written;
  for (const File& file : list) {
    const fs::path path = directory / files::from_utf8(file.name);
    const std::string shown = files::shown(path);
    const std::string_view wanted(reinterpret_cast<const char*>(file.content.data()), file.content.size());
    if (check) {
      std::string current;
      try {
        current = files::read_all(path);
      } catch (const AppError&) {
        throw AppError::io(text::concat(shown, " is missing: run make assets"));
      }
      if (current != wanted) {
        throw AppError::io(text::concat(shown, " is out of date: run make assets"));
      }
      continue;
    }
    std::error_code problem;
    fs::create_directories(path.parent_path(), problem);
    if (problem) {
      throw AppError::io(text::concat(files::shown(path.parent_path()), ": ", problem.message()));
    }
    std::ofstream out(path, std::ios::binary | std::ios::trunc);
    out.write(wanted.data(), static_cast<std::streamsize>(wanted.size()));
    out.close();
    if (!out) {
      throw AppError::io(text::concat(shown, ": the file could not be written"));
    }
    written.push_back(shown);
  }
  return written;
}

}
