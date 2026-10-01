// cpp/xpute/runtime/clock.hpp

// The host's clock, in milliseconds: what `performance.now()` is to a
// script. The host's is installed before the first turn; until then time
// stands at zero, which is what a test that never installs one runs on.

#pragma once

namespace xpute {

using Clock = double (*)();

// Installs the host's clock.
void set_clock(Clock clock) noexcept;

// The host's time, in milliseconds.
double now() noexcept;

} // namespace xpute
