find_package(doctest CONFIG REQUIRED)

file(GLOB_RECURSE test_sources CONFIGURE_DEPENDS "${CMAKE_CURRENT_SOURCE_DIR}/core/*.cpp"
     "${CMAKE_CURRENT_SOURCE_DIR}/app/*.cpp" "${CMAKE_CURRENT_SOURCE_DIR}/tui/*.cpp"
     "${CMAKE_CURRENT_SOURCE_DIR}/support/*.cpp")
if(PROJECT_GUI)
  file(GLOB_RECURSE gui_test_sources CONFIGURE_DEPENDS "${CMAKE_CURRENT_SOURCE_DIR}/gui/*.cpp"
       "${CMAKE_CURRENT_SOURCE_DIR}/guikit/*.cpp")
  list(APPEND test_sources ${gui_test_sources})
endif()
add_executable(templateapp_tests main.cpp ${test_sources})
target_include_directories(templateapp_tests PRIVATE "${CMAKE_CURRENT_SOURCE_DIR}")
target_compile_definitions(templateapp_tests PRIVATE "PROJECT_SOURCE_ROOT=\"${PROJECT_SOURCE_DIR}\"")
target_link_libraries(templateapp_tests PRIVATE app_cli app_tui app_core doctest::doctest nlohmann_json::nlohmann_json)
if(PROJECT_GUI)
  target_link_libraries(templateapp_tests PRIVATE app_gui_view)
endif()
project_quality(templateapp_tests)
add_test(NAME templateapp_tests COMMAND templateapp_tests)
