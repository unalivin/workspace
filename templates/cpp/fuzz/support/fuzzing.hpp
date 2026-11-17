#pragma once

#include <cstddef>
#include <cstdint>
#include <optional>
#include <string_view>

namespace fuzzing {

[[noreturn]] void fail(std::string_view target, std::string_view message, std::string_view detail);
void require(bool holds, std::string_view target, std::string_view message, std::string_view detail = {});
[[nodiscard]] std::string_view bytes(const std::uint8_t* data, std::size_t size);
[[nodiscard]] std::optional<std::string_view> text(const std::uint8_t* data, std::size_t size);

}

extern "C" int LLVMFuzzerTestOneInput(const std::uint8_t* data, std::size_t size);
