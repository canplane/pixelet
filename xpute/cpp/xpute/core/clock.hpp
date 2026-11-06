// cpp/xpute/core/clock.hpp

// Milliseconds, as `performance.now()`. Until the runtime installs one, time
// stands at zero, which tests run on.

#pragma once

namespace xpute {

using Clock = double (*)();

void set_clock(Clock clock) noexcept;

double now() noexcept;

} // namespace xpute
