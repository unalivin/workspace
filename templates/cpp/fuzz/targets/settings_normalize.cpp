#include <string>
#include <vector>

#include "core/error.hpp"
#include "core/settings.hpp"
#include "support/fuzzing.hpp"

extern "C" int LLVMFuzzerTestOneInput(const std::uint8_t* data, std::size_t size) {
  if (size == 0) {
    return 0;
  }
  const auto raw = fuzzing::text(data + 1, size - 1);
  if (!raw) {
    return 0;
  }
  const std::vector<app::settings::Definition>& definitions = app::settings::definitions();
  const app::settings::Definition& definition = definitions[data[0] % definitions.size()];
  std::string once;
  try {
    once = app::settings::normalize(definition, *raw);
  } catch (const app::AppError&) {
    return 0;
  }
  try {
    const std::string twice = app::settings::normalize(definition, once);
    fuzzing::require(twice == once, "settings_normalize", "normalizing twice changes the value", once);
  } catch (const app::AppError& problem) {
    fuzzing::fail("settings_normalize", "a normalized value stays valid", problem.message());
  }
  return 0;
}
