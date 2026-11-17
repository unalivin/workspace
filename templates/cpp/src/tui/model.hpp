#pragma once

#include <chrono>
#include <cstddef>
#include <cstdint>
#include <functional>
#include <map>
#include <optional>
#include <string>
#include <string_view>
#include <variant>
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
#include "tui/text.hpp"
#include "tui/widgets.hpp"

namespace app::tui {

inline constexpr std::size_t min_width = 60;
inline constexpr std::size_t min_height = 16;
inline constexpr std::string_view cancelled = "Cancelled, nothing was changed";

struct Options {
  platform::Env env = platform::Env::system();
  bool system_dark = true;
  std::optional<notifier::Autostart> autostart;
  bool has_dashboard = false;
  std::function<void(std::string_view)> open_url;
};

enum class Notice { Info, Success, Warning, Failure };

struct RemoveRecord {
  std::int64_t id = 0;
  bool operator==(const RemoveRecord&) const = default;
};

struct RemoveDemo {
  bool operator==(const RemoveDemo&) const = default;
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

using Action =
    std::variant<RemoveRecord, RemoveDemo, InstallNotifier, UninstallNotifier, MoveDatabase, ImportFile, OpenLink>;

enum class SubmitKind { AddRecord, EditRecord, Columns, Import, Export, Rule, Setting, MoveDatabase };

struct Submit {
  SubmitKind kind = SubmitKind::AddRecord;
  std::int64_t id = 0;
  std::int64_t revision = 0;
  std::string name;
};

struct OpenForm {
  Form form;
  Submit submit;
};

struct Confirmation {
  std::string title;
  std::string statement;
  std::string accept;
  Action action;
};

struct Preview {
  std::string title;
  std::vector<std::string> lines;
  std::size_t offset = 0;
  std::string accept;
  std::optional<Action> action;
};

struct RecordsPage {
  records::Query query;
  std::vector<records::Record> list;
  std::size_t cursor = 0;
  std::size_t offset = 0;
  bool detail = false;
  bool searching = false;
  Input search;

  RecordsPage();
  [[nodiscard]] const records::Record* selected() const;
  void select(std::int64_t id);
  [[nodiscard]] bool filtered() const;
};

struct InboxPage {
  bool rules = false;
  bool unread_only = false;
  std::vector<notify::Notification> list;
  std::size_t cursor = 0;
  std::vector<notify::Rule> rule_list;
  std::size_t rule_cursor = 0;
};

enum class RowAction { None, Database, Demo, Notifier, Launcher };

struct SettingRow {
  std::string group;
  std::string label;
  std::string value;
  std::string source;
  std::string help;
  std::string key;
  RowAction action = RowAction::None;
};

struct SettingsPage {
  std::vector<SettingRow> rows;
  std::size_t cursor = 0;
  std::size_t offset = 0;
};

enum class AboutTab { Overview, Guide, Shortcuts, Diagnostics, Licenses };

struct LinkRow {
  std::string_view label;
  service::Link link;
  bool issue = false;
};

struct AboutPage {
  AboutTab tab = AboutTab::Overview;
  std::size_t cursor = 0;
  std::size_t scroll = 0;
  guide::Guide usage;
  std::vector<std::size_t> hits;
  std::size_t section = 0;
  bool reading = false;
  bool searching = false;
  Input search;
  std::optional<service::Diagnostics> diagnostics;

  AboutPage();
  void filter();
  [[nodiscard]] std::vector<LinkRow> links() const;
  void turn(std::size_t delta);
  void scroll_key(Key key);
};

class Model {
 public:
  Model(service::App& app, Options options);

  [[nodiscard]] std::string_view switch_to() const;
  [[nodiscard]] bool finished() const;
  [[nodiscard]] Notice notice_kind() const;
  [[nodiscard]] const std::string& notice_text() const;
  void resize(std::size_t columns, std::size_t rows);
  [[nodiscard]] std::chrono::seconds interval() const;
  [[nodiscard]] theme::Style style(Look look) const;
  [[nodiscard]] theme::Profile profile() const;
  void tick();
  void handle(Key key);
  [[nodiscard]] std::string hints() const;
  [[nodiscard]] Lines view();
  [[nodiscard]] std::string screen();
  void reload();
  void open(std::string_view id);
  [[nodiscard]] std::string_view page_id() const;
  [[nodiscard]] const RecordsPage& records_page() const;
  [[nodiscard]] const InboxPage& inbox_page() const;
  [[nodiscard]] const SettingsPage& settings_page() const;
  [[nodiscard]] const AboutPage& about_page() const;
  [[nodiscard]] const Form* form() const;
  [[nodiscard]] const Confirmation* confirmation() const;
  [[nodiscard]] const Preview* preview() const;

