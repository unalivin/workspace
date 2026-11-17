#include <doctest/doctest.h>

#include <functional>
#include <set>
#include <string>
#include <variant>
#include <vector>

#include "core/text.hpp"
#include "core/theme.hpp"
#include "support/random.hpp"
#include "support/testkit.hpp"

using app::platform::Env;
using app::theme::Color;
using app::theme::Mode;
using app::theme::Options;
using app::theme::Profile;
using app::theme::Severity;
using app::theme::Tokens;
namespace text = app::text;
namespace theme = app::theme;

namespace {

Tokens resolved(const std::function<void(Options&)>& change) {
  Options options;
  change(options);
  return theme::resolve(options);
}

bool close_to(float left, float right) {
  return left - right < 0.001F && right - left < 0.001F;
}

}

TEST_CASE("every theme meets wcag aa in light and dark") {
  int variants = 0;
  for (const std::string& name : theme::names()) {
    for (const Mode mode : {Mode::Light, Mode::Dark}) {
      ++variants;
      const Tokens tokens = resolved([&](Options& options) {
        options.name = name;
        options.mode = mode;
        options.text_scale = 100;
      });
      const std::string id = text::concat(tokens.name, " dark=", tokens.dark);
      const theme::Palette& colors = tokens.palette;
      for (const Color surface : {colors.background, colors.surface, colors.surface_alt}) {
        CHECK_MESSAGE(theme::contrast(colors.text, surface) >= theme::min_text_contrast, id, " text");
        CHECK_MESSAGE(theme::contrast(colors.text_muted, surface) >= theme::min_text_contrast, id, " muted text");
        CHECK_MESSAGE(theme::contrast(colors.accent, surface) >= theme::min_text_contrast, id, " accent");
        CHECK_MESSAGE(theme::contrast(colors.border, surface) >= theme::min_component_contrast, id, " border");
        for (const Severity severity : theme::all_severities) {
          CHECK_MESSAGE(theme::contrast(tokens.semantic.severity(severity), surface) >= theme::min_text_contrast, id,
                        " severity ", theme::name(severity));
        }
        for (const Color color : {tokens.semantic.success, tokens.semantic.warning, tokens.semantic.error}) {
          CHECK_MESSAGE(theme::contrast(color, surface) >= theme::min_text_contrast, id, " status ", color.hex());
        }
      }
      CHECK_MESSAGE(theme::contrast(tokens.on_accent, colors.accent) >= theme::min_text_contrast, id);
      CHECK_MESSAGE(theme::contrast(colors.text, tokens.selection) >= theme::min_text_contrast, id);
      CHECK_MESSAGE(theme::contrast(tokens.focus, colors.surface) >= theme::min_component_contrast, id);
    }
  }
  CHECK(variants == 14);
}

TEST_CASE("mode follows the system only when asked") {
  const auto system = [](bool dark) { return resolved([dark](Options& options) { options.system_dark = dark; }).dark; };
  CHECK(system(true));
  CHECK_FALSE(system(false));
  CHECK_FALSE(resolved([](Options& options) {
                options.mode = Mode::Light;
                options.system_dark = true;
              }).dark);
  CHECK(resolved([](Options& options) { options.mode = Mode::Dark; }).dark);
  CHECK(theme::parse_mode("dark") == Mode::Dark);
  CHECK(theme::parse_mode("light") == Mode::Light);
  CHECK(theme::parse_mode("anything") == Mode::System);
}

