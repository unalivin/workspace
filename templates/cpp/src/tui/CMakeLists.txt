add_library(app_tui STATIC
  about_page.cpp
  inbox_page.cpp
  model.cpp
  records_page.cpp
  screen.cpp
  settings_page.cpp
  text.cpp
  widgets.cpp
)
target_link_libraries(app_tui
  PUBLIC app_core ftxui::screen ftxui::dom ftxui::component
)
project_quality(app_tui)
