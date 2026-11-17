#include "support/testkit.hpp"

#include <atomic>
#include <chrono>
#include <functional>

#include "core/error.hpp"
#include "core/files.hpp"
#include "core/text.hpp"

namespace testkit {

namespace fs = std::filesystem;

namespace {
std::atomic<std::uint64_t> counter{0};
}

TempDir::TempDir(std::string_view label) {
  const std::uint64_t number = counter.fetch_add(1);
  const auto stamp = std::chrono::steady_clock::now().time_since_epoch().count();
  std::error_code problem;
  fs::path base = fs::canonical(fs::temp_directory_path(), problem);
  if (problem) {
    base = fs::temp_directory_path();
  }
  path_ = base / app::text::concat("templateapp-test-", label, "-", app::files::process_id(), "-", number, "-", stamp);
  fs::create_directories(path_);
}

TempDir::~TempDir() {
  std::error_code ignored;
  fs::remove_all(path_, ignored);
}

const fs::path& TempDir::path() const {
  return path_;
}

app::clock::Timestamp fixed_time() {
  return app::clock::Timestamp::from_civil({2026, 3, 2, 9, 30, 0}).value();
}

fs::path source_root() {
  return app::files::from_utf8(PROJECT_SOURCE_ROOT);
}

std::string failure_of(const std::function<void()>& action) {
  try {
    action();
  } catch (const app::AppError& error) {
    return error.message().empty() ? std::string("(empty message)") : error.message();
  }
  return {};
}

}
