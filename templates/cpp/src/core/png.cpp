#include "core/png.hpp"

#include <algorithm>
#include <array>
#include <optional>

#include "core/error.hpp"

namespace app::png {

namespace {

constexpr std::array<std::uint8_t, 8> signature = {0x89, 'P', 'N', 'G', 0x0D, 0x0A, 0x1A, 0x0A};
constexpr std::array<std::size_t, 29> length_base = {3,  4,  5,  6,  7,  8,  9,  10, 11,  13,  15,  17,  19,  23, 27,
                                                     31, 35, 43, 51, 59, 67, 83, 99, 115, 131, 163, 195, 227, 258};
constexpr std::array<std::uint32_t, 29> length_extra = {0, 0, 0, 0, 0, 0, 0, 0, 1, 1, 1, 1, 2, 2, 2,
                                                        2, 3, 3, 3, 3, 4, 4, 4, 4, 5, 5, 5, 5, 0};
constexpr std::array<std::size_t, 30> distance_base = {1,    2,    3,    4,    5,    7,    9,    13,    17,    25,
                                                       33,   49,   65,   97,   129,  193,  257,  385,   513,   769,
                                                       1025, 1537, 2049, 3073, 4097, 6145, 8193, 12289, 16385, 24577};
constexpr std::array<std::uint32_t, 30> distance_extra = {0, 0, 0, 0, 1, 1, 2, 2,  3,  3,  4,  4,  5,  5,  6,
                                                          6, 7, 7, 8, 8, 9, 9, 10, 10, 11, 11, 12, 12, 13, 13};
constexpr std::size_t max_match = 258;
constexpr std::size_t min_match = 3;
constexpr std::uint32_t end_of_block = 256;
constexpr std::uint8_t filter_none = 0;
constexpr std::uint8_t filter_sub = 1;
constexpr std::uint8_t filter_up = 2;
constexpr std::size_t pixel = 4;
constexpr std::array<std::uint8_t, 5> rgba_layout = {8, 6, 0, 0, 0};

AppError damaged(std::string_view reason) {
  return AppError::io(std::string(reason));
}

std::uint8_t low_byte(std::uint32_t value) {
  return static_cast<std::uint8_t>(value & 0xFFU);
}

template <std::size_t Size>
std::size_t slot_for(const std::array<std::size_t, Size>& bases, std::size_t value) {
  std::size_t slot = 0;
  for (std::size_t index = 0; index < Size; ++index) {
    if (bases.at(index) <= value) {
      slot = index;
    }
  }
  return slot;
}

class Writer {
 public:
  void push(std::uint32_t value, std::uint32_t count) {
    pending_ |= value << count_;
    count_ += count;
    while (count_ >= 8) {
      bytes_.push_back(low_byte(pending_));
      pending_ >>= 8U;
      count_ -= 8;
    }
  }

  void code(std::uint32_t value, std::uint32_t count) {
    for (std::uint32_t bit = count; bit > 0; --bit) {
      push((value >> (bit - 1)) & 1U, 1);
    }
  }

  void symbol(std::uint32_t value) {
    if (value <= 143) {
      code(0x30 + value, 8);
    } else if (value <= 255) {
      code(0x190 + value - 144, 9);
    } else if (value <= 279) {
      code(value - 256, 7);
    } else {
      code(0xC0 + value - 280, 8);
    }
  }

  void repeat(std::size_t length, std::size_t distance) {
    const std::size_t length_slot = slot_for(length_base, length);
    symbol(257 + static_cast<std::uint32_t>(length_slot));
    push(static_cast<std::uint32_t>(length - length_base.at(length_slot)), length_extra.at(length_slot));
    const std::size_t distance_slot = slot_for(distance_base, distance);
    code(static_cast<std::uint32_t>(distance_slot), 5);
    push(static_cast<std::uint32_t>(distance - distance_base.at(distance_slot)), distance_extra.at(distance_slot));
  }

  Bytes finish() {
    if (count_ > 0) {
      bytes_.push_back(low_byte(pending_));
    }
    return std::move(bytes_);
  }

 private:
  Bytes bytes_;
  std::uint32_t pending_ = 0;
  std::uint32_t count_ = 0;
};

class Reader {
 public:
  explicit Reader(std::span<const std::uint8_t> data) : data_(data) {}

  std::uint32_t bit() {
    if (at_ / 8 >= data_.size()) {
      throw damaged("the compressed data ends early");
    }
    const std::uint32_t value = (static_cast<std::uint32_t>(data_[at_ / 8]) >> (at_ % 8)) & 1U;
    ++at_;
    return value;
  }

  std::uint32_t bits(std::uint32_t count) {
    std::uint32_t value = 0;
    for (std::uint32_t place = 0; place < count; ++place) {
      value |= bit() << place;
    }
    return value;
  }

