// cpp/xpute/guest/clock.hpp

// The host's clocks (`wit/clocks/monotonic-clock.wit`,
// `wit/clocks/system-clock.wit`), called in an entry, as `xpute-guest`'s
// `clock.rs` reads them. They count nanoseconds, as wasi:clocks does; the
// program reads them in ms, converted here alone.

#pragma once

#include <cstdint>

namespace xpute::guest::clock {

// The time in ms, which never runs backward; only differences mean anything.
double now() noexcept;

// A mark of the monotonic clock in ms, on `now`'s line, or a duration
// between two in ms: a time the host answers, as a display's frame start,
// or a span of one, as the gap since the frame before.
double ms(std::uint64_t mark) noexcept;

// A time in ms on `now`'s line as a mark of the monotonic clock, rounded
// down, and none before the clock's start (0): a time the program asks the
// host for, as a display's frame start.
std::uint64_t mark(double ms) noexcept;

// `ms` as a duration the monotonic clock waits for, rounded up.
std::uint64_t duration(double ms) noexcept;

// The system clock, in Unix ms.
double system() noexcept;

} // namespace xpute::guest::clock
