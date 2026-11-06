// cpp/xpute/core/sched/wake.hpp

// What a falling edge answers besides a delay. When the next turn runs, and
// how much it gets, is the runtime's scheduling policy's.

#pragma once

namespace xpute {

// The core waits on something only the runtime can bring, an input or an arrival.
inline constexpr double NO_WAKE = -1.0;

} // namespace xpute
