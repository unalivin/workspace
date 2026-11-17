#include <exception>
#include <filesystem>
#include <iostream>
#include <string>
#include <string_view>
#include <vector>

#include "app/cli.hpp"
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
      std::cerr << "error: unknown argument " << arguments[index] << ". Usage: gendocs [-root DIRECTORY] [-check]\n";
      return 2;
    }
  }
  try {
    const std::filesystem::path path = root / "docs" / "COMMANDS.md";
    const std::string current = tools::notices::read_text(path);
    const std::string wanted = app::cli::commands_document(current);
    if (wanted == current) {
      return 0;
    }
    if (check) {
      std::cerr << "error: docs/COMMANDS.md is out of date: run make docs\n";
      return 1;
    }
    tools::notices::write_text(path, wanted);
    std::cout << "wrote docs/COMMANDS.md\n";
  } catch (const std::exception& problem) {
    std::cerr << "error: " << problem.what() << "\n";
    return 1;
  }
  return 0;
}
