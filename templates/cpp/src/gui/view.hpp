#pragma once

#include <imgui.h>

#include <array>
#include <cstdint>
#include <functional>
#include <optional>
#include <string>
#include <string_view>
#include <vector>

#include "core/theme.hpp"
#include "gui/icons.hpp"
#include "gui/model.hpp"
#include "gui/skin.hpp"
#include "gui/ui.hpp"

namespace app::gui {

inline constexpr std::string_view search_hint = "Search records and the usage guide, then press Enter";
inline constexpr std::string_view demo_note =
    "Sample records are loaded. Remove demo data below deletes only the records whose names start with \"Demo:\".";

enum class MenuAct { Terminal, Quit, Page, Sidebar, Refresh, Guide, Shortcuts, Diagnostics, About };

struct MenuEntry {
  std::string_view menu;
  std::string label;
  std::string keys;
  MenuAct act = MenuAct::Quit;
  std::size_t page = 0;
  bool divider = false;
};

[[nodiscard]] std::string_view nav_icon(std::string_view id);
[[nodiscard]] std::string_view command_key();
[[nodiscard]] std::vector<MenuEntry> menu_entries();
[[nodiscard]] std::string sort_label(std::string_view key);
[[nodiscard]] std::string option_label(std::string_view key, std::string_view option);

class View {
 public:
  View(Dashboard& board, Faces faces, float scale = 1.0F);
  View(const View&) = delete;
  View& operator=(const View&) = delete;

  void prepare();
  void frame();
  void rescale(float scale);
  [[nodiscard]] const std::vector<Seen>& seen() const;
  [[nodiscard]] const Seen* find(SeenKind kind, std::string_view text) const;
  [[nodiscard]] bool shows(std::string_view text) const;
  [[nodiscard]] Icons& icons();
  [[nodiscard]] const theme::Tokens& tokens() const;
  [[nodiscard]] std::uint8_t notice_wash(double now) const;
  [[nodiscard]] float table_width() const;
  [[nodiscard]] float table_room() const;

 private:
  bool panel(const char* id, ImVec2 size, ImVec2 padding, std::optional<theme::Color> fill, bool border,
             ImGuiChildFlags child = ImGuiChildFlags_None, ImGuiWindowFlags window = ImGuiWindowFlags_None) const;
  bool card(const char* id, float height = 0.0F) const;
  void section(std::string_view title) const;
  void keys();
  void menu_bar();
  void choose(const MenuEntry& entry);
  void top_bar(float width, float height);
  [[nodiscard]] float notice_height(float width) const;
  void notice_bar(float width, float height);
  void nav_button(std::size_t index);
  void sidebar(float width, float height);
  void dialogs();
  void form_dialog();
  void confirm_dialog();
  void overview_page();
  void records_page();
  void record_toolbar(records::Query& query);
  void record_detail(const records::Record& record);
  void record_table();
  void inbox_page();
  void inbox_list();
  void inbox_rules();
  void settings_page();
  void setting_row(std::string_view title, std::string_view help, const std::function<void()>& control);
  void setting_control(const settings::Value& value);
  void special_rows(std::string_view group);
  void about_page();
  void about_overview();
  void about_guide();
  void about_shortcuts();
  void about_diagnostics();

  Dashboard* board_;
  Faces faces_;
  float scale_;
  theme::Tokens tokens_;
  Icons icons_;
  std::vector<Seen> seen_;
  Ui ui_;
  std::uint32_t flash_seen_ = 0;
  double flash_time_ = -10.0;
  float panel_height_ = 0.0F;
  float table_width_ = 0.0F;
  float table_room_ = 0.0F;
  std::function<void()> act_;
};

}
