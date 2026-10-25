// cpp/xpute/guest/clock.cpp

#include "clock.hpp"

namespace xpute {

namespace {
Clock installed_clock = nullptr;
}

void set_clock(Clock clock) noexcept {
    installed_clock = clock;
}

double now() noexcept {
    return installed_clock != nullptr ? installed_clock() : 0.0;
}

} // namespace xpute
