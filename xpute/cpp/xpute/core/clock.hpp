// cpp/xpute/core/clock.hpp

// The runtime's clock, in milliseconds, as `performance.now()` counts them.
// Until a runtime installs one, the time stays at zero, which native tests
// run with.

#pragma once

namespace xpute {

using Clock = double (*)();

void set_clock(Clock clock) noexcept;

double now() noexcept;

} // namespace xpute
