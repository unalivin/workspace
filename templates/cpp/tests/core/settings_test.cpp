#include <doctest/doctest.h>

#include <algorithm>
#include <set>
#include <string>
#include <tuple>
#include <vector>

#include "core/error.hpp"
#include "core/settings.hpp"
#include "core/text.hpp"
#include "core/theme.hpp"
#include "support/fixture.hpp"
#include "support/random.hpp"
#include "support/testkit.hpp"

using app::AppError;
using app::clock::Clock;
using app::platform::Env;
using app::settings::Flags;
using app::settings::Settings;
using app::settings::Source;
using app::settings::Value;
namespace settings = app::settings;
namespace text = app::text;

namespace {

Settings service(const app::store::Store& store, std::vector<std::pair<std::string, std::string>> env,
                 const Flags& flags) {
  return {store, Clock::fixed(testkit::fixed_time()), Env::from_pairs(std::move(env)), flags};
}

void store_raw(const app::store::Store& store, std::string_view key, std::string_view value) {
  store.prepare("INSERT INTO settings (key, value, updated_at) VALUES (?1, ?2, ?3)")
      .bind(key)
      .bind(value)
      .bind("2026-03-02T09:30:00Z")
      .run();
}

}

TEST_CASE("every default is valid and every key is unique") {
  std::set<std::string_view> keys;
  for (const settings::Definition& definition : settings::definitions()) {
    CHECK_MESSAGE(settings::normalize(definition, definition.fallback) == definition.fallback, definition.key);
    CHECK_FALSE(definition.label.empty());
    CHECK_FALSE(definition.help.empty());
    CHECK_FALSE(definition.group.empty());
    CHECK(settings::lookup(definition.key) == &definition);
    keys.insert(definition.key);
  }
  CHECK(keys.size() == settings::definitions().size());
  CHECK(settings::definitions().size() == 17);
  std::vector<std::string> options;
  for (const std::string_view option : settings::lookup(settings::key_theme)->options) {
    options.emplace_back(option);
  }
  CHECK(options == app::theme::names());
  CHECK(settings::label(settings::Kind::Columns) == "columns");
  CHECK(settings::label(Source::Environment) == "environment");
  CHECK(settings::lookup("no.such.key") == nullptr);
}

TEST_CASE("values are stored listed and reset") {
  const testkit::Fixture fixture("settings-store");
  const Flags flags;
  const Settings all = service(fixture.store, {}, flags);
  const Value initial = all.get(settings::key_density);
  CHECK(initial.raw == "comfortable");
  CHECK(initial.source == Source::Default);
  CHECK_FALSE(initial.stored.has_value());
  CHECK_FALSE(initial.locked());
  const Value changed = all.set(settings::key_density, " Compact ");
  CHECK(changed.raw == "compact");
  CHECK(changed.source == Source::Stored);
  CHECK(all.text(settings::key_density) == "compact");
  CHECK(all.list().size() == settings::definitions().size());
  CHECK(all.effective().at(std::string(settings::key_density)).raw == "compact");
  all.set(settings::key_density, "comfortable");
  CHECK(all.text(settings::key_density) == "comfortable");
  CHECK(all.reset(settings::key_density).source == Source::Default);
  const std::string unknown = testkit::failure_of([&] { all.set("no.such.key", "x"); });
  CHECK_MESSAGE(text::contains(unknown, "unknown setting"), unknown);
  CHECK_THROWS_AS(static_cast<void>(all.get("no.such.key")), AppError);
  CHECK_THROWS_AS(all.reset("no.such.key"), AppError);
  CHECK(all.flag(settings::key_confirm));
  CHECK(all.number(settings::key_text_scale) == 100);
}

TEST_CASE("invalid input is rejected with the problem and the fix") {
  const std::vector<std::tuple<std::string_view, std::string, std::string>> cases = {
      {settings::key_mode, "neon", "Use one of: system, light, dark"},
      {settings::key_text_scale, "big", "Use a number from 80 to 200"},
      {settings::key_text_scale, "500", "Use a number from 80 to 200"},
      {settings::key_text_scale, "+-90", "Use a number from 80 to 200"},
      {settings::key_text_scale, "1e2", "Use a number from 80 to 200"},
      {settings::key_confirm, "maybe", "Use true or false"},
      {settings::key_accent, "blue", "Use the form #RRGGBB"},
      {settings::key_accent, "1257C4", "Use the form #RRGGBB"},
      {settings::key_record_columns, "name,owner", "Use some of: id, name"},
      {settings::key_record_columns, std::string(401, 'a'), "Use some of: id, name"},
      {settings::key_record_sort, "owner:asc", "Use a column"},
      {settings::key_record_sort, "name:sideways", "Use a column"},
  };
  for (const auto& [key, raw, fix] : cases) {
    const std::string error =
        testkit::failure_of([&] { static_cast<void>(settings::normalize(*settings::lookup(key), raw)); });
    CHECK_MESSAGE((text::contains(error, fix) && error.starts_with(key)), key, "=", raw, ": ", error);
  }
}

