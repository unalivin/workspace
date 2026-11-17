#include "gui/shots.hpp"

#include "core/launcher.hpp"
#include "core/settings.hpp"
#include "gui/chooser.hpp"
#include "gui/skin.hpp"
#include "gui/ui.hpp"
#include "gui/view.hpp"

namespace app::gui::shots {

namespace {

constexpr std::size_t frames = 3;
constexpr double settle_seconds = 1.0;
constexpr double frame_seconds = 1.0 / 60.0;

}

Picture dashboard(Stage& stage, View& view) {
  for (std::size_t index = 0; index < frames; ++index) {
    view.prepare();
    stage.run([&] { view.frame(); });
  }
  stage.advance(settle_seconds);
  view.prepare();
  stage.run([&] { view.frame(); });
  stage.advance(frame_seconds);
  return stage.picture();
}

Picture chooser(bool dark) {
  Stage stage(chooser::width, chooser::height, 1.0F);
  const Faces faces = install_fonts();
  const theme::Tokens tokens = chooser::tokens(dark);
  apply(tokens);
  Icons icons;
  std::vector<Seen> seen;
  const Ui ui(tokens, faces, icons, seen, 1.0F);
  bool remember = false;
  return stage.settle(frames, [&] {
    seen.clear();
    chooser::show(ui, launcher::mode_gui, remember, false);
  });
}

std::vector<std::pair<std::string_view, Picture>> screens(service::App& app, Options options) {
  std::vector<std::pair<std::string_view, Picture>> shots;
  {
    Stage stage(width, height, 1.0F);
    const Faces faces = install_fonts();
    Dashboard board(app, std::move(options));
    View view(board, faces);
    const auto take = [&](std::string_view name) { shots.emplace_back(name, dashboard(stage, view)); };
    take("first-start");
    board.answer(false);
    board.load_demo();
    board.show(service::nav_overview);
    take("overview");
    board.show(service::nav_records);
    if (!board.rec.list.empty()) {
      board.rec.selected = board.rec.list.front().id;
    }
    take("records");
    board.choose_columns();
    take("columns");
    board.cancel();
    board.show(service::nav_notifications);
    board.inbox.rules = false;
    take("notifications");
    board.inbox.rules = true;
    take("rules");
    board.inbox.rules = false;
    board.show(service::nav_settings);
    take("settings");
    board.save_setting(settings::key_mode, "dark");
    take("settings-dark");
    board.reset_setting(settings::key_mode);
    const std::array<std::pair<std::string_view, AboutTab>, 4> tabs = {{
        {"about", AboutTab::Overview},
        {"guide", AboutTab::Guide},
        {"shortcuts", AboutTab::Shortcuts},
        {"diagnostics", AboutTab::Diagnostics},
    }};
    for (const auto& [name, tab] : tabs) {
      board.show(service::nav_about);
      board.show_about(tab);
      take(name);
    }
    board.show(service::nav_overview);
    board.toggle_sidebar();
    take("collapsed");
    board.toggle_sidebar();
  }
  shots.emplace_back("chooser", chooser(false));
  return shots;
}

}
