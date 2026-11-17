#include <exception>
#include <filesystem>
#include <iostream>
#include <string>
#include <string_view>
#include <vector>

#include "core/files.hpp"
#include "notices.hpp"

int main(int argc, char** argv) {
  const std::vector<std::string> arguments(argv + 1, argv + argc);
  std::filesystem::path root = app::files::from_utf8(PROJECT_SOURCE_ROOT);
  bool check = false;
  for (std::size_t index = 0; index < arguments.size(); ++index) {
    std::string_view argument = arguments[index];
    while (argument.starts_with('-')) {
      argument.remove_prefix(1);
    }
    if (argument == "check") {
      check = true;
    } else if (argument == "root" && index + 1 < arguments.size()) {
      ++index;
      root = app::files::from_utf8(arguments[index]);
    } else {
      std::cerr << "error: unknown argument " << arguments[index] << ". Usage: gennotices [-root DIRECTORY] [-check]\n";
      return 2;
    }
  }
  try {
    const std::filesystem::path path = root / app::files::from_utf8(tools::notices::notices_path);
    const std::string wanted = tools::notices::render(root);
    const std::string current = std::filesystem::exists(path) ? tools::notices::read_text(path) : std::string();
    if (wanted == current) {
      return 0;
    }
    if (check) {
      std::cerr << "error: THIRD_PARTY_NOTICES.md is out of date: run make notices\n";
      return 1;
    }
    tools::notices::write_text(path, wanted);
    std::cout << "wrote THIRD_PARTY_NOTICES.md\n";
  } catch (const std::exception& problem) {
    std::cerr << "error: " << problem.what() << "\n";
    return 1;
  }
  return 0;
}
