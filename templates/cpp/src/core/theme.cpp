#include "core/theme.hpp"

#include <algorithm>
#include <cmath>
#include <limits>

#include "core/text.hpp"

namespace app::theme {

namespace {

constexpr Color rgb(std::uint32_t value) {
  return Color{static_cast<std::uint8_t>((value >> 16U) & 0xFFU), static_cast<std::uint8_t>((value >> 8U) & 0xFFU),
               static_cast<std::uint8_t>(value & 0xFFU)};
}

constexpr Palette palette(const std::array<std::uint32_t, 8>& values) {
  return Palette{rgb(values[0]), rgb(values[1]), rgb(values[2]), rgb(values[3]),
                 rgb(values[4]), rgb(values[5]), rgb(values[6]), rgb(values[7])};
}

struct Definition {
  std::string_view name;
  std::string_view label;
  Palette light;
  Palette dark;
};

constexpr std::array<Definition, 7> themes = {{
    {"slate", "Neutral slate",
     palette({0xF6F7F8, 0xFFFFFF, 0xECEEF1, 0x6E7681, 0xDDE0E4, 0x16191D, 0x535B66, 0x2459C9}),
     palette({0x121416, 0x191C1F, 0x23272B, 0x7F8894, 0x2D3237, 0xECEEF0, 0xA3ABB5, 0x84ADFF})},
    {"blue", "Blue", palette({0xF5F7F9, 0xFFFFFF, 0xEBEEF2, 0x6D7682, 0xDCE0E5, 0x16191D, 0x535B66, 0x1257C4}),
     palette({0x121416, 0x191C1F, 0x23262B, 0x7F8894, 0x2D3137, 0xECEEF0, 0xA5ABB3, 0x6FA8FF})},
    {"green", "Green", palette({0xF5F9F7, 0xFFFFFF, 0xEBF2EE, 0x6D8276, 0xDCE5E0, 0x161D19, 0x53665B, 0x136B38}),
     palette({0x121614, 0x191F1C, 0x232B26, 0x7F9488, 0x2D3731, 0xECF0EE, 0xA5B3AB, 0x5FD08A})},
    {"crimson", "Crimson", palette({0xF9F5F6, 0xFFFFFF, 0xF2EBEC, 0x826D70, 0xE5DCDD, 0x1D1617, 0x665356, 0xA8112F}),
     palette({0x161213, 0x1F191A, 0x2B2324, 0x947F82, 0x372D2F, 0xF0ECED, 0xB3A5A8, 0xFF7A90})},
    {"amber", "Amber", palette({0xF9F7F5, 0xFFFFFF, 0xF2EFEB, 0x827A6D, 0xE5E2DC, 0x1D1A16, 0x665F53, 0x8A5200}),
     palette({0x161512, 0x1F1D19, 0x2B2823, 0x948C7F, 0x37332D, 0xF0EEEC, 0xB3AEA5, 0xF2B544})},
    {"purple", "Purple", palette({0xF7F5F9, 0xFFFFFF, 0xEEEBF2, 0x756D82, 0xE0DCE5, 0x19161D, 0x5B5366, 0x6230C2}),
     palette({0x141216, 0x1B191F, 0x26232B, 0x877F94, 0x312D37, 0xEEECF0, 0xABA5B3, 0xB79BFF})},
    {"contrast", "High contrast",
     palette({0xFFFFFF, 0xFFFFFF, 0xE6E6E6, 0x000000, 0x000000, 0x000000, 0x1A1A1A, 0x0000C8}),
     palette({0x000000, 0x000000, 0x1F1F1F, 0xFFFFFF, 0xFFFFFF, 0xFFFFFF, 0xE6E6E6, 0xFFD60A})},
}};

constexpr Semantic light_semantic{rgb(0xC01439), rgb(0x8E2E0B), rgb(0x7F600A), rgb(0x1E5FC4),
                                  rgb(0x59626E), rgb(0x16713D), rgb(0x7F600A), rgb(0xC01439)};

constexpr Semantic dark_semantic{rgb(0xFF7B88), rgb(0xFFA057), rgb(0xF2CC60), rgb(0x7AB7FF),
                                 rgb(0xB6BEC9), rgb(0x56D364), rgb(0xE3B341), rgb(0xFF7B88)};

double channel(std::uint8_t value) {
  const double scaled = static_cast<double>(value) / 255.0;
  return scaled <= 0.04045 ? scaled / 12.92 : std::pow((scaled + 0.055) / 1.055, 2.4);
}

double min_contrast(Color color, const std::vector<Color>& against) {
  double least = std::numeric_limits<double>::infinity();
  for (const Color other : against) {
    least = std::min(least, contrast(color, other));
  }
  return least;
}

int hex_digit(char letter) {
  if (letter >= '0' && letter <= '9') {
    return letter - '0';
  }
  if (letter >= 'a' && letter <= 'f') {
    return letter - 'a' + 10;
  }
  if (letter >= 'A' && letter <= 'F') {
    return letter - 'A' + 10;
  }
  return -1;
}

std::size_t theme_index(std::string_view wanted) {
  for (std::size_t index = 0; index < themes.size(); ++index) {
    if (themes[index].name == wanted) {
      return index;
    }
  }
  return themes.size();
}

Style fg(TermColor color) {
  Style style;
  style.foreground = color;
  return style;
}

Style bold(Style style) {
  style.bold = true;
  return style;
}

Style underline(Style style) {
  style.underline = true;
  return style;
}

Style on(Style style, TermColor background) {
  style.background = background;
  return style;
}

std::uint8_t ansi_accent(std::string_view theme_name, bool dark) {
  constexpr std::array<std::uint8_t, 7> dark_codes = {7, 12, 10, 9, 11, 13, 15};
  constexpr std::array<std::uint8_t, 7> light_codes = {8, 4, 2, 1, 3, 5, 0};
  std::size_t index = theme_index(theme_name);
  if (index == themes.size()) {
    index = 0;
  }
  return dark ? dark_codes[index] : light_codes[index];
}

std::string one_decimal(double value) {
  const auto tenths = static_cast<std::int64_t>(std::llround(value * 10.0));
  return text::concat(tenths / 10, ".", tenths % 10);
}

}

std::optional<Color> Color::parse(std::string_view value) {
  std::string_view digits = text::trim(value);
  if (!digits.empty() && digits.front() == '#') {
    digits.remove_prefix(1);
  }
  if (digits.size() != 6) {
    return std::nullopt;
  }
  std::uint32_t number = 0;
  for (const char letter : digits) {
    const int digit = hex_digit(letter);
    if (digit < 0) {
      return std::nullopt;
    }
    number = number * 16U + static_cast<std::uint32_t>(digit);
  }
  return rgb(number);
}

std::string Color::parse_problem(std::string_view value) {
  return text::concat("\"", value, "\" is not a color: use the form #RRGGBB");
}

std::string Color::hex() const {
  constexpr std::string_view digits = "0123456789ABCDEF";
  std::string out = "#";
  for (const std::uint8_t part : {r, g, b}) {
    out.push_back(digits[part >> 4U]);
    out.push_back(digits[part & 0x0FU]);
  }
  return out;
}

double luminance(Color color) {
  return 0.2126 * channel(color.r) + 0.7152 * channel(color.g) + 0.0722 * channel(color.b);
}

double contrast(Color first, Color second) {
  const double a = luminance(first);
  const double b = luminance(second);
  return (std::max(a, b) + 0.05) / (std::min(a, b) + 0.05);
}

Color mix(Color from, Color to, double share) {
  const auto blend = [share](std::uint8_t x, std::uint8_t y) {
    const double value = std::round(static_cast<double>(x) * (1.0 - share) + static_cast<double>(y) * share);
    return static_cast<std::uint8_t>(std::clamp(value, 0.0, 255.0));
  };
  return Color{blend(from.r, to.r), blend(from.g, to.g), blend(from.b, to.b)};
}

Color best_on(Color background) {
  return contrast(white, background) >= contrast(black, background) ? white : black;
}

std::pair<Color, bool> adjust_for_contrast(Color color, const std::vector<Color>& against, double minimum) {
  if (min_contrast(color, against) >= minimum) {
    return {color, false};
  }
  const Color target = min_contrast(black, against) > min_contrast(white, against) ? black : white;
  for (int step = 1; step <= 100; ++step) {
    const Color candidate = mix(color, target, static_cast<double>(step) / 100.0);
    if (min_contrast(candidate, against) >= minimum) {
      return {candidate, true};
    }
  }
  return {target, true};
}

std::optional<Severity> parse_severity(std::string_view text) {
  const std::string wanted = text::lower(text::trim(text));
  for (const Severity severity : all_severities) {
    if (name(severity) == wanted) {
      return severity;
    }
  }
  return std::nullopt;
}

std::string_view name(Severity severity) {
  switch (severity) {
    case Severity::Info:
      return "info";
    case Severity::Low:
      return "low";
    case Severity::Medium:
      return "medium";
    case Severity::High:
      return "high";
    case Severity::Critical:
      return "critical";
  }
  return "info";
}

std::string_view label(Severity severity) {
  switch (severity) {
    case Severity::Info:
      return "Info";
    case Severity::Low:
      return "Low";
    case Severity::Medium:
      return "Medium";
    case Severity::High:
      return "High";
    case Severity::Critical:
      return "Critical";
  }
  return "Info";
}

std::string_view glyph(Severity severity) {
  switch (severity) {
    case Severity::Info:
      return "i ";
    case Severity::Low:
      return "- ";
    case Severity::Medium:
      return "* ";
    case Severity::High:
      return "! ";
    case Severity::Critical:
      return "!!";
  }
  return "i ";
}

std::size_t rank(Severity severity) {
  return static_cast<std::size_t>(severity);
}

std::vector<std::string> severity_names() {
  std::vector<std::string> out;
  for (const Severity severity : all_severities) {
    out.emplace_back(name(severity));
  }
  return out;
}

Color Semantic::severity(Severity level) const {
  switch (level) {
    case Severity::Critical:
      return critical;
    case Severity::High:
      return high;
    case Severity::Medium:
      return medium;
    case Severity::Low:
      return low;
    case Severity::Info:
      return info;
  }
  return info;
}

Mode parse_mode(std::string_view text) {
  if (text == "light") {
    return Mode::Light;
  }
  if (text == "dark") {
    return Mode::Dark;
  }
  return Mode::System;
}

std::vector<std::string> names() {
  std::vector<std::string> out;
  for (const Definition& definition : themes) {
    out.emplace_back(definition.name);
  }
  return out;
}

std::string_view label(std::string_view theme_name) {
  const std::size_t index = theme_index(theme_name);
  return index == themes.size() ? std::string_view() : themes[index].label;
}

Tokens resolve(const Options& options) {
  std::vector<std::string> warnings;
  std::size_t index = theme_index(options.name);
  if (index == themes.size()) {
    if (!options.name.empty()) {
      warnings.push_back(text::concat("unknown theme \"", options.name, "\", using ", themes[0].name));
    }
    index = 0;
  }
  const Definition& selected = themes[index];
  const bool dark = options.mode == Mode::Dark || (options.mode != Mode::Light && options.system_dark);
  Palette colors = dark ? selected.dark : selected.light;
  const Semantic semantic = dark ? dark_semantic : light_semantic;
  if (!options.accent.empty()) {
    const std::optional<Color> custom = Color::parse(options.accent);
    if (!custom) {
      warnings.push_back(text::concat("custom accent ignored: ", Color::parse_problem(options.accent)));
    } else {
      const auto [adjusted, changed] =
          adjust_for_contrast(*custom, {colors.background, colors.surface, colors.surface_alt}, min_text_contrast);
      if (changed) {
        warnings.push_back(text::concat("custom accent ", custom->hex(), " was adjusted to ", adjusted.hex(),
                                        " to keep a ", one_decimal(min_text_contrast), ":1 contrast"));
      }
      colors.accent = adjusted;
    }
  }
  const float scale =
      options.text_scale >= 80 && options.text_scale <= 200 ? static_cast<float>(options.text_scale) / 100.0F : 1.0F;
  const Spacing spacing = options.density == "compact" ? Spacing{4.0F, 4.0F, 8.0F, 12.0F, 16.0F, 24.0F * scale}
                                                       : Spacing{4.0F, 8.0F, 12.0F, 16.0F, 24.0F, 32.0F * scale};
  Tokens tokens;
  tokens.name = selected.name;
  tokens.label = selected.label;
  tokens.dark = dark;
  tokens.palette = colors;
  tokens.on_accent = best_on(colors.accent);
  tokens.focus = colors.accent;
  tokens.selection = mix(colors.surface, colors.accent, 0.12);
  tokens.semantic = semantic;
  tokens.spacing = spacing;
  tokens.type_scale = TypeScale{12.0F * scale, 14.0F * scale, 16.0F * scale, 24.0F * scale, 28.0F * scale};
  tokens.radius = selected.name == "contrast" ? 0.0F : 8.0F;
  tokens.motion = !options.reduced_motion;
  tokens.warnings = std::move(warnings);
  return tokens;
}

std::string_view label(Profile profile) {
  switch (profile) {
    case Profile::TrueColor:
      return "truecolor";
    case Profile::Ansi256:
      return "256 colors";
    case Profile::Ansi16:
      return "16 colors";
    case Profile::Mono:
      return "monochrome";
  }
  return "monochrome";
}

Profile detect_profile(const platform::Env& env, bool monochrome) {
  const std::string term = text::lower(env.get("TERM").value_or(""));
  if (monochrome || env.get("NO_COLOR").has_value() || term == "dumb") {
    return Profile::Mono;
  }
  const std::string color_term = text::lower(env.get("COLORTERM").value_or(""));
  if (color_term == "truecolor" || color_term == "24bit" || env.get("WT_SESSION").has_value()) {
    return Profile::TrueColor;
  }
  return text::contains(term, "256color") ? Profile::Ansi256 : Profile::Ansi16;
}

const Style& Terminal::severity(Severity level) const {
  return severities.at(rank(level));
}

Terminal for_terminal(const Tokens& tokens, Profile profile) {
  const Style plain;
  Style reverse;
  reverse.reverse = true;
  Terminal out;
  out.profile = profile;
  if (profile == Profile::Mono) {
    out.text = plain;
    out.muted = plain;
    out.accent = bold(plain);
    out.heading = underline(bold(plain));
    out.selected = reverse;
    out.border = plain;
    out.status_bar = reverse;
    out.success = plain;
    out.warning = bold(plain);
    out.error = underline(bold(plain));
    out.severities = {plain, plain, bold(plain), bold(plain), underline(bold(plain))};
    return out;
  }
  if (profile == Profile::Ansi16) {
    const TermColor accent = Ansi{ansi_accent(tokens.name, tokens.dark)};
    const auto ansi = [](std::uint8_t code) { return fg(Ansi{code}); };
    const std::uint8_t critical = tokens.dark ? 9 : 1;
    const std::uint8_t high = tokens.dark ? 11 : 5;
    const std::uint8_t medium = 3;
    const std::uint8_t low = tokens.dark ? 12 : 4;
    const std::uint8_t success = tokens.dark ? 10 : 2;
    const std::uint8_t selected_foreground = tokens.dark ? 0 : 15;
    out.text = plain;
    out.muted = ansi(8);
    out.accent = bold(fg(accent));
    out.heading = bold(fg(accent));
    out.selected = bold(on(ansi(selected_foreground), accent));
    out.border = ansi(8);
    out.status_bar = reverse;
    out.success = ansi(success);
    out.warning = bold(ansi(medium));
    out.error = bold(ansi(critical));
    out.severities = {plain, ansi(low), ansi(medium), bold(ansi(high)), bold(ansi(critical))};
    return out;
  }
  const Palette& colors = tokens.palette;
  const auto full = [](Color color) { return fg(color); };
  out.text = full(colors.text);
  out.muted = full(colors.text_muted);
  out.accent = bold(full(colors.accent));
  out.heading = bold(full(colors.accent));
  out.selected = bold(on(full(tokens.on_accent), colors.accent));
  out.border = full(colors.border);
  out.status_bar = on(full(colors.text), colors.surface_alt);
  out.success = full(tokens.semantic.success);
  out.warning = bold(full(tokens.semantic.warning));
  out.error = bold(full(tokens.semantic.error));
  out.severities = {full(tokens.semantic.info), full(tokens.semantic.low), full(tokens.semantic.medium),
                    bold(full(tokens.semantic.high)), bold(full(tokens.semantic.critical))};
  return out;
}

std::uint8_t ansi256(Color color) {
  const auto cube = [](std::uint8_t value) -> int {
    if (value < 48) {
      return 0;
    }
    if (value < 115) {
      return 1;
    }
    return (static_cast<int>(value) - 35) / 40;
  };
  return static_cast<std::uint8_t>(16 + 36 * cube(color.r) + 6 * cube(color.g) + cube(color.b));
}

}
