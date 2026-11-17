#include "core/notifier.hpp"

#include <algorithm>
#include <optional>
#include <system_error>

#include <nlohmann/json.hpp>

#include "core/error.hpp"
#include "core/files.hpp"
#include "core/meta.hpp"
#include "core/notify.hpp"
#include "core/store.hpp"
#include "core/text.hpp"

namespace app::notifier {

namespace fs = std::filesystem;

namespace {

constexpr std::int64_t batch_limit = 50;
constexpr std::uint64_t longest_state = 4096;
constexpr std::string_view windows_script =
    "Add-Type -AssemblyName System.Windows.Forms; $n = New-Object System.Windows.Forms.NotifyIcon; $n.Icon = "
    "[System.Drawing.SystemIcons]::Information; $n.Visible = $true; $n.ShowBalloonTip(8000, $env:APP_NOTIFY_TITLE, "
    "$env:APP_NOTIFY_BODY, [System.Windows.Forms.ToolTipIcon]::Info); Start-Sleep -Seconds 9; $n.Dispose()";

struct State {
  std::int64_t last_id = 0;
  bool initialized = false;
};

State load_state(const fs::path& path) {
  if (!files::exists(path)) {
    return {};
  }
  std::string raw;
  try {
    raw = files::read_limited(path, longest_state);
  } catch (const AppError& error) {
    if (error.is_usage()) {
      return {};
    }
    throw AppError::io(text::concat("cannot read the notifier state: ", error.message()));
  }
  const nlohmann::json document = nlohmann::json::parse(raw, nullptr, false);
  if (!document.is_object()) {
    return {};
  }
  State state;
  if (const auto found = document.find("last_id"); found != document.end()) {
    const bool too_large =
        found->is_number_unsigned() && found->get<std::uint64_t>() > static_cast<std::uint64_t>(INT64_MAX);
    if (!found->is_number_integer() || too_large) {
      return {};
    }
    state.last_id = found->get<std::int64_t>();
  }
  if (const auto found = document.find("initialized"); found != document.end()) {
    if (!found->is_boolean()) {
      return {};
    }
    state.initialized = found->get<bool>();
  }
  if (state.last_id < 0) {
    return {};
  }
  return state;
}

void save_state(const fs::path& directory, State state) {
  files::create_private_dir(directory);
  nlohmann::ordered_json document;
  document["last_id"] = state.last_id;
  document["initialized"] = state.initialized;
  files::write_private(directory / files::from_utf8(state_file_name), document.dump(), true);
}

AppError unknown_home() {
  return AppError::io("the home directory is unknown, so the autostart entry cannot be placed");
}

std::string agent_label() {
  return text::concat(meta::app_id, ".notifier");
}

bool is_absolute_text(std::string_view path) {
  return path.starts_with('/');
}

void check_executable(const Autostart& entry) {
  const std::string& path = entry.executable;
  const bool drive = entry.os == platform::Os::Windows && path.size() > 2 &&
                     ((path[0] >= 'a' && path[0] <= 'z') || (path[0] >= 'A' && path[0] <= 'Z')) && path[1] == ':';
  if (path.empty() || !(is_absolute_text(path) || drive)) {
    throw AppError::usage(text::concat("the executable path \"", path, "\" must be absolute"));
  }
  const std::u32string letters = text::decode(path);
  const bool unusable = std::ranges::any_of(letters, [](char32_t letter) {
    return text::is_control(letter) || letter == U'"' || letter == U'`' || letter == U'$' || letter == U'%';
  });
  if (unusable || !text::is_valid_utf8(path)) {
    throw AppError::usage(text::concat("the executable path \"", path,
                                       "\" contains a character that cannot be used in an autostart entry: move the "
                                       "application to a simpler path"));
  }
}

}

std::int64_t clamp_interval(std::int64_t seconds) {
  return std::clamp(seconds, min_interval_seconds, max_interval_seconds);
}

std::size_t Runner::tick() const {
  std::optional<store::Store> opened;
  try {
    opened.emplace(store::Store::open(database, store::Options{true, std::nullopt}));
  } catch (const AppError& error) {
    throw AppError::storage(text::concat("cannot open the database read-only: ", error.message()));
  }
  const notify::Notifications source(*opened, clock, clock::Zone::utc());
  const fs::path state_path = state_dir / files::from_utf8(state_file_name);
  const State current = load_state(state_path);
  if (!current.initialized) {
    save_state(state_dir, {source.highest_id(), true});
    return 0;
  }
  const std::vector<notify::Notification> items = source.after(current.last_id, batch_limit);
  if (items.empty()) {
    return 0;
  }
  std::vector<notify::Notification> fresh;
  std::ranges::copy_if(items, std::back_inserter(fresh),
                       [](const notify::Notification& item) { return item.unread(); });
  if (!fresh.empty()) {
    const notify::Summary summary = notify::summary(fresh, show_titles);
    try {
      deliver(summary.title, summary.body);
    } catch (const AppError& error) {
      throw AppError::io(text::concat("cannot show the system notification: ", error.message()));
    }
  }
  save_state(state_dir, {items.back().id, true});
  return fresh.size();
}

void Runner::run(std::int64_t interval_seconds, std::size_t max_ticks, const std::function<bool(std::int64_t)>& wait,
                 const std::function<void(std::string_view)>& log) const {
  const std::int64_t interval = clamp_interval(interval_seconds);
  for (std::size_t round = 0; max_ticks == 0 || round < max_ticks; ++round) {
    try {
      static_cast<void>(tick());
    } catch (const AppError& error) {
      log(error.message());
    }
    if (!wait(interval)) {
      return;
    }
  }
}

DeliveryCommand delivery_command(platform::Os os, std::string_view title, std::string_view body) {
  const std::string safe_title = notify::safe_title(title);
  const std::string safe_body = notify::safe_title(body);
  switch (os) {
    case platform::Os::Windows:
      return {"powershell",
              {"-NoProfile", "-NonInteractive", "-WindowStyle", "Hidden", "-Command", std::string(windows_script)},
              {{"APP_NOTIFY_TITLE", safe_title}, {"APP_NOTIFY_BODY", safe_body}}};
    case platform::Os::Macos:
      return {"osascript",
              {"-e", "on run argv", "-e", "display notification (item 2 of argv) with title (item 1 of argv)", "-e",
               "end run", "--", safe_title, safe_body},
              {}};
    case platform::Os::Linux:
      break;
  }
  return {"notify-send", {"--app-name", safe_title, "--", safe_title, safe_body}, {}};
}

void deliver_with_system(platform::Os os, std::string_view title, std::string_view body) {
  const DeliveryCommand command = delivery_command(os, title, body);
  int code = 0;
  try {
    code = platform::run_quiet(command.program, command.arguments, command.environment);
  } catch (const AppError& error) {
    throw AppError::io(text::concat(command.program,
                                    " could not be started, so system notifications cannot be shown on this "
                                    "computer: ",
                                    error.message()));
  }
  if (code == 127) {
    throw AppError::io(text::concat(command.program,
                                    " could not be started, so system notifications cannot be shown on this "
                                    "computer: the program was not found"));
  }
  if (code != 0) {
    throw AppError::io(text::concat(command.program, " ended with exit status ", code));
  }
}

Autostart Autostart::for_system(platform::Os os, const platform::Env& env, const fs::path& executable) {
  Autostart entry;
  entry.os = os;
  const std::optional<std::string> home = env.get("HOME");
  entry.home_dir = home ? *home : env.get("USERPROFILE").value_or("");
  entry.config_home = env.get("XDG_CONFIG_HOME").value_or("");
  entry.app_data = env.get("APPDATA").value_or("");
  entry.executable = files::shown(executable);
  return entry;
}

fs::path Autostart::path() const {
  switch (os) {
    case platform::Os::Windows: {
      fs::path base;
      if (!app_data.empty()) {
        base = files::from_utf8(app_data);
      } else if (!home_dir.empty()) {
        base = files::from_utf8(home_dir) / "AppData" / "Roaming";
      } else {
        throw unknown_home();
      }
      return base / "Microsoft" / "Windows" / "Start Menu" / "Programs" / "Startup" /
             files::from_utf8(text::concat(meta::slug, "-notifier.cmd"));
    }
    case platform::Os::Macos:
      if (home_dir.empty()) {
        throw unknown_home();
      }
      return files::from_utf8(home_dir) / "Library" / "LaunchAgents" /
             files::from_utf8(text::concat(agent_label(), ".plist"));
    case platform::Os::Linux:
      break;
  }
  if (home_dir.empty()) {
    throw unknown_home();
  }
  const fs::path base = files::from_utf8(config_home).is_absolute() ? files::from_utf8(config_home)
                                                                    : files::from_utf8(home_dir) / ".config";
  return base / "autostart" / files::from_utf8(text::concat(meta::slug, "-notifier.desktop"));
}

std::string Autostart::content() const {
  check_executable(*this);
  switch (os) {
    case platform::Os::Windows:
      return text::concat("@echo off\r\nstart \"\" /min \"", executable, "\" notifier run\r\n");
    case platform::Os::Macos: {
      const std::string escaped =
          text::replace_all(text::replace_all(text::replace_all(executable, "&", "&amp;"), "<", "&lt;"), ">", "&gt;");
      return text::concat(
          "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<!DOCTYPE plist PUBLIC \"-//Apple//DTD PLIST 1.0//EN\" "
          "\"http://www.apple.com/DTDs/PropertyList-1.0.dtd\">\n<plist version=\"1.0\">\n<dict>\n  "
          "<key>Label</key>\n  <string>",
          agent_label(), "</string>\n  <key>ProgramArguments</key>\n  <array>\n    <string>", escaped,
          "</string>\n    <string>notifier</string>\n    <string>run</string>\n  </array>\n  "
          "<key>RunAtLoad</key>\n  <true/>\n  <key>ProcessType</key>\n  <string>Background</string>\n</dict>\n"
          "</plist>\n");
    }
    case platform::Os::Linux:
      break;
  }
  return text::concat("[Desktop Entry]\nType=Application\nName=", meta::app_name, " notifier\nComment=Shows ",
                      meta::app_name, " notifications\nExec=\"", text::replace_all(executable, "\\", "\\\\"),
                      "\" notifier run\nTerminal=false\nNoDisplay=true\nX-GNOME-Autostart-enabled=true\n");
}

fs::path Autostart::install(bool dry_run) const {
  fs::path target = path();
  const std::string body = content();
  if (!dry_run) {
    std::error_code problem;
    fs::create_directories(target.parent_path(), problem);
    if (problem) {
      throw AppError::io(
          text::concat("cannot create \"", files::shown(target.parent_path()), "\": ", problem.message()));
    }
    files::write_private(target, body, true);
  }
  return target;
}

Placement Autostart::uninstall(bool dry_run) const {
  fs::path target = path();
  if (!files::exists(target)) {
    return {false, std::move(target)};
  }
  if (!dry_run) {
    std::error_code problem;
    fs::remove(target, problem);
    if (problem) {
      throw AppError::io(text::concat("cannot remove \"", files::shown(target), "\": ", problem.message()));
    }
  }
  return {true, std::move(target)};
}

Placement Autostart::status() const {
  fs::path target = path();
  std::error_code problem;
  const bool present = fs::is_regular_file(target, problem) && !problem;
  return {present, std::move(target)};
}

}