 private:
  void say(Notice kind, std::string text);
  void report(const AppError& error);
  void restyle();
  void changed(std::string text);
  void ask(std::string_view title, std::string statement, std::string_view accept, Action action);
  void run(const Action& action);
  [[nodiscard]] std::string perform(const Action& action);
  [[nodiscard]] const notifier::Autostart& autostart() const;
  [[nodiscard]] std::string open_link(const service::Link& link) const;
  void open_guide();
  void toggle_sidebar();
  void load_demo();
  void choose(std::size_t entry);
  [[nodiscard]] bool captures_text() const;
  void page_key(Key key);
  void submit(const Form& form, const Submit& submit);
  [[nodiscard]] bool setting_flag(std::string_view key) const;
  [[nodiscard]] const settings::Value* setting(std::string_view key) const;
  [[nodiscard]] Lines status_lines() const;
  [[nodiscard]] Lines sidebar_view(std::size_t size, std::size_t height) const;
  [[nodiscard]] Lines welcome_view(std::size_t size) const;
  [[nodiscard]] Lines confirm_view(std::size_t size) const;
  [[nodiscard]] Lines preview_view(std::size_t size, std::size_t height);
  [[nodiscard]] Lines menu_view(std::size_t at, std::size_t size) const;
  [[nodiscard]] Lines page_view(std::size_t size, std::size_t height);
  [[nodiscard]] Lines overview_view(std::size_t size) const;

  void load_records();
  void records_key(Key key);
  void record_key(Key key, const records::Record& record);
  void submit_add(const Form& form);
  void submit_edit(const Form& form, std::int64_t id, std::int64_t revision);
  void keep(std::string_view key, std::string_view raw, std::string_view what);
  void sort_by(std::string_view key, bool descending);
  void column_form();
  void submit_columns(const Form& form);
  void submit_import(const Form& form);
  void submit_export(const Form& form);
  [[nodiscard]] std::string filter_line() const;
  [[nodiscard]] Lines records_view(std::size_t size, std::size_t height);

  void load_inbox();
  void inbox_key(Key key);
  void notification_key(Key key);
  void rule_key(Key key);
  void submit_rule(const Form& form, std::string_view event);
  [[nodiscard]] Lines inbox_view(std::size_t size, std::size_t height) const;

  void load_settings();
  void settings_key(Key key);
  void edit_setting(std::string_view key);
  void submit_setting(const Form& form, std::string_view key);
  void submit_move(const Form& form);
  void toggle_notifier();
  [[nodiscard]] Lines settings_view(std::size_t size, std::size_t height);

  void load_about();
  void about_key(Key key);
  [[nodiscard]] Lines about_view(std::size_t size, std::size_t height);

  template <typename Work>
  bool attempt(Work&& work) {
    try {
      std::forward<Work>(work)();
      return true;
    } catch (const AppError& error) {
      report(error);
      return false;
    }
  }

  service::App* app_;
  Options options_;
  std::optional<theme::Terminal> terminal_;
  std::vector<std::string> warnings_;
  std::map<std::string, settings::Value, std::less<>> values_;
  std::size_t width_ = 100;
  std::size_t height_ = 30;
  std::size_t page_ = 0;
  bool sidebar_ = false;
  std::string notice_;
  Notice kind_ = Notice::Info;
  std::optional<OpenForm> form_;
  std::optional<Confirmation> confirm_;
  std::optional<Preview> preview_;
  std::optional<std::size_t> menu_;
  bool welcome_ = false;
  std::string switch_to_;
  bool quitting_ = false;
  records::Overview overview_;
  RecordsPage rec_;
  InboxPage inbox_;
  SettingsPage set_;
  AboutPage about_;
};

[[nodiscard]] bool move_cursor(std::size_t& cursor, Key key, std::size_t count);
[[nodiscard]] std::string short_hints(std::string_view hints, std::size_t size);
[[nodiscard]] std::vector<Option> severity_options();
[[nodiscard]] std::string_view front_end_label(std::string_view mode);

inline constexpr std::int64_t list_limit = 500;

}
