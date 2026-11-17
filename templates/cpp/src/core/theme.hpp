#pragma once

#include <array>
#include <cstdint>
#include <optional>
#include <string>
#include <string_view>
#include <utility>
#include <variant>
#include <vector>

#include "core/platform.hpp"

namespace app::theme {

inline constexpr double min_text_contrast = 4.5;
inline constexpr double min_component_contrast = 3.0;

struct Color {
  std::uint8_t r = 0;
  std::uint8_t g = 0;
  std::uint8_t b = 0;

  [[nodiscard]] static std::optional<Color> parse(std::string_view value);
  [[nodiscard]] static std::string parse_problem(std::string_view value);
  [[nodiscard]] std::string hex() const;
  bool operator==(const Color&) const = default;
};

inline constexpr Color white{255, 255, 255};
inline constexpr Color black{0, 0, 0};

[[nodiscard]] double luminance(Color color);
[[nodiscard]] double contrast(Color first, Color second);
[[nodiscard]] Color mix(Color from, Color to, double share);
[[nodiscard]] Color best_on(Color background);
[[nodiscard]] std::pair<Color, bool> adjust_for_contrast(Color color, const std::vector<Color>& against,
                                                         double minimum);

enum class Severity { Info, Low, Medium, High, Critical };

inline constexpr std::array<Severity, 5> all_severities = {Severity::Info, Severity::Low, Severity::Medium,
                                                           Severity::High, Severity::Critical};

[[nodiscard]] std::optional<Severity> parse_severity(std::string_view text);
[[nodiscard]] std::string_view name(Severity severity);
[[nodiscard]] std::string_view label(Severity severity);
[[nodiscard]] std::string_view glyph(Severity severity);
[[nodiscard]] std::size_t rank(Severity severity);
[[nodiscard]] std::vector<std::string> severity_names();

struct Palette {
  Color background;
  Color surface;
  Color surface_alt;
  Color border;
  Color divider;
  Color text;
  Color text_muted;
  Color accent;
  bool operator==(const Palette&) const = default;
};

struct Semantic {
  Color critical;
  Color high;
  Color medium;
  Color low;
  Color info;
  Color success;
  Color warning;
  Color error;

  [[nodiscard]] Color severity(Severity level) const;
  bool operator==(const Semantic&) const = default;
};

struct Spacing {
  float xs = 0;
  float s = 0;
  float m = 0;
  float l = 0;
  float xl = 0;
  float row_height = 0;
  bool operator==(const Spacing&) const = default;
};

struct TypeScale {
  float caption = 0;
  float body = 0;
  float subheading = 0;
  float heading = 0;
  float figure = 0;
  bool operator==(const TypeScale&) const = default;
};

struct Tokens {
  std::string_view name;
  std::string_view label;
  bool dark = false;
  Palette palette;
  Color on_accent;
  Color focus;
  Color selection;
  Semantic semantic;
  Spacing spacing;
  TypeScale type_scale;
  float radius = 0;
  bool motion = true;
  std::vector<std::string> warnings;
  bool operator==(const Tokens&) const = default;
};

enum class Mode { System, Light, Dark };

[[nodiscard]] Mode parse_mode(std::string_view text);

struct Options {
  std::string name;
  Mode mode = Mode::System;
  bool system_dark = false;
  std::string accent;
  std::string density;
  std::int64_t text_scale = 0;
  bool reduced_motion = false;
  bool operator==(const Options&) const = default;
};

[[nodiscard]] std::vector<std::string> names();
[[nodiscard]] std::string_view label(std::string_view name);
[[nodiscard]] Tokens resolve(const Options& options);

enum class Profile { Mono, Ansi16, Ansi256, TrueColor };

[[nodiscard]] std::string_view label(Profile profile);
[[nodiscard]] Profile detect_profile(const platform::Env& env, bool monochrome);

struct Ansi {
  std::uint8_t code = 0;
  bool operator==(const Ansi&) const = default;
};

using TermColor = std::variant<std::monostate, Ansi, Color>;

struct Style {
  TermColor foreground;
  TermColor background;
  bool bold = false;
  bool underline = false;
  bool reverse = false;
  bool operator==(const Style&) const = default;
};

struct Terminal {
  Profile profile = Profile::Mono;
  Style text;
  Style muted;
  Style accent;
  Style heading;
  Style selected;
  Style border;
  Style status_bar;
  Style success;
  Style warning;
  Style error;
  std::array<Style, 5> severities;

  [[nodiscard]] const Style& severity(Severity level) const;
  bool operator==(const Terminal&) const = default;
};

[[nodiscard]] Terminal for_terminal(const Tokens& tokens, Profile profile);
[[nodiscard]] std::uint8_t ansi256(Color color);

}
