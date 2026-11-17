#pragma once

#include <array>
#include <cstdint>
#include <filesystem>
#include <optional>
#include <string>
#include <string_view>
#include <utility>
#include <vector>

#include "core/clock.hpp"
#include "core/guide.hpp"
#include "core/meta.hpp"
#include "core/notify.hpp"
#include "core/platform.hpp"
#include "core/records.hpp"
#include "core/settings.hpp"
#include "core/store.hpp"
#include "core/theme.hpp"
#include "core/transfer.hpp"
#include "core/userconfig.hpp"

namespace app::service {

struct Options {
  platform::Os os = platform::current_os();
  platform::Env env = platform::Env::system();
  std::optional<std::filesystem::path> config_dir;
  std::optional<std::filesystem::path> database_path;
  settings::Flags overrides;
  clock::Clock clock = clock::Clock::system();
  clock::Zone zone = clock::Zone::local();
  bool read_only = false;

  [[nodiscard]] static Options system();
};

struct Location {
  std::filesystem::path path;
  std::string_view source;
  std::string overridden_by;
  bool exists = false;
  std::filesystem::path config_dir;
  userconfig::Config config;
  bool operator==(const Location&) const = default;
};

[[nodiscard]] Location locate(const Options& options);

enum class MoveMode { Copy, Switch };

struct MovePlan {
  std::filesystem::path from;
  std::filesystem::path to;
  MoveMode mode = MoveMode::Copy;
  bool target_exists = false;
  std::string statement;
  bool operator==(const MovePlan&) const = default;
};

struct IssueReport {
  std::string title;
  std::string body;
  std::string url;
  bool operator==(const IssueReport&) const = default;
};

struct Diagnostics {
  meta::BuildInfo build;
  std::string database_path;
  std::string database_source;
  std::string config_dir;
  std::int64_t schema_version = 0;
  std::string journal_mode;
  bool foreign_keys = false;
  bool full_text_search = false;
  std::string integrity;
  std::int64_t records = 0;
  std::string front_end;
  std::vector<std::string> changed;
  std::vector<std::string> overridden;
  std::vector<std::string> problems;

  [[nodiscard]] std::vector<std::pair<std::string_view, std::string>> lines() const;
  [[nodiscard]] IssueReport issue_report() const;
  bool operator==(const Diagnostics&) const = default;
};

struct Exported {
  std::string text;
  std::size_t count = 0;
};

class App {
 public:
  [[nodiscard]] static App open(Options options);

  [[nodiscard]] const store::Store& store() const;
  [[nodiscard]] settings::Settings settings() const;
  [[nodiscard]] records::Records records() const;
  [[nodiscard]] notify::Notifications notifications() const;
  [[nodiscard]] const Location& location() const;
  [[nodiscard]] const Options& options() const;
  [[nodiscard]] bool created_database() const;
  void dismiss_welcome();
  [[nodiscard]] clock::Timestamp now() const;
  [[nodiscard]] const std::string& front_end() const;
  void remember_front_end(std::string_view mode);
  [[nodiscard]] MovePlan plan_move(std::string_view target, MoveMode mode) const;
  void move_database(const MovePlan& plan);
  [[nodiscard]] bool poll();
  [[nodiscard]] theme::Tokens appearance(bool system_dark) const;
  [[nodiscard]] clock::Zone display_zone() const;
  [[nodiscard]] std::string format_time(clock::Timestamp moment) const;
  [[nodiscard]] transfer::ImportResult import_file(const std::filesystem::path& path, std::string_view format,
                                                   bool dry_run) const;
  [[nodiscard]] Exported export_text(transfer::Format format) const;
  std::size_t export_file(const std::filesystem::path& path, std::string_view format, bool overwrite) const;
  [[nodiscard]] Diagnostics diagnostics(bool dashboard) const;

 private:
  App(store::Store store, Location location, Options options, bool created, std::int64_t seen);
  [[nodiscard]] static App open_at(Options options, Location location);

  store::Store store_;
  Location location_;
  Options options_;
  bool created_ = false;
  std::int64_t seen_ = 0;
};

struct NavItem {
  std::string_view id;
  std::string_view label;
  std::string_view hint;
  bool bottom = false;
};

inline constexpr std::string_view nav_overview = "overview";
inline constexpr std::string_view nav_records = "records";
inline constexpr std::string_view nav_notifications = "notifications";
inline constexpr std::string_view nav_settings = "settings";
inline constexpr std::string_view nav_about = "about";

inline constexpr std::array<NavItem, 5> navigation = {{
    {nav_overview, "Overview", "What needs attention", false},
    {nav_records, "Records", "Everything the application tracks", false},
    {nav_notifications, "Notifications", "Events and notification rules", false},
    {nav_settings, "Settings", "Appearance, data and notifier", true},
    {nav_about, "About", "Guide, shortcuts, diagnostics and licenses", true},
}};

struct Shortcut {
  std::string_view keys;
  std::string_view terminal;
  std::string_view action;
};

inline constexpr std::array<Shortcut, 12> shortcuts = {{
    {"Ctrl/Cmd+1 to 5", "1 to 5", "Open a category"},
    {"Ctrl/Cmd+F", "/", "Search"},
    {"Ctrl/Cmd+,", ",", "Open Settings"},
    {"F1", "F1 or ?", "Open the usage guide"},
    {"Ctrl/Cmd+N", "n", "Add a record"},
    {"Enter", "Enter", "Open the selected row"},
    {"Up and Down", "Up, Down, j, k", "Move between rows"},
    {"Ctrl/Cmd+R", "r", "Refresh now"},
    {"Ctrl/Cmd+B", "b", "Collapse or expand the sidebar"},
    {"Delete", "d", "Remove the selected record after confirmation"},
    {"Escape", "Esc", "Close a panel or cancel"},
    {"Ctrl/Cmd+Q", "q or Ctrl+C", "Quit"},
}};

[[nodiscard]] guide::Guide usage_guide();

struct Link {
  std::string label;
  std::string url;
  bool allowed = false;
  std::string reason;
  bool operator==(const Link&) const = default;
};

[[nodiscard]] Link external_link(std::string_view label, std::string_view url);
[[nodiscard]] Link source_link();
[[nodiscard]] Link advisory_link();

struct Capability {
  std::string_view id;
  std::string_view name;
  std::string_view dashboard;
  std::string_view terminal;
  std::string_view cli;
};

inline constexpr std::string_view not_applicable = "n/a";

[[nodiscard]] const std::vector<Capability>& capabilities();
[[nodiscard]] std::string capability_table();

}
