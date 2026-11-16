// cpp/xpute/guest/clock.cpp

#include "clock.hpp"

#include <cmath>

#include "xpute/kit/status/bug.hpp"
#include "xpute/guest/abi/clocks/monotonic_clock.hpp"
#include "xpute/guest/abi/clocks/system_clock.hpp"

namespace xpute::guest::clock {

namespace {
constexpr double NS_PER_MS = 1e6;
}

double now() noexcept {
    return ms(or_bug(abi::clocks::monotonic_clock::now(), Errno::eproto));
}

double ms(abi::clocks::monotonic_clock::Mark mark) noexcept {
    return static_cast<double>(mark) / NS_PER_MS;
}

std::uint64_t mark(double ms) noexcept {
    return static_cast<std::uint64_t>(std::floor((ms > 0 ? ms : 0) * NS_PER_MS));
}

std::uint64_t duration(double ms) noexcept {
    return static_cast<std::uint64_t>(std::ceil((ms > 0 ? ms : 0) * NS_PER_MS));
}

double system() noexcept {
    abi::clocks::system_clock::Instant t = or_bug(abi::clocks::system_clock::now(), Errno::eproto);
    return static_cast<double>(t.seconds) * 1e3 + static_cast<double>(t.nanoseconds) / NS_PER_MS;
}

} // namespace xpute::guest::clock
