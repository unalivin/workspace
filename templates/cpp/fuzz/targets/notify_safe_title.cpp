#include <algorithm>
#include <string>

#include "core/notify.hpp"
#include "core/text.hpp"
#include "support/fuzzing.hpp"

extern "C" int LLVMFuzzerTestOneInput(const std::uint8_t* data, std::size_t size) {
  const auto text = fuzzing::text(data, size);
  if (!text) {
    return 0;
  }
  const std::string cleaned = app::notify::safe_title(*text);
  const std::u32string letters = app::text::decode(cleaned);
  fuzzing::require(!cleaned.empty(), "notify_safe_title", "the title is empty");
  fuzzing::require(letters.size() <= app::notify::max_title_length, "notify_safe_title", "the title is too long",
                   cleaned);
  fuzzing::require(std::none_of(letters.begin(), letters.end(), app::text::is_control), "notify_safe_title",
                   "the title keeps a control character");
  return 0;
}
