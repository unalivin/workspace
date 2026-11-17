#include <doctest/doctest.h>

#include <array>
#include <cstdint>
#include <set>
#include <string>
#include <vector>

#include "core/brand.hpp"
#include "core/error.hpp"
#include "core/files.hpp"
#include "core/meta.hpp"
#include "core/png.hpp"
#include "core/text.hpp"
#include "support/random.hpp"
#include "support/testkit.hpp"

using app::AppError;
using app::png::Bytes;
namespace brand = app::brand;
namespace files = app::files;
namespace meta = app::meta;
namespace png = app::png;
namespace text = app::text;

namespace {

Bytes noise(std::size_t count) {
  std::uint32_t state = 0x2545F491U;
  Bytes out;
  out.reserve(count);
  for (std::size_t index = 0; index < count; ++index) {
    state ^= state << 13U;
    state ^= state >> 17U;
    state ^= state << 5U;
    out.push_back(static_cast<std::uint8_t>(state >> 24U));
  }
  return out;
}

Bytes repeated(const Bytes& pattern, std::size_t times) {
  Bytes out;
  for (std::size_t index = 0; index < times; ++index) {
    out.insert(out.end(), pattern.begin(), pattern.end());
  }
  return out;
}

std::array<std::uint8_t, 4> pixel(const Bytes& picture, std::size_t size, std::size_t x, std::size_t y) {
  const std::size_t at = (y * size + x) * 4;
  return {picture[at], picture[at + 1], picture[at + 2], picture[at + 3]};
}

std::size_t number(const Bytes& data, std::size_t at, bool little) {
  const std::array<std::size_t, 4> bytes = {data.at(at), data.at(at + 1), data.at(at + 2), data.at(at + 3)};
  return little ? bytes[0] | (bytes[1] << 8U) | (bytes[2] << 16U) | (bytes[3] << 24U)
                : bytes[3] | (bytes[2] << 8U) | (bytes[1] << 16U) | (bytes[0] << 24U);
}

Bytes slice(const Bytes& data, std::size_t from, std::size_t count) {
  return {data.begin() + static_cast<std::ptrdiff_t>(from), data.begin() + static_cast<std::ptrdiff_t>(from + count)};
}

std::size_t occurrences(std::string_view whole, std::string_view part) {
  std::size_t count = 0;
  for (std::size_t at = whole.find(part); at != std::string_view::npos; at = whole.find(part, at + part.size())) {
    ++count;
  }
  return count;
}

bool rejected(const Bytes& data) {
  try {
    static_cast<void>(png::decode(data));
  } catch (const AppError&) {
    return true;
  }
  return false;
}

}

TEST_CASE("checksums match the published values") {
  CHECK(png::crc32(png::bytes_of("123456789")) == 0xCBF43926U);
  CHECK(png::crc32(png::bytes_of("1234"), png::bytes_of("56789")) == 0xCBF43926U);
  CHECK(png::crc32(png::bytes_of("IEND")) == 0xAE426082U);
  CHECK(png::adler32(png::bytes_of("Wikipedia")) == 0x11E60398U);
  CHECK(png::adler32({}) == 1);
}

TEST_CASE("the empty stream is the known fixed block") {
  CHECK(png::deflate({}) == Bytes{0x03, 0x00});
  CHECK(png::deflate(png::bytes_of("a")) == Bytes{0x4B, 0x04, 0x00});
  CHECK(png::inflate(Bytes{0x03, 0x00}, 10).empty());
  CHECK(png::inflate(Bytes{0x4B, 0x04, 0x00}, 10) == png::bytes_of("a"));
}

TEST_CASE("deflate round trips runs literals and every byte value") {
  Bytes every;
  for (int value = 0; value <= 255; ++value) {
    every.push_back(static_cast<std::uint8_t>(value));
  }
  Bytes mixed = noise(300);
  mixed.insert(mixed.end(), 400, 200);
  const Bytes tail = noise(10);
  mixed.insert(mixed.end(), tail.begin(), tail.end());
  const std::vector<Bytes> samples = {
      Bytes(1000, 7), every, repeated({1, 2, 3, 4}, 300), noise(5000), Bytes(258 * 3 + 2, 0), Bytes{9, 9}, mixed};
  for (const Bytes& sample : samples) {
    const Bytes packed = png::deflate(sample);
    CHECK(png::inflate(packed, sample.size()) == sample);
    if (!sample.empty()) {
      CHECK_THROWS_AS(static_cast<void>(png::inflate(packed, sample.size() - 1)), AppError);
    }
  }
  CHECK(png::deflate(Bytes(100000, 0)).size() < 1000);
}

