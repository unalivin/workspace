#include "core/userconfig.hpp"

#include <nlohmann/json.hpp>

#include "core/error.hpp"
#include "core/files.hpp"
#include "core/text.hpp"

namespace app::userconfig {

namespace fs = std::filesystem;

namespace {

std::string member(const nlohmann::json& document, const char* name, const fs::path& path) {
  const auto found = document.find(name);
  if (found == document.end()) {
    return {};
  }
  if (!found->is_string()) {
    throw AppError::io(text::concat("\"", files::shown(path), "\" is not valid: ", name,
                                    " must be text, delete it to start with defaults"));
  }
  return found->get<std::string>();
}

}

void Config::validate() const {
  if (!front_end.empty() && front_end != front_end_gui && front_end != front_end_tui) {
    throw AppError::usage(text::concat("front_end must be \"", front_end_gui, "\", \"", front_end_tui,
                                       "\" or empty, found \"", front_end, "\""));
  }
  if (!database_path.empty() && !files::from_utf8(database_path).is_absolute()) {
    throw AppError::usage(text::concat("database_path must be an absolute path, found \"", database_path, "\""));
  }
}

fs::path file(const fs::path& directory) {
  return directory / "config.json";
}

Config load(const fs::path& directory) {
  const fs::path path = file(directory);
  if (!files::exists(path)) {
    return {};
  }
  std::string raw;
  try {
    raw = files::read_limited(path, max_file_size);
  } catch (const AppError& error) {
    throw AppError::io(text::concat(error.message(), ": delete it to start with defaults"));
  }
  const nlohmann::json document = nlohmann::json::parse(raw, nullptr, false);
  if (document.is_discarded() || !document.is_object()) {
    throw AppError::io(
        text::concat("\"", files::shown(path), "\" is not valid JSON: delete it to start with defaults"));
  }
  Config config{member(document, "database_path", path), member(document, "front_end", path)};
  try {
    config.validate();
  } catch (const AppError& error) {
    throw AppError::io(text::concat("\"", files::shown(path), "\": ", error.message()));
  }
  return config;
}

void save(const fs::path& directory, const Config& config) {
  config.validate();
  files::create_private_dir(directory);
  nlohmann::ordered_json document;
  document["database_path"] = config.database_path;
  document["front_end"] = config.front_end;
  files::write_private(file(directory), document.dump(2) + "\n", true);
}

}
