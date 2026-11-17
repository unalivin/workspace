#include "core/embedded.hpp"

namespace app::embedded {

extern const unsigned char license_data[];
extern const std::size_t license_size;
extern const unsigned char notices_data[];
extern const std::size_t notices_size;
extern const unsigned char usage_data[];
extern const std::size_t usage_size;

namespace {
std::string_view view(const unsigned char* data, std::size_t size) {
  return {reinterpret_cast<const char*>(data), size};
}
}

std::string_view license() {
  return view(license_data, license_size);
}

std::string_view notices() {
  return view(notices_data, notices_size);
}

std::string_view usage() {
  return view(usage_data, usage_size);
}

}
