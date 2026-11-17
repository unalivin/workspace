#pragma once

#include <array>
#include <cstddef>
#include <cstdint>
#include <functional>
#include <map>
#include <optional>
#include <set>
#include <string>
#include <string_view>
#include <utility>
#include <vector>

#include "core/error.hpp"
#include "core/guide.hpp"
#include "core/notifier.hpp"
#include "core/notify.hpp"
#include "core/platform.hpp"
#include "core/records.hpp"
#include "core/service.hpp"
#include "core/settings.hpp"
#include "core/theme.hpp"
#include "gui/forms.hpp"

namespace app::gui {

inline constexpr std::int64_t list_limit = 500;
inline constexpr std::size_t search_limit = 200;
inline constexpr std::size_t path_limit = 1024;
inline constexpr std::string_view demo_prefix = "Demo:";
inline constexpr std::string_view notifier_off =
    "Off. Turn it on to see system notifications while the application is closed. It adds a per-user entry, needs "
    "no administrator rights and makes no network requests.";
inline constexpr std::string_view notifier_missing = "This build cannot manage the sign-in entry.";

struct Options {
  platform::Env env = platform::Env::system();
  bool system_dark = false;
  std::optional<notifier::Autostart> autostart;
  bool in_terminal = false;
  std::function<void(const std::vector<std::string>&)> open_terminal;
  std::function<void(std::string_view)> open_url;
};

enum class Tone { Info, Success, Warning, Failure };

[[nodiscard]] theme::Severity severity(Tone tone);

enum class ShortcutKind { Page, Search, Settings, Add, Refresh, Sidebar, Quit, Guide, Remove, Escape };

struct Shortcut {
  ShortcutKind kind = ShortcutKind::Escape;
  std::size_t page = 0;
  bool operator==(const Shortcut&) const = default;
};

enum class AboutTab { Overview, Guide, Shortcuts, Diagnostics, Licenses };

struct AboutTabName {
  AboutTab tab = AboutTab::Overview;
  std::string_view label;
};

inline constexpr std::array<AboutTabName, 5> about_tabs = {{
    {AboutTab::Overview, "Overview"},
    {AboutTab::Guide, "Usage guide"},
    {AboutTab::Shortcuts, "Shortcuts"},
    {AboutTab::Diagnostics, "Diagnostics"},
    {AboutTab::Licenses, "Licenses and notices"},
}};

struct Message {
  Tone tone = Tone::Info;
  std::string text;
  bool operator==(const Message&) const = default;
};

struct RecordsPage {
  records::Query query;
  std::vector<records::Record> list;
  std::optional<std::int64_t> selected;
  std::vector<std::string> columns;
};

struct InboxPage {
  bool rules = false;
  bool unread_only = false;
  std::vector<notify::Notification> list;
  std::optional<std::int64_t> selected;
  std::vector<notify::Rule> rule_list;
  std::optional<std::string> rule;
};

enum class NotifierKind { Missing, Unknown, On, Off };

struct NotifierState {
  NotifierKind kind = NotifierKind::Missing;
  std::string detail;
  bool operator==(const NotifierState&) const = default;
};

using Texts = std::map<std::string, std::string, std::less<>>;

struct SettingsPage {
  std::vector<settings::Value> list;
  std::vector<std::string> groups;
  Texts drafts;
  std::set<std::string, std::less<>> dirty;
  Texts problems;
  NotifierState notifier;
};

struct AboutPage {
  AboutTab tab = AboutTab::Overview;
  guide::Guide usage;
  std::string search;
  std::vector<std::size_t> hits;
  std::optional<std::size_t> section;
  std::optional<service::Diagnostics> diagnostics;
};

struct WindowSize {
  float width = 0;
  float height = 0;
  bool operator==(const WindowSize&) const = default;
};

using Values = std::map<std::string, settings::Value, std::less<>>;

[[nodiscard]] Choices severity_options();
[[nodiscard]] Choices status_options();
[[nodiscard]] std::string_view event_label(std::string_view id);
[[nodiscard]] std::string_view enabled_label(bool enabled);
[[nodiscard]] std::string_view front_end_label(std::string_view mode);
[[nodiscard]] WindowSize window_size(const Values& values);
[[nodiscard]] std::string_view saved_value(const settings::Value& value);

class Dashboard {
 public:
  Dashboard(service::App& app, Options options);

