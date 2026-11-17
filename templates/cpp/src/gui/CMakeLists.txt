find_package(imgui CONFIG REQUIRED)
find_package(glfw3 CONFIG REQUIRED)
find_package(OpenGL REQUIRED)

add_library(app_gui_view STATIC
  appearance.cpp
  chooser.cpp
  extra.cpp
  forms.cpp
  icons.cpp
  ledger.cpp
  model.cpp
  pages.cpp
  render.cpp
  shots.cpp
  skin.cpp
  table.cpp
  ui.cpp
  view.cpp
)
target_link_libraries(app_gui_view PUBLIC app_core imgui::imgui)
project_embed(app_gui_view
  font_regular=src/gui/fonts/NotoSans-Regular.ttf
  font_medium=src/gui/fonts/NotoSans-Medium.ttf
  font_license=src/gui/fonts/OFL.txt
)
project_quality(app_gui_view)

add_library(app_gui STATIC host.cpp)
target_link_libraries(app_gui PUBLIC app_gui_view PRIVATE glfw OpenGL::GL)
project_quality(app_gui)
