#include <string_view>

#include "core/error.hpp"
#include "core/transfer.hpp"
#include "support/fuzzing.hpp"

extern "C" int LLVMFuzzerTestOneInput(const std::uint8_t* data, std::size_t size) {
  const std::string_view raw = fuzzing::bytes(data, size);
  for (const app::transfer::Format format : {app::transfer::Format::Csv, app::transfer::Format::Json}) {
    try {
      static_cast<void>(app::transfer::parse(raw, format));
    } catch (const app::AppError&) {
      continue;
    }
  }
  return 0;
}
