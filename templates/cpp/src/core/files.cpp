#include "core/files.hpp"

#include <atomic>
#include <cerrno>
#include <fstream>
#include <system_error>

#include "core/error.hpp"
#include "core/text.hpp"

#if defined(_WIN32)
#include <process.h>
#else
#include <fcntl.h>
#include <sys/stat.h>
#include <unistd.h>
#endif

namespace app::files {

namespace fs = std::filesystem;

namespace {

std::atomic<std::uint64_t> sequence{0};

fs::path temporary_sibling(const fs::path& path) {
  const std::string name = path.has_filename() ? shown(path.filename()) : std::string("file");
  const std::uint64_t number = sequence.fetch_add(1);
  fs::path sibling = path;
  sibling.replace_filename(from_utf8(text::concat(".", name, ".", process_id(), ".", number, ".tmp")));
  return sibling;
}

void flush_to_disk(const fs::path& path) {
#if defined(_WIN32)
  static_cast<void>(path);
#else
  const int descriptor = ::open(path.c_str(), O_RDONLY | O_CLOEXEC);
  if (descriptor >= 0) {
    static_cast<void>(::fsync(descriptor));
    static_cast<void>(::close(descriptor));
  }
#endif
}

}

std::string shown(const fs::path& path) {
  const std::u8string wide = path.u8string();
  return {wide.begin(), wide.end()};
}

fs::path from_utf8(std::string_view text) {
  return {std::u8string(text.begin(), text.end())};
}

std::uint64_t process_id() {
#if defined(_WIN32)
  return static_cast<std::uint64_t>(_getpid());
#else
  return static_cast<std::uint64_t>(::getpid());
#endif
}

void restrict(const fs::path& path, bool directory) {
#if defined(_WIN32)
  static_cast<void>(path);
  static_cast<void>(directory);
#else
  if (::chmod(path.c_str(), directory ? 0700 : 0600) != 0) {
    throw AppError::io(text::concat("cannot restrict \"", shown(path), "\": ", std::generic_category().message(errno)));
  }
#endif
}

bool is_directory(const fs::path& path) {
  std::error_code ignored;
  return fs::is_directory(path, ignored);
}

bool exists(const fs::path& path) {
  std::error_code ignored;
  return fs::exists(path, ignored);
}

std::uint32_t permissions(const fs::path& path) {
  std::error_code problem;
  const fs::file_status status = fs::status(path, problem);
  if (problem) {
    throw AppError::io(text::concat("cannot inspect \"", shown(path), "\": ", problem.message()));
  }
  return static_cast<std::uint32_t>(status.permissions()) & 0777U;
}

void create_private_dir(const fs::path& path) {
  if (files::is_directory(path)) {
    return;
  }
  std::error_code problem;
  fs::create_directories(path, problem);
  if (problem) {
    throw AppError::io(text::concat("cannot create the directory \"", shown(path), "\": ", problem.message()));
  }
  restrict(path, true);
}

void write_private(const fs::path& path, std::string_view content, bool overwrite) {
  if (files::is_directory(path)) {
    throw AppError::usage(text::concat("\"", shown(path), "\" is a directory: choose a file path"));
  }
  if (!overwrite && files::exists(path)) {
    throw AppError::conflict(text::concat(
        "\"", shown(path), "\" already exists and was not changed: choose another name or allow overwriting"));
  }
  if (path.has_parent_path()) {
    create_private_dir(path.parent_path());
  }
  const fs::path temporary = temporary_sibling(path);
  std::string failure;
  try {
    {
      std::ofstream file(temporary, std::ios::binary | std::ios::trunc);
      if (!file) {
        throw AppError::io("the temporary file cannot be created");
      }
      restrict(temporary, false);
      file.write(content.data(), static_cast<std::streamsize>(content.size()));
      file.flush();
      if (!file) {
        throw AppError::io("the content cannot be written");
      }
    }
    flush_to_disk(temporary);
    std::error_code problem;
    fs::rename(temporary, path, problem);
    if (problem) {
      throw AppError::io(problem.message());
    }
    return;
  } catch (const AppError& error) {
    failure = error.message();
  }
  std::error_code ignored;
  fs::remove(temporary, ignored);
  throw AppError::io(text::concat("cannot write \"", shown(path), "\": ", failure));
}

std::string read_all(const fs::path& path) {
  std::ifstream file(path, std::ios::binary);
  if (!file) {
    throw AppError::io(text::concat("cannot read \"", shown(path), "\": the file cannot be opened"));
  }
  std::string content((std::istreambuf_iterator<char>(file)), std::istreambuf_iterator<char>());
  if (file.bad()) {
    throw AppError::io(text::concat("cannot read \"", shown(path), "\": reading failed"));
  }
  return content;
}

std::string read_limited(const fs::path& path, std::uint64_t limit) {
  std::error_code problem;
  const std::uintmax_t size = fs::file_size(path, problem);
  if (problem) {
    throw AppError::io(text::concat("cannot read \"", shown(path), "\": ", problem.message()));
  }
  if (size > limit) {
    throw AppError::usage(text::concat("\"", shown(path), "\" is larger than ", limit, " bytes and was not read"));
  }
  return read_all(path);
}

}
