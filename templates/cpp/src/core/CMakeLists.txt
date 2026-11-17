configure_file("${CMAKE_CURRENT_SOURCE_DIR}/build_config.hpp.in" "${CMAKE_CURRENT_BINARY_DIR}/generated/core/build_config.hpp" @ONLY)

file(GLOB core_sources CONFIGURE_DEPENDS "${CMAKE_CURRENT_SOURCE_DIR}/*.cpp")
add_library(app_core STATIC ${core_sources})
target_include_directories(app_core PUBLIC "${PROJECT_SOURCE_DIR}/src" "${CMAKE_CURRENT_BINARY_DIR}/generated")
target_link_libraries(app_core PUBLIC vendored_sqlite PRIVATE nlohmann_json::nlohmann_json)
if(WIN32)
  target_link_libraries(app_core PRIVATE shell32)
endif()
project_quality(app_core)

file(GLOB migration_files CONFIGURE_DEPENDS "${PROJECT_SOURCE_DIR}/migrations/*.sql")
list(SORT migration_files)
set(embedded_entries "license=LICENSE" "notices=THIRD_PARTY_NOTICES.md" "usage=docs/USAGE.md")
set(migration_externs "")
set(migration_rows "")
foreach(migration_file IN LISTS migration_files)
  get_filename_component(migration_name "${migration_file}" NAME)
  get_filename_component(migration_stem "${migration_file}" NAME_WE)
  list(APPEND embedded_entries "migration_${migration_stem}=${migration_file}")
  string(APPEND migration_externs
         "extern const unsigned char migration_${migration_stem}_data[];\nextern const std::size_t migration_${migration_stem}_size;\n")
  string(APPEND migration_rows
         "    {\"${migration_name}\", {reinterpret_cast<const char*>(migration_${migration_stem}_data), migration_${migration_stem}_size}},\n")
endforeach()
file(
  WRITE "${CMAKE_CURRENT_BINARY_DIR}/embedded/migrations_table.cpp.new"
  "#include \"core/embedded.hpp\"\n\nnamespace app::embedded {\n${migration_externs}\nstd::vector<File> migrations() {\n  return {\n${migration_rows}  };\n}\n}\n")
configure_file("${CMAKE_CURRENT_BINARY_DIR}/embedded/migrations_table.cpp.new"
               "${CMAKE_CURRENT_BINARY_DIR}/embedded/migrations_table.cpp" COPYONLY)
target_sources(app_core PRIVATE "${CMAKE_CURRENT_BINARY_DIR}/embedded/migrations_table.cpp")
project_embed(app_core ${embedded_entries})
