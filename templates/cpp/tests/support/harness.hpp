#pragma once

#include <map>
#include <optional>
#include <string>
#include <string_view>
#include <vector>

#include "core/launcher.hpp"
#include "core/platform.hpp"
#include "core/service.hpp"
#include "support/testkit.hpp"

namespace testkit {

struct Outcome {
  std::string output;
  std::string errors;
  int code = 0;
};

class Harness {
 public:
  explicit Harness(std::string_view label);

  void set(std::string_view name, std::string_view value);
  void unset(std::string_view name);
  [[nodiscard]] app::platform::Env environment() const;
  [[nodiscard]] app::service::Options options() const;
  [[nodiscard]] std::string path(std::string_view name) const;
  [[nodiscard]] Outcome run(const std::vector<std::string>& arguments);
  std::string ok(const std::vector<std::string>& arguments);
  std::string fails(const std::vector<std::string>& arguments, int code, std::string_view fragment);
  [[nodiscard]] std::vector<std::string> opened();

  TempDir home;
  std::map<std::string, std::string, std::less<>> variables;
  std::string standard_input;
  bool terminal = false;
  std::optional<std::vector<std::string>> gui;
  std::optional<std::vector<std::string>> tui;
  std::optional<app::launcher::PromptResult> chooser;
  bool window = false;
  std::vector<std::string> delivered;

 private:
  [[nodiscard]] std::string visit(std::string_view name, const std::vector<std::string>& script);

  std::vector<std::string> opened_;
};

}
