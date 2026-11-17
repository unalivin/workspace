#pragma once

#include <cstddef>
#include <cstdint>
#include <optional>
#include <string>
#include <string_view>
#include <utility>
#include <variant>
#include <vector>

#include "core/service.hpp"

namespace app::gui {

struct RemoveRecord {
  std::int64_t id = 0;
  bool operator==(const RemoveRecord&) const = default;
};

struct RemoveDemo {
  bool operator==(const RemoveDemo&) const = default;
};

struct LoadDemo {
  bool operator==(const LoadDemo&) const = default;
};

struct InstallNotifier {
  bool operator==(const InstallNotifier&) const = default;
};

struct UninstallNotifier {
  bool operator==(const UninstallNotifier&) const = default;
};

struct MoveDatabase {
  service::MovePlan plan;
  bool operator==(const MoveDatabase&) const = default;
};

struct ImportFile {
  std::string path;
  std::string format;
  bool operator==(const ImportFile&) const = default;
};

struct OpenLink {
  service::Link link;
  bool operator==(const OpenLink&) const = default;
};

using Action = std::variant<RemoveRecord, RemoveDemo, LoadDemo, InstallNotifier, UninstallNotifier, MoveDatabase,
                            ImportFile, OpenLink>;

enum class SubmitKind { AddRecord, EditRecord, Columns, Import, Export, Rule, MoveDatabase };

struct Submit {
  SubmitKind kind = SubmitKind::AddRecord;
  std::int64_t id = 0;
  std::int64_t revision = 0;
  std::string name;
  bool operator==(const Submit&) const = default;
};

enum class Verdict { Open, Accepted, Cancelled };

using Choices = std::vector<std::pair<std::string, std::string>>;

struct Field {
  std::string key;
  std::string label;
  std::string value;
  std::string help;
  Choices options;
  bool multiline = false;
  std::size_t limit = 0;

  [[nodiscard]] static Field text(std::string_view key, std::string_view label, std::string_view value,
                                  std::string_view help, std::size_t limit);
  [[nodiscard]] static Field area(std::string_view key, std::string_view label, std::string_view value,
                                  std::string_view help, std::size_t limit);
  [[nodiscard]] static Field choice(std::string_view key, std::string_view label, std::string_view value,
                                    std::string_view help, Choices options);
  [[nodiscard]] static Field yes_no(std::string_view key, std::string_view label, bool value, std::string_view help);
  [[nodiscard]] std::string_view shown() const;
  bool operator==(const Field&) const = default;
};

struct Form {
  std::string title;
  std::string accept;
  std::vector<Field> fields;
  std::string problem;
  Submit submit;
  bool fresh = true;

  Form(std::string heading, std::string_view accept_label, std::vector<Field> inputs, Submit target);
  [[nodiscard]] std::string value(std::string_view key) const;
  void set(std::string_view key, std::string_view value);
  bool operator==(const Form&) const = default;
};

struct Confirm {
  std::string title;
  std::string statement;
  std::string accept;
  std::string cancel = "Cancel";
  bool danger = false;
  std::optional<Action> action;

  Confirm(std::string_view heading, std::string text, std::string_view accept_label, std::optional<Action> target);
  bool operator==(const Confirm&) const = default;
};

}
