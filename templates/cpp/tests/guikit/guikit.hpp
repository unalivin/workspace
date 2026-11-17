#pragma once

#include <imgui.h>

#include <array>
#include <cstddef>
#include <cstdint>
#include <initializer_list>
#include <memory>
#include <string>
#include <string_view>
#include <utility>
#include <vector>

#include "core/platform.hpp"
#include "core/records.hpp"
#include "core/service.hpp"
#include "core/theme.hpp"
#include "gui/model.hpp"
#include "gui/render.hpp"
#include "gui/skin.hpp"
#include "gui/ui.hpp"
#include "gui/view.hpp"
#include "support/testkit.hpp"

namespace guikit {

using Log = std::shared_ptr<std::vector<std::string>>;

[[nodiscard]] Log new_log();
[[nodiscard]] app::platform::Env environment(const testkit::TempDir& home);
[[nodiscard]] app::service::App open_app(const testkit::TempDir& home);
[[nodiscard]] app::gui::Options options(const testkit::TempDir& home);
[[nodiscard]] app::gui::Options wired(const testkit::TempDir& home, const Log& opened, const Log& terminals);
[[nodiscard]] std::string said(const app::gui::Dashboard& board);
void shows(const app::gui::Dashboard& board, std::initializer_list<std::string_view> parts);
void fill(app::gui::Dashboard& board, std::initializer_list<std::pair<std::string_view, std::string_view>> values);
[[nodiscard]] app::gui::Dashboard started(app::service::App& app, app::gui::Options options);
[[nodiscard]] app::records::Input input(std::string_view name, app::theme::Severity severity);
[[nodiscard]] std::vector<std::string> names(const app::gui::Dashboard& board);
[[nodiscard]] bool shown_line(std::string_view text, std::string_view wanted);
[[nodiscard]] bool near(std::array<std::uint8_t, 4> pixel, app::theme::Color tint);
[[nodiscard]] std::size_t tints(const app::gui::Picture& picture);

class Window {
 public:
  Window(app::gui::Dashboard& board, float width, float height);

  void step();
  std::string paint(std::size_t frames = 3);
  [[nodiscard]] app::gui::Picture picture();
  [[nodiscard]] std::string text() const;
  [[nodiscard]] std::vector<std::string> hints() const;
  [[nodiscard]] std::vector<app::gui::Seen> all(std::string_view wanted) const;
  [[nodiscard]] app::gui::Seen leftmost(std::string_view wanted) const;
  [[nodiscard]] app::gui::Seen rightmost(std::string_view wanted) const;
  bool click(std::string_view wanted);
  void click_at(ImVec2 point);
  void hover(ImVec2 point, double seconds);
  void wheel(ImVec2 point, float sideways, float upright);
  void chord(ImGuiKey key, bool command);
  void type(std::string_view words);
  [[nodiscard]] app::gui::View& view();
  [[nodiscard]] app::gui::Stage& stage();

 private:
  app::gui::Stage stage_;
  app::gui::Faces faces_;
  app::gui::View view_;
};

}
