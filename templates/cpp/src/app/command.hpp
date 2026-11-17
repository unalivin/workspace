#pragma once

#include <cstdint>
#include <functional>
#include <map>
#include <set>
#include <string>
#include <string_view>
#include <vector>

#include <nlohmann/json_fwd.hpp>

#include "app/cli.hpp"
#include "core/error.hpp"
#include "core/service.hpp"
#include "core/settings.hpp"

namespace app::cli {

[[nodiscard]] AppError usage(std::string message);
[[nodiscard]] AppError failure(std::string message);

struct Globals {
  std::string database;
  settings::Flags overrides;
  std::string ui;
  bool ask = false;
  bool no_prompt = false;
  bool help = false;
  bool version = false;
  bool operator==(const Globals&) const = default;
};

enum class Kind { Switch, Text, Number };

struct Flag {
  std::string_view name;
  Kind kind = Kind::Switch;
  std::string_view fallback;
  std::string_view help;
  bool operator==(const Flag&) const = default;
};

[[nodiscard]] constexpr Flag toggle(std::string_view name, std::string_view help) {
  return {name, Kind::Switch, "false", help};
}

[[nodiscard]] constexpr Flag text_flag(std::string_view name, std::string_view fallback, std::string_view help) {
  return {name, Kind::Text, fallback, help};
}

[[nodiscard]] constexpr Flag number_flag(std::string_view name, std::string_view fallback, std::string_view help) {
  return {name, Kind::Number, fallback, help};
}

struct Parsed {
  std::map<std::string_view, std::string> values;
  std::set<std::string_view> given;
  std::vector<std::string> positional;
};

[[nodiscard]] Parsed parse_flags(const std::vector<Flag>& flags, const std::vector<std::string>& arguments);

struct Call {
  Globals globals;
  std::string parent;
  std::vector<std::string> args;
  std::map<std::string_view, std::string> values;
  std::set<std::string_view> given_flags;

  [[nodiscard]] std::string_view text(std::string_view name) const;
  [[nodiscard]] bool given(std::string_view name) const;
  [[nodiscard]] bool truth(std::string_view name) const;
  [[nodiscard]] std::int64_t number(std::string_view name) const;
  void no_arguments() const;
  [[nodiscard]] std::int64_t identifier(std::string_view what) const;
};

using WithApp = std::function<void(Env&, const Call&, service::App&)>;
using WithoutApp = std::function<void(Env&, const Call&)>;

struct Command {
  std::string_view name;
  std::string_view summary;
  std::string_view args;
  std::vector<Flag> flags;
  std::vector<Command> sub;
  WithApp with_app;
  WithoutApp without_app;

  [[nodiscard]] static Command group(std::string_view name, std::string_view summary, std::vector<Command> sub);
  [[nodiscard]] static Command leaf(std::string_view name, std::string_view summary, WithApp run);
  [[nodiscard]] static Command standalone(std::string_view name, std::string_view summary, WithoutApp run);
  [[nodiscard]] Command taking(std::string_view arguments) &&;
  [[nodiscard]] Command with(std::vector<Flag> list) &&;
  [[nodiscard]] const Command* find(std::string_view child) const;
  [[nodiscard]] bool runnable() const;
};

[[nodiscard]] std::string tabulate(std::string_view content);
void print(Env& env, std::string_view content);
void line(Env& env, std::string_view content);
void note(Env& env, std::string_view content);
void table(Env& env, const std::vector<std::string_view>& header, const std::vector<std::vector<std::string>>& rows);
void json(Env& env, const nlohmann::ordered_json& value);
[[nodiscard]] bool interactive(const Env& env);
void confirm(Env& env, const Call& call, const service::App* app, std::string_view statement);
[[nodiscard]] service::Options app_options(const Env& env, const Globals& globals, bool read_only);
[[nodiscard]] std::string_view enabled_text(bool enabled);
[[nodiscard]] std::string capitalize(std::string_view content);

[[nodiscard]] Command overview_command();
[[nodiscard]] Command records_command();
[[nodiscard]] Command demo_command();
[[nodiscard]] Command notifications_command();
[[nodiscard]] Command notifier_command();
[[nodiscard]] Command config_command();
[[nodiscard]] Command database_command();
void run_diagnostics(Env& env, const Call& call, service::App& app);
void run_guide(Env& env, const Call& call);
void run_about(Env& env, const Call& call, service::App& app);
void run_licenses(Env& env, const Call& call);

}
