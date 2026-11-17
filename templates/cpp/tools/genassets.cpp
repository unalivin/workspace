#include <exception>
#include <filesystem>
#include <iostream>
#include <string>
#include <string_view>
#include <vector>

#include "core/brand.hpp"
#include "core/error.hpp"
#include "core/meta.hpp"

int main(int argc, char** argv) {
  const std::vector<std::string> arguments(argv + 1, argv + argc);
  std::filesystem::path directory = "assets";
  bool check = false;
  for (std::size_t index = 0; index < arguments.size(); ++index) {
    std::string_view argument = arguments[index];
    while (argument.starts_with('-')) {
      argument.remove_prefix(1);
    }
    if (argument == "check") {
      check = true;
    } else if (argument == "out" && index + 1 < arguments.size()) {
      ++index;
      directory = arguments[index];
    } else if (argument == "out") {
      std::cerr << "error: -out needs a directory\n";
      return 2;
    } else {
      std::cerr << "error: unknown argument " << arguments[index] << ". Usage: genassets [-out DIRECTORY] [-check]\n";
      return 2;
    }
  }
  try {
    const std::vector<app::brand::File> files =
        app::brand::files(app::meta::app_name, app::meta::tagline, app::meta::slug);
    for (const std::string& path : app::brand::write_files(directory, files, check)) {
      std::cout << "wrote " << path << "\n";
    }
  } catch (const std::exception& problem) {
    std::cerr << "error: " << problem.what() << "\n";
    return 1;
  }
  return 0;
}