TEST_CASE("a weak custom accent is adjusted and reported") {
  const Tokens tokens = resolved([](Options& options) {
    options.name = "slate";
    options.mode = Mode::Light;
    options.accent = "#FFFF66";
  });
  CHECK(tokens.palette.accent.hex() != "#FFFF66");
  CHECK(theme::contrast(tokens.palette.accent, tokens.palette.surface) >= theme::min_text_contrast);
  REQUIRE(tokens.warnings.size() == 1);
  CHECK_MESSAGE(text::contains(tokens.warnings.front(), "was adjusted"), tokens.warnings.front());
  CHECK_MESSAGE(text::contains(tokens.warnings.front(), "to keep a 4.5:1 contrast"), tokens.warnings.front());
  const Tokens kept = resolved([](Options& options) {
    options.name = "slate";
    options.mode = Mode::Light;
    options.accent = "#1257C4";
  });
  CHECK(kept.palette.accent.hex() == "#1257C4");
  CHECK(kept.warnings.empty());
  const Tokens ignored = resolved([](Options& options) { options.accent = "blue"; });
  REQUIRE(ignored.warnings.size() == 1);
  CHECK(text::contains(ignored.warnings.front(), "ignored"));
}

TEST_CASE("density text scale and unknown names resolve to safe values") {
  const Tokens compact = resolved([](Options& options) {
    options.density = "compact";
    options.text_scale = 150;
  });
  const Tokens comfortable = resolved([](Options& options) {
    options.density = "comfortable";
    options.text_scale = 100;
  });
  CHECK(compact.spacing.m < comfortable.spacing.m);
  CHECK(close_to(compact.type_scale.body, 21.0F));
  CHECK(close_to(compact.type_scale.figure, 42.0F));
  CHECK(close_to(resolved([](Options& options) { options.text_scale = 999; }).type_scale.body, 14.0F));
  const Tokens unknown = resolved([](Options& options) { options.name = "neon"; });
  CHECK(unknown.name == "slate");
  CHECK(unknown.warnings.size() == 1);
  CHECK(close_to(resolved([](Options& options) { options.name = "contrast"; }).radius, 0.0F));
  CHECK(close_to(unknown.radius, 8.0F));
  CHECK_FALSE(resolved([](Options& options) { options.reduced_motion = true; }).motion);
  CHECK(theme::label("contrast") == "High contrast");
  CHECK(theme::label("neon").empty());
  CHECK(theme::names().size() == 7);
}

TEST_CASE("severities are ordered named and never color only") {
  CHECK(theme::severity_names() == std::vector<std::string>{"info", "low", "medium", "high", "critical"});
  CHECK(Severity::Critical > Severity::High);
  CHECK(theme::rank(Severity::High) == 3);
  CHECK(theme::parse_severity(" HIGH ") == Severity::High);
  CHECK_FALSE(theme::parse_severity("urgent").has_value());
  std::set<std::string_view> glyphs;
  for (const Severity severity : theme::all_severities) {
    glyphs.insert(theme::glyph(severity));
    CHECK_FALSE(theme::label(severity).empty());
  }
  CHECK(glyphs.size() == 5);
}

TEST_CASE("terminal profiles are detected from the environment") {
  const auto detect = [](std::vector<std::pair<std::string, std::string>> pairs, bool monochrome) {
    return theme::detect_profile(Env::from_pairs(std::move(pairs)), monochrome);
  };
  CHECK(detect({{"TERM", "xterm-256color"}, {"COLORTERM", "truecolor"}}, false) == Profile::TrueColor);
  CHECK(detect({{"TERM", "xterm-256color"}, {"COLORTERM", "24bit"}}, false) == Profile::TrueColor);
  CHECK(detect({{"TERM", "xterm-256color"}}, false) == Profile::Ansi256);
  CHECK(detect({{"TERM", "xterm"}}, false) == Profile::Ansi16);
  CHECK(detect({{"TERM", "dumb"}}, false) == Profile::Mono);
  CHECK(detect({{"TERM", "xterm-256color"}, {"NO_COLOR", "1"}}, false) == Profile::Mono);
  CHECK(detect({{"TERM", "xterm-256color"}}, true) == Profile::Mono);
  CHECK(detect({{"WT_SESSION", "abc"}}, false) == Profile::TrueColor);
  CHECK(theme::label(Profile::Mono) == "monochrome");
  CHECK(theme::label(Profile::Ansi256) == "256 colors");
}