TEST_CASE("pictures round trip and flat areas shrink") {
  const std::size_t width = 37;
  const std::size_t height = 21;
  Bytes rgba = noise(width * height * 4);
  for (std::size_t at = width * 5 * 4; at < rgba.size(); at += 4) {
    rgba[at] = 31;
    rgba[at + 1] = 58;
    rgba[at + 2] = 95;
    rgba[at + 3] = 255;
  }
  const Bytes file = png::encode(width, height, rgba);
  CHECK(slice(file, 0, 8) == Bytes{0x89, 'P', 'N', 'G', 0x0D, 0x0A, 0x1A, 0x0A});
  CHECK(slice(file, 12, 4) == png::bytes_of("IHDR"));
  CHECK(slice(file, file.size() - 12, 12) == Bytes{0, 0, 0, 0, 'I', 'E', 'N', 'D', 0xAE, 0x42, 0x60, 0x82});
  CHECK(png::decode(file) == png::Picture{width, height, rgba});
  const Bytes flat = png::encode(256, 256, repeated({31, 58, 95, 255}, 256 * 256));
  CHECK_MESSAGE(flat.size() < 4000, flat.size(), " bytes");
  CHECK(png::decode(png::encode(0, 0, {})) == png::Picture{});
  CHECK_THROWS_AS(static_cast<void>(png::encode(2, 2, Bytes(15, 0))), AppError);
}

TEST_CASE("damaged pictures are rejected") {
  const Bytes file = png::encode(4, 4, Bytes(64, 1));
  CHECK(rejected(slice(file, 0, 20)));
  CHECK(rejected(png::bytes_of("not a picture")));
  Bytes flipped = file;
  flipped[flipped.size() - 20] ^= 0x40U;
  CHECK(rejected(flipped));
  Bytes gray = file;
  gray[25] = 0;
  CHECK(rejected(gray));
  for (std::size_t length = 0; length < file.size(); ++length) {
    CHECK(rejected(slice(file, 0, length)));
  }
  testkit::Random random(91);
  for (int round = 0; round < 3000; ++round) {
    Bytes changed = file;
    const std::size_t at = static_cast<std::size_t>(random.between(0, static_cast<std::int64_t>(file.size()) - 1));
    changed[at] = static_cast<std::uint8_t>(random.between(0, 255));
    if (changed == file) {
      continue;
    }
    try {
      const png::Picture picture = png::decode(changed);
      CHECK(picture.rgba.size() == picture.width * picture.height * 4);
    } catch (const AppError& error) {
      CHECK_FALSE(error.message().empty());
    }
  }
}

TEST_CASE("the icon has the plate and the motif at every size") {
  for (const std::size_t size : brand::sizes) {
    if (size > 256) {
      continue;
    }
    const Bytes picture = brand::icon(size);
    CHECK(picture.size() == size * size * 4);
    CHECK_MESSAGE(pixel(picture, size, 0, 0)[3] == 0, "size ", size, ": the corner is not transparent");
    const std::array<std::uint8_t, 4> plate = {brand::background.r, brand::background.g, brand::background.b, 255};
    CHECK_MESSAGE(pixel(picture, size, size / 2, size / 8) == plate, "size ", size, ": the plate");
    std::set<std::array<std::uint8_t, 3>> tints;
    for (std::size_t at = 0; at < picture.size(); at += 4) {
      tints.insert({picture[at], picture[at + 1], picture[at + 2]});
    }
    for (const app::theme::Color wanted : {brand::foreground, brand::accent}) {
      const std::array<std::uint8_t, 3> tint = {wanted.r, wanted.g, wanted.b};
      CHECK_MESSAGE(tints.contains(tint), "size ", size, " lacks ", wanted.hex());
    }
  }
}

TEST_CASE("small sizes use the simplified motif") {
  CHECK(brand::parts(16).size() < brand::parts(32).size());
  CHECK(occurrences(brand::small_svg("x"), "<rect") == brand::small_motif.size() + 1);
  CHECK(occurrences(brand::svg("x"), "<rect") == brand::motif.size() + 1);
}

