#include "support/fixture.hpp"

#include <string>

namespace testkit {

app::records::Records Fixture::records() const {
  return {store, clock, app::clock::Zone::utc()};
}

app::notify::Notifications Fixture::notifications() const {
  return {store, clock, app::clock::Zone::utc()};
}

app::records::Input input(std::string_view name, app::records::Status status, app::theme::Severity severity) {
  return {std::string(name), status, severity, {}, true};
}

}
