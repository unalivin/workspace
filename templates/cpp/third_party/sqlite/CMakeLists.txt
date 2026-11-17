add_library(vendored_sqlite STATIC sqlite3.c)
target_include_directories(vendored_sqlite SYSTEM PUBLIC "${CMAKE_CURRENT_SOURCE_DIR}")
target_compile_definitions(
  vendored_sqlite
  PUBLIC SQLITE_THREADSAFE=1
         SQLITE_ENABLE_FTS5
         SQLITE_DQS=0
         SQLITE_DEFAULT_WAL_SYNCHRONOUS=1
         SQLITE_OMIT_DEPRECATED
         SQLITE_OMIT_SHARED_CACHE
         SQLITE_OMIT_LOAD_EXTENSION)
if(NOT MSVC)
  target_compile_options(vendored_sqlite PRIVATE -w)
else()
  target_compile_options(vendored_sqlite PRIVATE /w)
endif()
find_package(Threads REQUIRED)
target_link_libraries(vendored_sqlite PUBLIC Threads::Threads)
if(UNIX AND NOT APPLE)
  target_link_libraries(vendored_sqlite PUBLIC m)
endif()
project_sanitize(vendored_sqlite)
