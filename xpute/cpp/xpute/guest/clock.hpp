// cpp/xpute/guest/clock.hpp

// The host's clocks (wit/clocks/monotonic-clock.wit,
// wit/clocks/system-clock.wit), called in an entry, as xpute-guest's clock.rs
// reads them. They count nanoseconds, as wasi:clocks does; the program reads
// them in ms, converted here alone.

#pragma once

#include <cstdint>

namespace xpute::guest::clock {

// The time in ms, which never runs backward; only differences mean anything.
double now() noexcept;

// `ms` as a duration the monotonic clock waits for, rounded up.
std::uint64_t duration(double ms) noexcept;

// The system clock, in Unix ms.
double system() noexcept;

} // namespace xpute::guest::clock