TEST_CASE("normalization produces canonical text") {
  const auto check = [](std::string_view key, std::string_view raw, std::string_view expected) {
    CHECK_MESSAGE(settings::normalize(*settings::lookup(key), raw) == expected, key, "=", raw);
  };
  check(settings::key_confirm, "YES", "true");
  check(settings::key_confirm, "0", "false");
  check(settings::key_text_scale, " 0120 ", "120");
  check(settings::key_text_scale, "+120", "120");
  check(settings::key_accent, "#1257c4", "#1257C4");
  check(settings::key_accent, "  ", "");
  check(settings::key_record_columns, "state, id", "id,name,state");
  check(settings::key_record_columns, "", "name");
  check(settings::key_record_sort, "Severity", "severity:asc");
  check(settings::key_record_sort, "updated : DESC", "updated:desc");
  CHECK(settings::split_sort("updated:desc") == std::pair<std::string, bool>{"updated", true});
  CHECK(settings::split_sort("name") == std::pair<std::string, bool>{"name", false});
  CHECK(settings::split_columns("id, name,,state") == std::vector<std::string>{"id", "name", "state"});
}

TEST_CASE("flag beats environment beats stored beats default") {
  const testkit::Fixture fixture("settings-precedence");
  const std::string variable = settings::env_name(settings::key_theme);
  const Flags none;
  const Flags flags = {{std::string(settings::key_theme), "amber"}};
  service(fixture.store, {}, none).set(settings::key_theme, "green");
  CHECK(service(fixture.store, {}, none).get(settings::key_theme).raw == "green");
  const Value from_env = service(fixture.store, {{variable, "purple"}}, none).get(settings::key_theme);
  CHECK(from_env.raw == "purple");
  CHECK(from_env.source == Source::Environment);
  CHECK(text::contains(from_env.overridden_by, variable));
  CHECK(from_env.locked());
  CHECK(from_env.stored == "green");
  const Value from_flag = service(fixture.store, {{variable, "purple"}}, flags).get(settings::key_theme);
  CHECK(from_flag.raw == "amber");
  CHECK(from_flag.source == Source::Flag);
  CHECK(text::contains(from_flag.overridden_by, "--set appearance.theme"));
}

TEST_CASE("invalid overrides are ignored and explained") {
  const testkit::Fixture fixture("settings-overrides");
  const std::string variable = settings::env_name(settings::key_theme);
  const Flags none;
  Value value = service(fixture.store, {{variable, "neon"}}, none).get(settings::key_theme);
  CHECK(value.raw == "slate");
  CHECK(value.source == Source::Default);
  CHECK(text::contains(value.override_error, variable));
  CHECK(text::contains(value.override_error, "is ignored"));
  const Flags flags = {{std::string(settings::key_theme), "neon"}};
  value = service(fixture.store, {}, flags).get(settings::key_theme);
  CHECK_MESSAGE(text::contains(value.override_error, "--set appearance.theme is ignored"), value.override_error);
}

TEST_CASE("stored values from other versions are kept and not used") {
  const testkit::Fixture fixture("settings-unknown");
  const Flags none;
  store_raw(fixture.store, "future.option", "42");
  store_raw(fixture.store, settings::key_theme, "neon");
  const Settings all = service(fixture.store, {}, none);
  const std::vector<Value> values = all.list();
  const auto unknown = std::ranges::find_if(values, [](const Value& value) { return value.key == "future.option"; });
  REQUIRE(unknown != values.end());
  CHECK(unknown->unknown);
  CHECK(unknown->raw == "42");
  CHECK(unknown->definition.group == "Unknown");
  const Value broken = all.get(settings::key_theme);
  CHECK(broken.raw == "slate");
  CHECK_MESSAGE(text::contains(broken.override_error, "stored value is invalid"), broken.override_error);
  CHECK(settings::groups(values).back() == "Unknown");
  CHECK(settings::groups(values).front() == "Appearance");
  CHECK(settings::groups(values).size() == 8);
}

TEST_CASE("normalize is idempotent and survives noise") {
  testkit::Random random(23);
  const std::vector<std::string_view> alphabet = {"a",        "1",  "0",     "#",    ":",    ",",  " ",
                                                  "-",        "+",  "name",  "desc", "true", "id", "F",
                                                  "\xC3\xA9", "\t", "state", "dark", "9"};
  int accepted = 0;
  for (int round = 0; round < 6000; ++round) {
    const settings::Definition& definition = settings::definitions()[static_cast<std::size_t>(random.between(0, 16))];
    const std::string raw = round % 5 == 0 ? random.bytes(60) : random.pieces(8, alphabet);
    try {
      const std::string once = settings::normalize(definition, raw);
      ++accepted;
      CHECK(settings::normalize(definition, once) == once);
    } catch (const AppError& error) {
      CHECK(error.field() == definition.key);
    }
  }
  CHECK(accepted > 100);
}
