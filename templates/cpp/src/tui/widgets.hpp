#pragma once

#include <cstddef>
#include <string>
#include <string_view>
#include <utility>
#include <variant>
#include <vector>

#include "tui/text.hpp"

namespace app::tui {

enum class KeyKind {
  Char,
  Ctrl,
  Function,
  Enter,
  Esc,
  Tab,
  BackTab,
  Up,
  Down,
  Left,
  Right,
  Home,
  End,
  PageUp,
  PageDown,
  Backspace,
  Delete
};

struct Key {
  KeyKind kind = KeyKind::Char;
  char32_t value = 0;

  [[nodiscard]] static constexpr Key letter(char32_t value) {
    return {KeyKind::Char, value};
  }
  [[nodiscard]] static constexpr Key ctrl(char32_t value) {
    return {KeyKind::Ctrl, value};
  }
  [[nodiscard]] static constexpr Key function(unsigned number) {
    return {KeyKind::Function, char32_t{number}};
  }
  [[nodiscard]] constexpr bool is(char32_t wanted) const {
    return kind == KeyKind::Char && value == wanted;
  }
  [[nodiscard]] constexpr bool is_ctrl(char32_t wanted) const {
    return kind == KeyKind::Ctrl && value == wanted;
  }
  [[nodiscard]] constexpr bool is_function(unsigned number) const {
    return kind == KeyKind::Function && value == char32_t{number};
  }
  bool operator==(const Key&) const = default;
};

namespace keys {

inline constexpr Key enter{KeyKind::Enter};
inline constexpr Key esc{KeyKind::Esc};
inline constexpr Key tab{KeyKind::Tab};
inline constexpr Key back_tab{KeyKind::BackTab};
inline constexpr Key up{KeyKind::Up};
inline constexpr Key down{KeyKind::Down};
inline constexpr Key left{KeyKind::Left};
inline constexpr Key right{KeyKind::Right};
inline constexpr Key home{KeyKind::Home};
inline constexpr Key end{KeyKind::End};
inline constexpr Key page_up{KeyKind::PageUp};
inline constexpr Key page_down{KeyKind::PageDown};
inline constexpr Key backspace{KeyKind::Backspace};
inline constexpr Key del{KeyKind::Delete};

}

class Input {
 public:
  Input() = default;
  Input(std::string_view value, std::size_t limit);

  [[nodiscard]] std::string value() const;
  bool handle(Key key);
  [[nodiscard]] std::string view(std::size_t size, bool focused) const;
  bool operator==(const Input&) const = default;

 private:
  std::u32string letters_;
  std::size_t cursor_ = 0;
  std::size_t limit_ = 0;
};

using Option = std::pair<std::string, std::string>;

struct Choice {
  std::vector<Option> options;
  std::size_t chosen = 0;
  bool operator==(const Choice&) const = default;
};

struct Field {
  std::string key;
  std::string label;
  std::string help;
  std::variant<Input, Choice> control;

  [[nodiscard]] static Field text(std::string_view key, std::string_view label, std::string_view value,
                                  std::string_view help, std::size_t limit);
  [[nodiscard]] static Field choice(std::string_view key, std::string_view label, std::string_view value,
                                    std::string_view help, std::vector<Option> options);
  [[nodiscard]] static Field yes_no(std::string_view key, std::string_view label, bool value, std::string_view help);
  [[nodiscard]] std::string value() const;
  bool operator==(const Field&) const = default;
};

enum class FormEvent { Open, Submit, Cancel };

inline constexpr std::string_view form_hint =
    "Tab next field · Left/Right change a choice · Enter or Ctrl+S save · Esc cancel";

struct Form {
  std::string title;
  std::vector<Field> fields;
  std::size_t focus = 0;
  std::string problem;

  Form() = default;
  Form(std::string heading, std::vector<Field> inputs);
  [[nodiscard]] std::string value(std::string_view key) const;
  FormEvent handle(Key key);
  [[nodiscard]] Lines view(std::size_t size) const;
  bool operator==(const Form&) const = default;
};

struct Column {
  std::size_t width = 0;
  bool flexible = false;
};

[[nodiscard]] std::vector<std::size_t> layout(const std::vector<Column>& columns, std::size_t size);
[[nodiscard]] std::string row(const std::vector<std::string>& cells, const std::vector<std::size_t>& widths);
[[nodiscard]] std::string tail(std::string_view text, std::size_t size);
[[nodiscard]] std::string clip_start(std::string_view text, std::size_t size);
[[nodiscard]] Line selectable(std::string_view text, bool selected, std::size_t size);
[[nodiscard]] std::size_t step(std::size_t current, std::ptrdiff_t delta, std::size_t count);

}