  theme::Tokens tokens;
  Values values;
  std::size_t page = 0;
  bool collapsed = false;
  std::optional<Message> notice;
  std::optional<Form> form;
  std::optional<Confirm> confirm;
  std::string search;
  bool focus_search = false;
  records::Overview overview;
  RecordsPage rec;
  InboxPage inbox;
  SettingsPage set;
  AboutPage about;
  std::optional<std::string> clipboard;
  bool restyled = true;
  std::uint32_t flash = 0;

  [[nodiscard]] const service::App& app() const;
  [[nodiscard]] const Options& options() const;
  [[nodiscard]] const std::string& next() const;
  [[nodiscard]] bool finished() const;
  [[nodiscard]] WindowSize size() const;
  void follow_system(bool dark);
  [[nodiscard]] double interval() const;
  void close(std::string_view next);
  void keep_window_size(float width, float height);
  void say(Tone tone, std::string text);
  void reload();
  void tick();
  [[nodiscard]] std::string_view page_id() const;
  void show(std::string_view id);
  void show_about(AboutTab tab);
  [[nodiscard]] std::string nav_label(const service::NavItem& item) const;
  [[nodiscard]] std::string bell_label() const;
  void toggle_sidebar();
  void refresh();
  void open_terminal();
  void shortcut(Shortcut pressed);
  void search_everywhere();
  void answer(bool accepted);
  void follow(const service::Link& link);
  void load_demo();
  void submit();
  void cancel();

  void load_records();
  [[nodiscard]] const records::Record* selected_record() const;
  [[nodiscard]] bool filtered() const;
  [[nodiscard]] std::string_view empty_records_text() const;
  void clear_filters();
  void add_record();
  void edit_record();
  void toggle_record();
  void remove_record();
  void copy_record();
  [[nodiscard]] std::string record_details(const records::Record& record) const;
  void sort_by(std::string_view key, bool descending);
  void choose_columns();
  void import_records();
  void export_records();

  void load_inbox();
  [[nodiscard]] std::string inbox_summary() const;
  [[nodiscard]] std::string_view empty_inbox_text() const;
  void mark_read();
  void mark_all_read();
  void open_notification_record();
  void edit_rule();
  void reset_rule();

  void load_settings();
  [[nodiscard]] std::string setting_note(const settings::Value& value) const;
  void save_setting(std::string_view key, std::string_view raw);
  void reset_setting(std::string_view key);
  [[nodiscard]] std::string database_note() const;
  [[nodiscard]] bool database_locked() const;
  [[nodiscard]] std::string database_path() const;
  void change_database();
  void remove_demo();
  void toggle_notifier();
  [[nodiscard]] std::optional<std::string> launcher_override() const;
  [[nodiscard]] const std::string& front_end() const;
  void remember(std::string_view mode);

  void filter_guide();
  void report_bug();
  [[nodiscard]] std::string diagnostics_text() const;
  void copy_diagnostics();
  void check_database();

 private:
  void report(const AppError& error);
  bool attempt(const std::function<void()>& work);
  void changed(std::string text);
  void ask(Confirm question);
  void run(const Action& action);
  [[nodiscard]] std::string perform(const Action& action);
  [[nodiscard]] const notifier::Autostart& autostart() const;
  [[nodiscard]] std::string open_link(const service::Link& link) const;
  [[nodiscard]] std::vector<std::string> terminal_arguments() const;
  void keep(std::string_view key, std::string_view raw, std::string_view what);
  void submit_add(const Form& sent);
  void submit_edit(const Form& sent, std::int64_t id, std::int64_t revision);
  void submit_columns(const Form& sent);
  void submit_import(const Form& sent);
  void submit_export(const Form& sent);
  void submit_rule(const Form& sent, std::string_view event);
  void submit_move(const Form& sent);
  [[nodiscard]] std::optional<notify::Notification> selected_notification();
  [[nodiscard]] std::optional<notify::Rule> selected_rule();

  service::App* app_;
  Options options_;
  std::string next_;
  bool closing_ = false;
};

}
