#pragma once

#include <cstdint>
#include <map>
#include <optional>
#include <string>
#include <string_view>
#include <utility>
#include <vector>

#include "core/clock.hpp"
#include "core/platform.hpp"
#include "core/store.hpp"

namespace app::settings {

enum class Kind { Enum, Bool, Int, Color, Text, Columns, Sort };
enum class Source { Default, Stored, Environment, Flag };

[[nodiscard]] std::string_view label(Kind kind);
[[nodiscard]] std::string_view label(Source source);

inline constexpr std::string_view key_mode = "appearance.mode";
inline constexpr std::string_view key_theme = "appearance.theme";
inline constexpr std::string_view key_accent = "appearance.accent";
inline constexpr std::string_view key_density = "appearance.density";
inline constexpr std::string_view key_text_scale = "appearance.text_scale";
inline constexpr std::string_view key_reduced_motion = "appearance.reduced_motion";
inline constexpr std::string_view key_monochrome = "appearance.monochrome";
inline constexpr std::string_view key_time_zone = "display.time_zone";
inline constexpr std::string_view key_sidebar_collapsed = "window.sidebar_collapsed";
inline constexpr std::string_view key_window_width = "window.width";
inline constexpr std::string_view key_window_height = "window.height";
inline constexpr std::string_view key_refresh_seconds = "data.refresh_seconds";
inline constexpr std::string_view key_confirm = "safety.confirm_destructive";
inline constexpr std::string_view key_notifier_interval = "notifier.interval_seconds";
inline constexpr std::string_view key_notifier_titles = "notifier.show_titles";
inline constexpr std::string_view key_record_columns = "table.records.columns";
inline constexpr std::string_view key_record_sort = "table.records.sort";

struct Definition {
  std::string_view key;
  std::string_view group;
  std::string_view label;
  std::string_view help;
  Kind kind = Kind::Text;
  std::string_view fallback;
  std::vector<std::string_view> options;
  std::vector<std::string_view> pinned;
  std::int64_t min = 0;
  std::int64_t max = 0;
  bool operator==(const Definition&) const = default;
};

using Flags = std::map<std::string, std::string, std::less<>>;

struct Value {
  Definition definition;
  std::string key;
  std::string raw;
  std::optional<std::string> stored;
  Source source = Source::Default;
  std::string overridden_by;
  std::string override_error;
  bool unknown = false;

  [[nodiscard]] bool flag() const;
  [[nodiscard]] std::int64_t number() const;
  [[nodiscard]] bool locked() const;
  bool operator==(const Value&) const = default;
};

[[nodiscard]] const std::vector<Definition>& definitions();
[[nodiscard]] const std::vector<std::string_view>& record_columns();
[[nodiscard]] const Definition* lookup(std::string_view key);
[[nodiscard]] std::string env_name(std::string_view key);
[[nodiscard]] std::vector<std::string> split_columns(std::string_view value);
[[nodiscard]] std::pair<std::string, bool> split_sort(std::string_view value);
[[nodiscard]] std::string normalize(const Definition& definition, std::string_view raw);
[[nodiscard]] std::vector<std::string_view> groups(const std::vector<Value>& values);

class Settings {
 public:
  Settings(const store::Store& database, clock::Clock clock, platform::Env env, const Flags& flags);

  [[nodiscard]] Value get(std::string_view key) const;
  [[nodiscard]] std::vector<Value> list() const;
  Value set(std::string_view key, std::string_view raw) const;
  Value reset(std::string_view key) const;
  [[nodiscard]] std::map<std::string, Value, std::less<>> effective() const;
  [[nodiscard]] std::string text(std::string_view key) const;
  [[nodiscard]] bool flag(std::string_view key) const;
  [[nodiscard]] std::int64_t number(std::string_view key) const;

 private:
  [[nodiscard]] Value resolve(const Definition& definition, std::optional<std::string> stored) const;

  const store::Store* database_;
  clock::Clock clock_;
  platform::Env env_;
  const Flags* flags_;
};

}
