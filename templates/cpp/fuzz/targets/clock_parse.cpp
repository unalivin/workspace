#include "core/clock.hpp"
#include "support/fuzzing.hpp"

extern "C" int LLVMFuzzerTestOneInput(const std::uint8_t* data, std::size_t size) {
  if (const auto text = fuzzing::text(data, size)) {
    static_cast<void>(app::clock::Timestamp::parse(*text));
  }
  return 0;
}
