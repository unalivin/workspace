#include "gui/forms.hpp"

#include <algorithm>

namespace app::gui {

Field Field::text(std::string_view key, std::string_view label, std::string_view value, std::string_view help,
                  std::size_t limit) {
  Field field;
  field.key = std::string(key);
  field.label = std::string(label);
  field.value = std::string(value);
  field.help = std::string(help);
  field.limit = limit;
  return field;
}

Field Field::area(std::string_view key, std::string_view label, std::string_view value, std::string_view help,
                  std::size_t limit) {
  Field field = Field::text(key, label, value, help, limit);
  field.multiline = true;
  return field;
}

Field Field::choice(std::string_view key, std::string_view label, std::string_view value, std::string_view help,
                    Choices options) {
  Field field = Field::text(key, label, value, help, 0);
  field.options = std::move(options);
  return field;
}

Field Field::yes_no(std::string_view key, std::string_view label, bool value, std::string_view help) {
  return Field::choice(key, label, value ? "true" : "false", help, {{"true", "Yes"}, {"false", "No"}});
}

std::string_view Field::shown() const {
  const auto found = std::ranges::find(options, value, &std::pair<std::string, std::string>::first);
  return found == options.end() ? std::string_view() : std::string_view(found->second);
}

Form::Form(std::string heading, std::string_view accept_label, std::vector<Field> inputs, Submit target)
    : title(std::move(heading)), accept(accept_label), fields(std::move(inputs)), submit(std::move(target)) {}

std::string Form::value(std::string_view key) const {
  const auto found = std::ranges::find(fields, key, &Field::key);
  return found == fields.end() ? std::string() : found->value;
}

void Form::set(std::string_view key, std::string_view value) {
  const auto found = std::ranges::find(fields, key, &Field::key);
  if (found != fields.end()) {
    found->value = std::string(value);
  }
}

Confirm::Confirm(std::string_view heading, std::string text, std::string_view accept_label,
                 std::optional<Action> target)
    : title(heading), statement(std::move(text)), accept(accept_label), action(std::move(target)) {}

}