TEST_CASE("the svg sources are plain shapes with a title") {
  for (const std::string& source : {brand::svg("Icon"), brand::small_svg("Icon")}) {
    CHECK(source.starts_with("<svg xmlns=\"http://www.w3.org/2000/svg\""));
    CHECK(source.ends_with("</svg>\n"));
    CHECK(text::contains(source, "<title>Icon</title>"));
    CHECK_FALSE(text::contains(source, "<image"));
    CHECK(occurrences(source, "<") == occurrences(source, ">"));
  }
  CHECK(text::contains(
      brand::svg("Icon"),
      "<rect x=\"10.24\" y=\"10.24\" width=\"235.52\" height=\"235.52\" rx=\"51.2\" fill=\"#1F3A5F\"/>"));
  CHECK(text::contains(brand::svg("Icon"), "<rect x=\"56.32\" y=\"66.56\" width=\"25.6\" height=\"25.6\" rx=\"12.8\""));
}

TEST_CASE("the png pictures decode to the drawn pixels") {
  for (const std::size_t size : {16U, 24U, 32U, 48U, 64U}) {
    const png::Picture decoded = png::decode(brand::picture(size));
    CHECK(decoded.width == size);
    CHECK(decoded.height == size);
    CHECK(decoded.rgba == brand::icon(size));
  }
  CHECK_MESSAGE(brand::picture(256).size() < 40000, brand::picture(256).size(), " bytes at 256 pixels");
}

TEST_CASE("the ico file lists valid pictures") {
  const Bytes data = brand::ico();
  CHECK(slice(data, 0, 6) == Bytes{0, 0, 1, 0, 4, 0});
  for (std::size_t index = 0; index < brand::ico_sizes.size(); ++index) {
    const std::size_t entry = 6 + 16 * index;
    const std::size_t length = number(data, entry + 8, true);
    const std::size_t offset = number(data, entry + 12, true);
    const png::Picture decoded = png::decode(slice(data, offset, length));
    const std::size_t listed = data[entry] == 0 ? 256 : data[entry];
    CHECK(decoded.width == brand::ico_sizes.at(index));
    CHECK(decoded.height == brand::ico_sizes.at(index));
    CHECK(listed == brand::ico_sizes.at(index));
  }
}

TEST_CASE("the committed assets match the generator") {
  const std::vector<brand::File> all = brand::files(meta::app_name, meta::tagline, meta::slug);
  CHECK(all.size() == 8 + brand::sizes.size());
  CHECK(brand::write_files(testkit::source_root() / "assets", all, true).empty());
  const Bytes* icns = nullptr;
  for (const brand::File& file : all) {
    if (file.name == "icon.icns") {
      icns = &file.content;
    }
  }
  REQUIRE(icns != nullptr);
  const Bytes& data = *icns;
  CHECK(slice(data, 0, 4) == png::bytes_of("icns"));
  CHECK(number(data, 4, false) == data.size());
  std::vector<std::string> found;
  std::size_t position = 8;
  while (position < data.size()) {
    const std::size_t length = number(data, position + 4, false);
    CHECK(slice(data, position + 8, 8) == Bytes{0x89, 'P', 'N', 'G', 0x0D, 0x0A, 0x1A, 0x0A});
    const Bytes kind = slice(data, position, 4);
    found.emplace_back(kind.begin(), kind.end());
    position += length;
  }
  CHECK(position == data.size());
  std::vector<std::string> wanted;
  for (const brand::IcnsEntry& entry : brand::icns_entries) {
    wanted.emplace_back(entry.kind);
  }
  CHECK(found == wanted);
  const std::string entry = brand::text_files(meta::app_name, meta::tagline, meta::slug).at(2).second;
  for (const std::string& line : {text::concat("Name=", meta::app_name), text::concat("Exec=", meta::slug),
                                  text::concat("Icon=", meta::slug), std::string("Type=Application")}) {
    CHECK_MESSAGE(text::contains(entry, text::concat(line, "\n")), "the desktop entry lacks ", line);
  }
}

TEST_CASE("the generator writes and checks every file") {
  const testkit::TempDir folder("brand");
  const std::vector<brand::File> list = {{"a.txt", png::bytes_of("one")}, {"deep/b.bin", {0, 1, 2}}};
  CHECK(text::contains(testkit::failure_of([&] { brand::write_files(folder.path(), list, true); }),
                       "is missing: run make assets"));
  CHECK(brand::write_files(folder.path(), list, false).size() == 2);
  CHECK(brand::write_files(folder.path(), list, true).empty());
  files::write_private(folder.path() / "deep" / "b.bin", "\x09", true);
  CHECK(text::contains(testkit::failure_of([&] { brand::write_files(folder.path(), list, true); }),
                       "is out of date: run make assets"));
}
