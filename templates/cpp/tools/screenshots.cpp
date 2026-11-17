#include <algorithm>
#include <cstdlib>
#include <filesystem>
#include <fstream>
#include <iostream>
#include <string>
#include <string_view>
#include <vector>

#include "core/clock.hpp"
#include "core/error.hpp"
#include "core/files.hpp"
#include "core/platform.hpp"
#include "core/png.hpp"
#include "core/service.hpp"
#include "core/text.hpp"
#include "gui/model.hpp"
#include "gui/shots.hpp"

namespace {

namespace fs = std::filesystem;

class Scratch {
 public:
  Scratch() {
    const fs::path base = fs::temp_directory_path();
    for (unsigned attempt = 0; attempt < 1000; ++attempt) {
      fs::path candidate = base / app::text::concat("templateapp-screens-", attempt);
      if (fs::create_directory(candidate)) {
        path_ = std::move(candidate);
        return;
      }
    }
    throw app::AppError::io("no temporary directory could be created");
  }
  Scratch(const Scratch&) = delete;
  Scratch& operator=(const Scratch&) = delete;
  ~Scratch() {
    std::error_code ignored;
    fs::remove_all(path_, ignored);
  }

  [[nodiscard]] const fs::path& path() const {
    return path_;
  }

 private:
  fs::path path_;
};

std::size_t render(const fs::path& directory) {
  const Scratch home;
  const app::platform::Env env = app::platform::Env::from_pairs({{"HOME", app::files::shown(home.path())}});
  app::service::Options opening;
  opening.os = app::platform::Os::Linux;
  opening.env = env;
  opening.clock = app::clock::Clock::fixed(app::clock::Timestamp::from_civil({2026, 3, 2, 9, 30, 0}).value());
  opening.zone = app::clock::Zone::utc();
  app::service::App app = app::service::App::open(std::move(opening));
  app::gui::Options options;
  options.env = env;
  fs::create_directories(directory);
  const auto pictures = app::gui::shots::screens(app, std::move(options));
  for (const auto& [name, picture] : pictures) {
    const fs::path file = directory / app::text::concat(name, ".png");
    const app::png::Bytes data = app::png::encode(picture.width, picture.height, picture.pixels);
    std::ofstream out(file, std::ios::binary | std::ios::trunc);
    out.write(reinterpret_cast<const char*>(data.data()), static_cast<std::streamsize>(data.size()));
    if (!out.flush()) {
      throw app::AppError::io(app::text::concat(app::files::shown(file), ": the file could not be written"));
    }
  }
  return pictures.size();
}

}

int main(int argc, char** argv) {
  const std::vector<std::string> arguments(argv + std::min(argc, 1), argv + argc);
  fs::path directory = "dist/screens";
  if (arguments.size() == 2 && arguments[0] == "-out") {
    directory = app::files::from_utf8(arguments[1]);
  } else if (!arguments.empty()) {
    std::cerr << "usage: screenshots [-out DIRECTORY]\n";
    return 2;
  }
  try {
    const std::size_t count = render(directory);
    std::cout << "wrote " << count << " screens to " << app::files::shown(directory) << '\n';
    return EXIT_SUCCESS;
  } catch (const app::AppError& error) {
    std::cerr << "screenshots: " << error.message() << '\n';
    return EXIT_FAILURE;
  } catch (const std::exception& error) {
    std::cerr << "screenshots: " << error.what() << '\n';
    return EXIT_FAILURE;
  }
}
