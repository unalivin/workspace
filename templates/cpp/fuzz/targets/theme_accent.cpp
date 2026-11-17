#include <string>

#include "core/theme.hpp"
#include "support/fuzzing.hpp"

extern "C" int LLVMFuzzerTestOneInput(const std::uint8_t* data, std::size_t size) {
  namespace theme = app::theme;
  if (const auto text = fuzzing::text(data, size)) {
    static_cast<void>(theme::Color::parse(*text));
  }
  if (size < 4) {
    return 0;
  }
  const std::string accent = theme::Color{data[0], data[1], data[2]}.hex();
  const theme::Mode mode = data[3] % 2 == 1 ? theme::Mode::Dark : theme::Mode::Light;
  for (const std::string& name : theme::names()) {
    theme::Options options;
    options.name = name;
    options.mode = mode;
    options.accent = accent;
    const theme::Tokens tokens = theme::resolve(options);
    for (const theme::Color surface : {tokens.palette.background, tokens.palette.surface, tokens.palette.surface_alt}) {
      fuzzing::require(theme::contrast(tokens.palette.accent, surface) >= theme::min_text_contrast, "theme_accent",
                       "the accent is unreadable on a surface", accent);
    }
    fuzzing::require(theme::contrast(tokens.on_accent, tokens.palette.accent) >= theme::min_text_contrast,
                     "theme_accent", "text on the accent is unreadable", accent);
  }
  return 0;
}
