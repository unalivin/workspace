#include <chrono>
#include <optional>
#include <ostream>
#include <thread>
#include <utility>

#include <nlohmann/json.hpp>

#include "app/command.hpp"
#include "core/files.hpp"
#include "core/meta.hpp"
#include "core/notifier.hpp"
#include "core/notify.hpp"
#include "core/records.hpp"
#include "core/text.hpp"
#include "core/theme.hpp"

namespace app::cli {

namespace {

using notify::Notification;
using notify::Rule;

std::string event_names() {
  std::vector<std::string> names;
  for (const notify::EventType& event : notify::event_types()) {
    names.emplace_back(event.id);
  }
  return text::join(names, ", ");
}

void run_list(Env& env, const Call& call, service::App& app) {
  call.no_arguments();
  notify::Filter filter;
  if (!call.text("severity").empty()) {
    filter.severity = records::parse_severity(call.text("severity"));
  }
  const std::string_view event = call.text("event");
  if (!event.empty() && notify::event_type(event) == nullptr) {
    throw usage(text::concat("unknown event type \"", event, "\": use one of ", event_names()));
  }
  filter.unread_only = call.truth("unread");
  filter.event_type = std::string(event);
  filter.limit = call.number("limit");
  const std::vector<Notification> list = app.notifications().list(filter);
  const std::string_view format = call.text("format");
  if (format == "json") {
    nlohmann::ordered_json rows = nlohmann::ordered_json::array();
    for (const Notification& item : list) {
      nlohmann::ordered_json row;
      row["id"] = item.id;
      row["event_type"] = item.event_type;
      row["severity"] = theme::name(item.severity);
      row["title"] = item.title;
      row["record_kind"] = item.record_kind;
      row["record_id"] = item.record_id;
      row["created_at"] = item.created_at.to_rfc3339();
      row["read"] = !item.unread();
      rows.push_back(std::move(row));
    }
    json(env, rows);
    return;
  }
  if (format != "table") {
    throw usage(text::concat("unknown format \"", format, "\": use table or json"));
  }
  if (list.empty()) {
    line(env, "No notifications. They appear when a record needs attention or is completed.");
    return;
  }
  std::vector<std::vector<std::string>> rows;
  for (const Notification& item : list) {
    rows.push_back({std::to_string(item.id), app.format_time(item.created_at),
                    text::concat(theme::glyph(item.severity), " ", theme::label(item.severity)),
                    item.unread() ? "unread" : "read", item.title,
                    item.record_id == 0 ? std::string("-") : text::concat(item.record_kind, " ", item.record_id)});
  }
  table(env, {"ID", "WHEN", "SEVERITY", "STATE", "TITLE", "LINK"}, rows);
}

void run_read(Env& env, const Call& call, service::App& app) {
  if (call.args.size() != 1) {
    throw usage("give one notification identifier or the word all");
  }
  const std::string& target = call.args.front();
  if (target == "all") {
    line(env, text::concat("Marked ", app.notifications().mark_all_read(), " notifications as read"));
    return;
  }
  const std::optional<std::int64_t> id = text::parse_integer(target);
  if (!id || *id <= 0) {
    throw usage(text::concat("\"", target,
                             "\" is not a notification identifier: use the number from the ID column or the word all"));
  }
  app.notifications().mark_read(*id);
  line(env, text::concat("Marked notification ", *id, " as read"));
}

std::string quiet_text(const Rule& rule) {
  return rule.quiet_start.empty() ? std::string("none") : text::concat(rule.quiet_start, " to ", rule.quiet_end);
}

void run_rules_list(Env& env, const Call& call, service::App& app) {
  call.no_arguments();
  std::vector<std::vector<std::string>> rows;
  for (const Rule& rule : app.notifications().rules()) {
    const notify::EventType* event = notify::event_type(rule.event_type);
    rows.push_back({rule.event_type, event == nullptr ? std::string() : std::string(event->label),
                    std::string(enabled_text(rule.enabled)), std::string(theme::label(rule.min_severity)),
                    quiet_text(rule), std::to_string(rule.dedup_minutes), rule.customized ? "changed" : "default"});
  }
  table(env, {"EVENT", "MEANING", "ENABLED", "MIN SEVERITY", "QUIET HOURS", "DEDUP MINUTES", "SOURCE"}, rows);
}

const std::string& event_argument(const Call& call) {
  if (call.args.size() != 1) {
    throw usage(text::concat("give one event type: ", event_names()));
  }
  return call.args.front();
}

void run_rules_set(Env& env, const Call& call, service::App& app) {
  const std::string& event = event_argument(call);
  Rule rule = app.notifications().rule(event);
  bool changed = false;
  if (call.given("enabled")) {
    const std::string lowered = text::lower(call.text("enabled"));
    if (lowered == "true" || lowered == "yes" || lowered == "on" || lowered == "1") {
      rule.enabled = true;
    } else if (lowered == "false" || lowered == "no" || lowered == "off" || lowered == "0") {
      rule.enabled = false;
    } else {
      throw usage(text::concat("--enabled needs true or false, found \"", lowered, "\""));
    }
    changed = true;
  }
  if (call.given("min-severity")) {
    const std::string raw = text::lower(call.text("min-severity"));
    const std::optional<theme::Severity> parsed = theme::parse_severity(raw);
    if (!parsed) {
      throw usage(text::concat("--min-severity needs one of ", text::join(theme::severity_names(), ", "), ", found \"",
                               raw, "\""));
    }
    rule.min_severity = *parsed;
    changed = true;
  }
  if (call.given("quiet-start")) {
    rule.quiet_start = std::string(call.text("quiet-start"));
    changed = true;
  }
  if (call.given("quiet-end")) {
    rule.quiet_end = std::string(call.text("quiet-end"));
    changed = true;
  }
  if (call.given("dedup-minutes")) {
    const std::optional<std::int64_t> minutes = text::parse_integer(call.text("dedup-minutes"));
    if (!minutes) {
      throw usage(text::concat("--dedup-minutes needs a whole number, found \"", call.text("dedup-minutes"), "\""));
    }
    rule.dedup_minutes = *minutes;
    changed = true;
  }
  if (!changed) {
    throw usage(
        "nothing to change: give at least one of --enabled, --min-severity, --quiet-start, --quiet-end or "
        "--dedup-minutes");
  }
  app.notifications().set_rule(rule);
  line(env, text::concat("Rule for ", rule.event_type, ": enabled ", enabled_text(rule.enabled), ", minimum severity ",
                         theme::label(rule.min_severity), ", quiet hours ", quiet_text(rule), ", de-duplication ",
                         rule.dedup_minutes, " minutes"));
}

void run_rules_reset(Env& env, const Call& call, service::App& app) {
  const std::string& event = event_argument(call);
  app.notifications().reset_rule(event);
  line(env, text::concat("Rule for ", event, " is back to its defaults"));
}

bool sleep_for(std::int64_t seconds) {
  std::this_thread::sleep_for(std::chrono::seconds(seconds < 0 ? 0 : seconds));
  return true;
}

void run_notifier(Env& env, const Call& call, service::App& app) {
  call.no_arguments();
  if (!env.hooks.deliver) {
    throw failure("this build cannot show system notifications");
  }
  const std::int64_t interval = app.settings().number(settings::key_notifier_interval);
  notifier::Runner runner;
  runner.database = app.location().path;
  runner.state_dir = app.location().config_dir;
  runner.show_titles = app.settings().flag(settings::key_notifier_titles);
  runner.clock = app.options().clock;
  runner.deliver = env.hooks.deliver;
  if (call.truth("once")) {
    line(env, text::concat("Checked once: ", runner.tick(), " new notifications shown"));
    return;
  }
  note(env, text::concat(meta::app_name, " notifier is running, checking every ", notifier::clamp_interval(interval),
                         " seconds. It reads the database and makes no network requests. Press Ctrl+C to stop."));
  const auto log = [&env](std::string_view message) { note(env, text::concat("notifier: ", message)); };
  runner.run(interval, 0, env.hooks.wait ? env.hooks.wait : sleep_for, log);
}

notifier::Autostart autostart(const Env& env) {
  if (!env.hooks.autostart) {
    throw failure("this build cannot manage the sign-in entry");
  }
  return env.hooks.autostart();
}

void run_install(Env& env, const Call& call) {
  call.no_arguments();
  const notifier::Autostart entry = autostart(env);
  const std::string path = files::shown(entry.install(call.truth("dry-run")));
  if (call.truth("dry-run")) {
    line(env, text::concat("Dry run: would write ", path, " to start \"", entry.executable,
                           " notifier run\" at sign-in. Nothing was changed."));
  } else {
    line(env, text::concat("Wrote ", path, ". The notifier starts the next time you sign in. Start it now with \"",
                           meta::slug, " notifier run\"."));
  }
}

void run_uninstall(Env& env, const Call& call) {
  call.no_arguments();
  const notifier::Placement placement = autostart(env).uninstall(call.truth("dry-run"));
  const std::string path = files::shown(placement.path);
  if (!placement.present) {
    line(env, text::concat("The notifier is not set to start at sign-in (", path, " does not exist)."));
  } else if (call.truth("dry-run")) {
    line(env, text::concat("Dry run: would remove ", path, ". Nothing was changed."));
  } else {
    line(env, text::concat("Removed ", path,
                           ". A notifier that is running now keeps running until you sign out or stop it."));
  }
}

void run_status(Env& env, const Call& call) {
  call.no_arguments();
  const notifier::Placement placement = autostart(env).status();
  const std::string path = files::shown(placement.path);
  if (placement.present) {
    line(env, text::concat("Start at sign-in: on (", path, ")"));
  } else {
    line(env, text::concat("Start at sign-in: off (", path, " does not exist)"));
  }
  line(env, "Display of system notifications depends on the desktop and is not verified by the automated tests.");
}

}

Command notifications_command() {
  return Command::group(
      "notifications", "Read notifications and manage their rules",
      {
          Command::leaf("list", "List notifications, newest first", run_list)
              .with({
                  toggle("unread", "only unread notifications"),
                  text_flag("severity", "", "only this severity"),
                  text_flag("event", "", "only this event type"),
                  number_flag("limit", "200", "largest number of rows"),
                  text_flag("format", "table", "table or json"),
              }),
          Command::leaf("read", "Mark one notification, or all of them, as read", run_read).taking("<id|all>"),
          Command::group("rules", "Show or change the rule of each event type",
                         {
                             Command::leaf("list", "List the rule of every event type", run_rules_list),
                             Command::leaf("set", "Change the rule of an event type", run_rules_set)
                                 .taking("<event>")
                                 .with({
                                     text_flag("enabled", "", "true or false"),
                                     text_flag("min-severity", "", "lowest severity that raises a notification"),
                                     text_flag("quiet-start", "", "start of quiet hours as HH:MM, empty for none"),
                                     text_flag("quiet-end", "", "end of quiet hours as HH:MM, empty for none"),
                                     text_flag("dedup-minutes", "",
                                               "minutes in which a repeated event is not raised again, 0 for off"),
                                 }),
                             Command::leaf("reset", "Return the rule of an event type to its defaults", run_rules_reset)
                                 .taking("<event>"),
                         }),
      });
}

Command notifier_command() {
  const Flag dry_run = toggle("dry-run", "show what would change without changing it");
  return Command::group(
      "notifier", "Show system notifications while the application is closed (opt-in)",
      {
          Command::leaf("run", "Check for new notifications on an interval until stopped", run_notifier)
              .with({toggle("once", "check one time and exit")}),
          Command::standalone(
              "install", "Start the notifier when you sign in (per-user entry, no administrator rights)", run_install)
              .with({dry_run}),
          Command::standalone("uninstall", "Remove the sign-in entry", run_uninstall).with({dry_run}),
          Command::standalone("status", "Show whether the sign-in entry exists", run_status),
      });
}

}
