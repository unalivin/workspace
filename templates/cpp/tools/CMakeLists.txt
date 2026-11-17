add_library(app_notices STATIC notices.cpp)
target_include_directories(app_notices PUBLIC "${CMAKE_CURRENT_SOURCE_DIR}")
target_link_libraries(app_notices PUBLIC app_core)
project_quality(app_notices)

add_executable(genassets genassets.cpp)
target_link_libraries(genassets PRIVATE app_core)
project_quality(genassets)

add_executable(gennotices gennotices.cpp)
target_link_libraries(gennotices PRIVATE app_notices)
target_compile_definitions(gennotices PRIVATE "PROJECT_SOURCE_ROOT=\"${PROJECT_SOURCE_DIR}\"")
project_quality(gennotices)

add_executable(gendocs gendocs.cpp)
target_link_libraries(gendocs PRIVATE app_notices app_cli)
target_compile_definitions(gendocs PRIVATE "PROJECT_SOURCE_ROOT=\"${PROJECT_SOURCE_DIR}\"")
project_quality(gendocs)

if(PROJECT_GUI)
  add_executable(screenshots screenshots.cpp)
  target_link_libraries(screenshots PRIVATE app_gui_view)
  project_quality(screenshots)
endif()