TEST_CASE("monochrome terminals use attributes instead of color") {
  const Tokens tokens = theme::resolve({});
  const theme::Terminal mono = theme::for_terminal(tokens, Profile::Mono);
  CHECK(mono.selected.reverse);
  CHECK(mono.heading.bold);
  for (const Severity severity : theme::all_severities) {
    const theme::Style& style = mono.severity(severity);
    CHECK(std::holds_alternative<std::monostate>(style.foreground));
    CHECK(std::holds_alternative<std::monostate>(style.background));
  }
  CHECK(mono.severity(Severity::Critical) != mono.severity(Severity::Info));
  const theme::Terminal basic = theme::for_terminal(tokens, Profile::Ansi16);
  CHECK(std::holds_alternative<theme::Ansi>(basic.accent.foreground));
  const theme::Terminal dark_basic = theme::for_terminal(resolved([](Options& options) {
                                                           options.mode = Mode::Dark;
                                                           options.name = "green";
                                                         }),
                                                         Profile::Ansi16);
  CHECK(dark_basic.accent.foreground == theme::TermColor(theme::Ansi{10}));
  CHECK(dark_basic.severity(Severity::Critical).foreground == theme::TermColor(theme::Ansi{9}));
  const theme::Terminal full = theme::for_terminal(tokens, Profile::TrueColor);
  CHECK(full.text.foreground == theme::TermColor(tokens.palette.text));
  CHECK(full.severity(Severity::High).foreground == theme::TermColor(tokens.semantic.high));
  CHECK(full.selected.background == theme::TermColor(tokens.palette.accent));
}

TEST_CASE("colors parse print and map to the 256 color cube") {
  CHECK(Color::parse("#1257c4").value().hex() == "#1257C4");
  CHECK(Color::parse(" 1257C4 ") == Color{0x12, 0x57, 0xC4});
  for (const char* bad : {"", "#12", "#GGGGGG", "#1257C4FF", "+12345", "##12345"}) {
    CHECK_FALSE_MESSAGE(Color::parse(bad).has_value(), bad);
  }
  CHECK(theme::contrast(theme::white, theme::black) > 20.999);
  CHECK(theme::contrast(theme::white, theme::black) < 21.001);
  CHECK(theme::best_on(theme::black) == theme::white);
  CHECK(theme::best_on(theme::white) == theme::black);
  CHECK(theme::mix(theme::black, theme::white, 0.5) == Color{128, 128, 128});
  CHECK(theme::ansi256(theme::black) == 16);
  CHECK(theme::ansi256(theme::white) == 231);
  CHECK(theme::ansi256(Color{255, 0, 0}) == 196);
}

TEST_CASE("any accent ends up readable and color parsing survives noise") {
  testkit::Random random(31);
  for (int round = 0; round < 300; ++round) {
    const Color accent{static_cast<std::uint8_t>(random.between(0, 255)),
                       static_cast<std::uint8_t>(random.between(0, 255)),
                       static_cast<std::uint8_t>(random.between(0, 255))};
    const Mode mode = round % 2 == 0 ? Mode::Dark : Mode::Light;
    for (const std::string& name : theme::names()) {
      const Tokens tokens = resolved([&](Options& options) {
        options.name = name;
        options.mode = mode;
        options.accent = accent.hex();
      });
      for (const Color surface : {tokens.palette.background, tokens.palette.surface, tokens.palette.surface_alt}) {
        CHECK(theme::contrast(tokens.palette.accent, surface) >= theme::min_text_contrast);
      }
      CHECK(theme::contrast(tokens.on_accent, tokens.palette.accent) >= theme::min_text_contrast);
    }
  }
  for (int round = 0; round < 3000; ++round) {
    const std::string noise = round % 2 == 0 ? random.bytes(12) : random.text(8, "#0123456789abcdefABCDEFg ");
    if (const auto parsed = Color::parse(noise)) {
      CHECK(Color::parse(parsed->hex()) == parsed);
    }
  }
}