  std::uint32_t code(std::uint32_t count) {
    std::uint32_t value = 0;
    for (std::uint32_t round = 0; round < count; ++round) {
      value = (value << 1U) | bit();
    }
    return value;
  }

  std::uint32_t symbol() {
    std::uint32_t found = code(7);
    if (found <= 0x17) {
      return 256 + found;
    }
    found = (found << 1U) | bit();
    if (found >= 0x30 && found <= 0xBF) {
      return found - 0x30;
    }
    if (found >= 0xC0 && found <= 0xC7) {
      return 280 + found - 0xC0;
    }
    found = (found << 1U) | bit();
    return 144 + found - 0x190;
  }

 private:
  std::span<const std::uint8_t> data_;
  std::size_t at_ = 0;
};

void append_number(Bytes& out, std::uint32_t value) {
  out.push_back(low_byte(value >> 24U));
  out.push_back(low_byte(value >> 16U));
  out.push_back(low_byte(value >> 8U));
  out.push_back(low_byte(value));
}

void chunk(Bytes& out, std::string_view kind, std::span<const std::uint8_t> body) {
  const Bytes name = bytes_of(kind);
  append_number(out, static_cast<std::uint32_t>(body.size()));
  out.insert(out.end(), name.begin(), name.end());
  out.insert(out.end(), body.begin(), body.end());
  append_number(out, crc32(name, body));
}

std::uint32_t number(std::span<const std::uint8_t> data, std::size_t at) {
  if (data.size() < 4 || at > data.size() - 4) {
    throw damaged("the picture ends early");
  }
  return (static_cast<std::uint32_t>(data[at]) << 24U) | (static_cast<std::uint32_t>(data[at + 1]) << 16U) |
         (static_cast<std::uint32_t>(data[at + 2]) << 8U) | static_cast<std::uint32_t>(data[at + 3]);
}

bool named(std::span<const std::uint8_t> kind, std::string_view name) {
  return std::equal(kind.begin(), kind.end(), name.begin(), name.end(),
                    [](std::uint8_t byte, char letter) { return byte == static_cast<std::uint8_t>(letter); });
}

}

Bytes bytes_of(std::string_view text) {
  Bytes out;
  out.reserve(text.size());
  for (const char letter : text) {
    out.push_back(static_cast<std::uint8_t>(letter));
  }
  return out;
}

std::uint32_t crc32(std::span<const std::uint8_t> first, std::span<const std::uint8_t> second) {
  std::uint32_t crc = 0xFFFFFFFFU;
  for (const std::span<const std::uint8_t> part : {first, second}) {
    for (const std::uint8_t byte : part) {
      crc ^= byte;
      for (int round = 0; round < 8; ++round) {
        crc = (crc & 1U) == 1U ? (crc >> 1U) ^ 0xEDB88320U : crc >> 1U;
      }
    }
  }
  return ~crc;
}

std::uint32_t adler32(std::span<const std::uint8_t> data) {
  std::uint32_t low = 1;
  std::uint32_t high = 0;
  for (const std::uint8_t byte : data) {
    low = (low + byte) % 65521U;
    high = (high + low) % 65521U;
  }
  return (high << 16U) | low;
}

Bytes deflate(std::span<const std::uint8_t> data) {
  Writer out;
  out.push(1, 1);
  out.push(1, 2);
  std::size_t at = 0;
  while (at < data.size()) {
    std::size_t best = 0;
    std::size_t reach = 0;
    for (const std::size_t distance : {std::size_t{1}, pixel}) {
      if (at < distance) {
        continue;
      }
      std::size_t length = 0;
      while (length < max_match && at + length < data.size() && data[at + length] == data[at + length - distance]) {
        ++length;
      }
      if (length > best) {
        best = length;
        reach = distance;
      }
    }
    if (best >= min_match) {
      out.repeat(best, reach);
      at += best;
    } else {
      out.symbol(data[at]);
      ++at;
    }
  }
  out.symbol(end_of_block);
  return out.finish();
}

Bytes inflate(std::span<const std::uint8_t> data, std::size_t limit) {
  Reader reader(data);
  Bytes out;
  const auto room = [&out, limit](std::size_t wanted) {
    if (wanted > limit - out.size()) {
      throw damaged("the compressed data is longer than expected");
    }
  };
  while (true) {
    const bool last = reader.bit() == 1;
    if (reader.bits(2) != 1) {
      throw damaged("only blocks with fixed codes are supported");
    }
    while (true) {
      const std::uint32_t symbol = reader.symbol();
      if (symbol < end_of_block) {
        room(1);
        out.push_back(low_byte(symbol));
        continue;
      }
      if (symbol == end_of_block) {
        break;
      }
      const std::size_t length_slot = symbol - 257;
      if (length_slot >= length_base.size()) {
        throw damaged("the compressed data has an unknown length code");
      }
      const std::size_t length = length_base.at(length_slot) + reader.bits(length_extra.at(length_slot));
      const std::size_t distance_slot = reader.code(5);
      if (distance_slot >= distance_base.size()) {
        throw damaged("the compressed data has an unknown distance code");
      }
      const std::size_t distance = distance_base.at(distance_slot) + reader.bits(distance_extra.at(distance_slot));
      if (distance > out.size()) {
        throw damaged("the compressed data points before its start");
      }
      room(length);
      for (std::size_t copied = 0; copied < length; ++copied) {
        out.push_back(out[out.size() - distance]);
      }
    }
    if (last) {
      return out;
    }
  }
}

Bytes encode(std::size_t width, std::size_t height, std::span<const std::uint8_t> rgba) {
  if (width > max_dimension || height > max_dimension || rgba.size() != width * height * pixel) {
    throw AppError::usage("the picture size does not match its pixels");
  }
  const std::size_t stride = width * pixel;
  Bytes raw;
  raw.reserve((stride + 1) * height);
  for (std::size_t row = 0; row < height; ++row) {
    raw.push_back(filter_up);
    for (std::size_t column = 0; column < stride; ++column) {
      const std::uint8_t above = row == 0 ? std::uint8_t{0} : rgba[(row - 1) * stride + column];
      raw.push_back(static_cast<std::uint8_t>(rgba[row * stride + column] - above));
    }
  }
  Bytes packed = {0x78, 0x01};
  const Bytes body = deflate(raw);
  packed.insert(packed.end(), body.begin(), body.end());
  append_number(packed, adler32(raw));
  Bytes header;
  append_number(header, static_cast<std::uint32_t>(width));
  append_number(header, static_cast<std::uint32_t>(height));
  header.insert(header.end(), rgba_layout.begin(), rgba_layout.end());
  Bytes out(signature.begin(), signature.end());
  chunk(out, "IHDR", header);
  chunk(out, "IDAT", packed);
  chunk(out, "IEND", {});
  return out;
}

Picture decode(std::span<const std::uint8_t> data) {
  if (data.size() < signature.size() || !std::equal(signature.begin(), signature.end(), data.begin())) {
    throw damaged("this is not a PNG picture");
  }
  std::optional<Picture> picture;
  Bytes packed;
  std::size_t at = signature.size();
  while (true) {
    const std::size_t length = number(data, at);
    if (data.size() - at < 12 || length > data.size() - at - 12) {
      throw damaged("the picture ends early");
    }
    const std::span<const std::uint8_t> kind = data.subspan(at + 4, 4);
    const std::span<const std::uint8_t> body = data.subspan(at + 8, length);
    if (number(data, at + 8 + length) != crc32(kind, body)) {
      throw damaged("a checksum of the picture is wrong");
    }
    at += 12 + length;
    if (named(kind, "IHDR")) {
      if (picture) {
        throw damaged("the picture has two headers");
      }
      picture.emplace();
      picture->width = number(body, 0);
      picture->height = number(body, 4);
      if (body.size() != 13 || !std::equal(rgba_layout.begin(), rgba_layout.end(), body.begin() + 8)) {
        throw damaged("only 8 bit RGBA pictures without interlacing are supported");
      }
      if (picture->width > max_dimension || picture->height > max_dimension) {
        throw damaged("the picture is too large");
      }
    } else if (named(kind, "IDAT")) {
      packed.insert(packed.end(), body.begin(), body.end());
    } else if (named(kind, "IEND")) {
      break;
    }
  }
  if (!picture) {
    throw damaged("the picture has no header");
  }
  if (packed.size() < 6 || packed.front() != 0x78) {
    throw damaged("the picture data is not a zlib stream");
  }
  const std::size_t stride = picture->width * pixel;
  const std::size_t expected = (stride + 1) * picture->height;
  const Bytes raw = inflate(std::span<const std::uint8_t>(packed).subspan(2, packed.size() - 6), expected);
  if (number(packed, packed.size() - 4) != adler32(raw)) {
    throw damaged("the checksum of the picture data is wrong");
  }
  if (raw.size() != expected) {
    throw damaged("the picture data has the wrong length");
  }
  picture->rgba.assign(stride * picture->height, 0);
  Bytes& rgba = picture->rgba;
  for (std::size_t row = 0; row < picture->height; ++row) {
    const std::uint8_t filter = raw[row * (stride + 1)];
    if (filter != filter_none && filter != filter_sub && filter != filter_up) {
      throw damaged("the picture uses an unsupported row filter");
    }
    for (std::size_t column = 0; column < stride; ++column) {
      std::uint8_t before = 0;
      if (filter == filter_sub && column >= pixel) {
        before = rgba[row * stride + column - pixel];
      } else if (filter == filter_up && row > 0) {
        before = rgba[(row - 1) * stride + column];
      }
      rgba[row * stride + column] = static_cast<std::uint8_t>(raw[row * (stride + 1) + 1 + column] + before);
    }
  }
  return std::move(*picture);
}

}
