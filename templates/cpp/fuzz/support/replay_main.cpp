#include <algorithm>
#include <cstdint>
#include <exception>
#include <filesystem>
#include <fstream>
#include <iostream>
#include <iterator>
#include <string>
#include <string_view>
#include <vector>

#include "support/fuzzing.hpp"

namespace {

namespace fs = std::filesystem;

class Random {
 public:
  explicit Random(std::uint64_t seed) : state_(seed == 0 ? 0x9E3779B97F4A7C15ULL : seed) {}

  [[nodiscard]] std::uint64_t next() {
    state_ ^= state_ << 13U;
    state_ ^= state_ >> 7U;
    state_ ^= state_ << 17U;
    return state_;
  }

  [[nodiscard]] std::size_t below(std::size_t limit) {
    return limit == 0 ? 0 : static_cast<std::size_t>(next() % limit);
  }

 private:
  std::uint64_t state_;
};

constexpr std::string_view interesting =
    "\n\r\t ,;\"'\\-+=@#*:.()[]{}<>|/0123456789abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ";

std::string read(const fs::path& path) {
  std::ifstream file(path, std::ios::binary);
  return {std::istreambuf_iterator<char>(file), std::istreambuf_iterator<char>()};
}

void collect(const fs::path& path, std::vector<std::string>& seeds) {
  if (fs::is_directory(path)) {
    std::vector<fs::path> entries;
    for (const fs::directory_entry& entry : fs::recursive_directory_iterator(path)) {
      if (entry.is_regular_file()) {
        entries.push_back(entry.path());
      }
    }
    std::sort(entries.begin(), entries.end());
    for (const fs::path& entry : entries) {
      seeds.push_back(read(entry));
    }
  } else if (fs::is_regular_file(path)) {
    seeds.push_back(read(path));
  } else {
    throw std::runtime_error("not a file or directory: " + path.string());
  }
}

void feed(const std::string& input) {
  static_cast<void>(LLVMFuzzerTestOneInput(reinterpret_cast<const std::uint8_t*>(input.data()), input.size()));
}

std::string mutated(const std::string& seed, Random& random) {
  std::string out = seed;
  const std::size_t changes = 1 + random.below(4);
  for (std::size_t change = 0; change < changes; ++change) {
    switch (random.below(5)) {
      case 0:
        if (!out.empty()) {
          out.erase(random.below(out.size()), 1 + random.below(4));
        }
        break;
      case 1:
        out.insert(random.below(out.size() + 1), 1, interesting[random.below(interesting.size())]);
        break;
      case 2:
        if (!out.empty()) {
          out[random.below(out.size())] = static_cast<char>(random.below(256));
        }
        break;
      case 3:
        if (!out.empty()) {
          const std::size_t from = random.below(out.size());
          out.insert(random.below(out.size() + 1), out.substr(from, 1 + random.below(16)));
        }
        break;
      default:
        out.resize(random.below(out.size() + 1));
        break;
    }
  }
  return out;
}

std::string noise(Random& random) {
  std::string out(random.below(96), '\0');
  const bool printable = random.below(2) == 0;
  for (char& letter : out) {
    letter = printable ? interesting[random.below(interesting.size())] : static_cast<char>(random.below(256));
  }
  return out;
}

}

int main(int argc, char** argv) {
  const std::vector<std::string> arguments(argv + 1, argv + argc);
  std::size_t rounds = 2000;
  std::vector<std::string> seeds;
  try {
    for (std::size_t index = 0; index < arguments.size(); ++index) {
      if (arguments[index] == "-rounds" && index + 1 < arguments.size()) {
        ++index;
        rounds = static_cast<std::size_t>(std::stoull(arguments[index]));
      } else if (arguments[index].starts_with('-')) {
        std::cerr << "error: unknown argument " << arguments[index] << ". Usage: " << (argc > 0 ? argv[0] : "fuzz")
                  << " [-rounds NUMBER] [FILE or DIRECTORY]...\n";
        return 2;
      } else {
        collect(arguments[index], seeds);
      }
    }
  } catch (const std::exception& problem) {
    std::cerr << "error: " << problem.what() << "\n";
    return 2;
  }
  feed(std::string());
  for (const std::string& seed : seeds) {
    feed(seed);
  }
  Random random(0x5EEDF00DULL + seeds.size());
  for (std::size_t round = 0; round < rounds; ++round) {
    if (!seeds.empty() && round % 2 == 0) {
      feed(mutated(seeds[random.below(seeds.size())], random));
    } else {
      feed(noise(random));
    }
  }
  std::cout << "replayed " << seeds.size() << " seed inputs and " << rounds << " generated inputs\n";
  return 0;
}
