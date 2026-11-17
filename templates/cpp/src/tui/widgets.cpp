#include "tui/widgets.hpp"

#include <algorithm>

#include "core/text.hpp"

namespace app::tui {

namespace {

constexpr std::size_t min_flexible = 8;

}

Input::Input(std::string_view value, std::size_t limit) : letters_(text::decode(value)), limit_(limit) {
  cursor_ = letters_.size();
}

std::string Input::value() const {
  return text::encode(letters_);
}

bool Input::handle(Key key) {
  if (key == keys::left) {
    cursor_ = cursor_ > 0 ? cursor_ - 1 : 0;
  } else if (key == keys::right) {
    cursor_ = std::min(cursor_ + 1, letters_.size());
  } else if (key == keys::home || key.is_ctrl(U'a')) {
    cursor_ = 0;
  } else if (key == keys::end || key.is_ctrl(U'e')) {
    cursor_ = letters_.size();
  } else if (key == keys::backspace) {
    if (cursor_ > 0) {
      --cursor_;
      letters_.erase(cursor_, 1);
    }
  } else if (key == keys::del) {
    if (cursor_ < letters_.size()) {
      letters_.erase(cursor_, 1);
    }
  } else if (key.is_ctrl(U'u')) {
    letters_.erase(0, cursor_);
    cursor_ = 0;
  } else if (key.kind == KeyKind::Char) {
    if (!text::is_control(key.value) && (limit_ == 0 || letters_.size() < limit_)) {
      letters_.insert(cursor_, 1, key.value);
      ++cursor_;
    }
  } else {
    return false;
  }
  return true;
}

std::string Input::view(std::size_t size, bool focused) const {
  if (!focused) {
    return pad(value(), size);
  }
  const std::u32string_view letters = letters_;
  const std::string after = text::encode(letters.substr(cursor_));
  std::size_t start = 0;
  while (true) {
    const std::string shown = text::concat(text::encode(letters.substr(start, cursor_ - start)), "▏", after);
    if (width(shown) <= size || start >= cursor_) {
      return pad(shown, size);
    }
    ++start;
  }
}

Field Field::text(std::string_view key, std::string_view label, std::string_view value, std::string_view help,
                  std::size_t limit) {
  return {std::string(key), std::string(label), std::string(help), Input(value, limit)};
}

Field Field::choice(std::string_view key, std::string_view label, std::string_view value, std::string_view help,
                    std::vector<Option> options) {
  const auto found = std::ranges::find(options, value, &Option::first);
  const std::size_t chosen = found == options.end() ? 0 : static_cast<std::size_t>(found - options.begin());
  return {std::string(key), std::string(label), std::string(help), Choice{std::move(options), chosen}};
}

Field Field::yes_no(std::string_view key, std::string_view label, bool value, std::string_view help) {
  return choice(key, label, value ? "true" : "false", help, {{"true", "Yes"}, {"false", "No"}});
}

std::string Field::value() const {
  if (const auto* input = std::get_if<Input>(&control)) {
    return input->value();
  }
  const auto& choice = std::get<Choice>(control);
  return choice.options.at(choice.chosen).first;
}

Form::Form(std::string heading, std::vector<Field> inputs) : title(std::move(heading)), fields(std::move(inputs)) {}

std::string Form::value(std::string_view key) const {
  const auto found = std::ranges::find(fields, key, &Field::key);
  return found == fields.end() ? std::string() : found->value();
}

FormEvent Form::handle(Key key) {
  const std::size_t count = fields.size();
  if (count == 0 || key == keys::esc) {
    return FormEvent::Cancel;
  }
  if (key == keys::tab || key == keys::down) {
    focus = (focus + 1) % count;
  } else if (key == keys::back_tab || key == keys::up) {
    focus = (focus + count - 1) % count;
  } else if (key == keys::enter && focus + 1 < count) {
    ++focus;
  } else if (key == keys::enter || key.is_ctrl(U's')) {
    return FormEvent::Submit;
  } else if (auto* choice = std::get_if<Choice>(&fields[focus].control)) {
    const std::size_t options = choice->options.size();
    if (key == keys::left) {
      choice->chosen = (choice->chosen + options - 1) % options;
    } else if (key == keys::right || key.is(U' ')) {
      choice->chosen = (choice->chosen + 1) % options;
    }
  } else {
    std::get<Input>(fields[focus].control).handle(key);
  }
  return FormEvent::Open;
}

Lines Form::view(std::size_t size) const {
  Lines lines = {Line::styled(title, Look::Heading), Line{}};
  std::size_t label_width = 0;
  for (const Field& field : fields) {
    label_width = std::max(label_width, width(field.label));
  }
  const std::size_t value_width = std::max<std::size_t>(size > label_width + 6 ? size - label_width - 6 : 0, 10);
  for (std::size_t index = 0; index < fields.size(); ++index) {
    const Field& field = fields[index];
    const bool focused = index == focus;
    std::string value;
    if (const auto* choice = std::get_if<Choice>(&field.control)) {
      value = text::concat("‹ ", choice->options.at(choice->chosen).second, " ›");
    } else {
      value = std::get<Input>(field.control).view(value_width, focused);
    }
    const Look look = focused ? Look::Accent : Look::Plain;
    lines.push_back(Line::styled(focused ? "▌ " : "  ", Look::Accent)
                        .with(pad(field.label, label_width), look)
                        .with(text::concat("  ", value), Look::Plain));
    if (focused && !field.help.empty()) {
      for (const Line& help : wrapped(field.help, size > 4 ? size - 4 : 0, Look::Muted)) {
        lines.push_back(Line::plain("    ").with(help.text(), Look::Muted));
      }
    }
  }
  lines.emplace_back();
  if (!problem.empty()) {
    bool first = true;
    for (const Line& line : wrapped(text::concat("Not saved: ", problem), size > 2 ? size - 2 : 0, Look::Failure)) {
      lines.push_back(Line::styled(text::concat(first ? "✖ " : "  ", line.text()), Look::Failure));
      first = false;
    }
    lines.emplace_back();
  }
  extend(lines, wrapped(form_hint, size, Look::Muted));
  return lines;
}

std::vector<std::size_t> layout(const std::vector<Column>& columns, std::size_t size) {
  std::size_t fixed = 0;
  std::size_t flexible = 0;
  for (const Column& column : columns) {
    fixed += column.flexible ? 2 : column.width + 2;
    flexible += column.flexible ? 1 : 0;
  }
  const std::size_t spare = size > fixed ? size - fixed : 0;
  const std::size_t share = std::max(flexible == 0 ? 0 : spare / flexible, min_flexible);
  std::vector<std::size_t> widths;
  widths.reserve(columns.size());
  for (const Column& column : columns) {
    widths.push_back(column.flexible ? share : column.width);
  }
  return widths;
}

std::string row(const std::vector<std::string>& cells, const std::vector<std::size_t>& widths) {
  std::string out;
  for (std::size_t index = 0; index < cells.size() && index < widths.size(); ++index) {
    out.append(pad(cells[index], widths[index]));
    out.append("  ");
  }
  return out;
}

std::string tail(std::string_view text, std::size_t size) {
  const std::u32string letters = text::decode(text);
  if (letters.size() <= size || size == 0) {
    return clip(text, size);
  }
  return text::concat("…", text::encode(std::u32string_view(letters).substr(letters.size() - (size - 1))));
}

std::string clip_start(std::string_view text, std::size_t size) {
  return size < 8 ? std::string() : clip(text, size);
}

Line selectable(std::string_view text, bool selected, std::size_t size) {
  if (selected) {
    return Line::styled(pad(text::concat("▌ ", text), size), Look::Selected);
  }
  return Line::plain(text::concat("  ", text));
}

std::size_t step(std::size_t current, std::ptrdiff_t delta, std::size_t count) {
  if (count == 0) {
    return 0;
  }
  const std::size_t distance = static_cast<std::size_t>(delta < 0 ? -delta : delta);
  const std::size_t moved = delta < 0 ? (current > distance ? current - distance : 0) : current + distance;
  return std::min(moved, count - 1);
}

}
