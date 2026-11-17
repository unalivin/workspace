#pragma once

#include <string_view>
#include <utility>

#include "core/clock.hpp"
#include "core/notify.hpp"
#include "core/records.hpp"
#include "core/store.hpp"
#include "support/testkit.hpp"

namespace testkit {

struct Fixture {
  explicit Fixture(std::string_view label) : Fixture(label, app::clock::Clock::manual(fixed_time())) {}

  [[nodiscard]] app::records::Records records() const;
  [[nodiscard]] app::notify::Notifications notifications() const;

  TempDir directory;
  app::store::Store store;
  app::clock::Clock clock;
  app::clock::ManualClock hand;

 private:
  Fixture(std::string_view label, std::pair<app::clock::Clock, app::clock::ManualClock> hands)
      : directory(label),
        store(app::store::Store::open_ready(directory.path() / "app.db")),
        clock(std::move(hands.first)),
        hand(std::move(hands.second)) {}
};

[[nodiscard]] app::records::Input input(std::string_view name, app::records::Status status,
                                        app::theme::Severity severity);

}
