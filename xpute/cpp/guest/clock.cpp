// cpp/guest/clock.cpp

#include "clock.hpp"

namespace xpute {

namespace {
Clock installed = nullptr;
}

void set_clock(Clock clock) noexcept {
    installed = clock;
}

double now() noexcept {
    return installed != nullptr ? installed() : 0.0;
}

} // namespace xpute
