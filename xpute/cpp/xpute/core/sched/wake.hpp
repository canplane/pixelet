// cpp/xpute/core/sched/wake.hpp

// A turn's end may answer this in place of a delay (sched/turn.hpp). When the
// next turn runs, and how long it may run, is up to the runtime's
// scheduling policy.

#pragma once

namespace xpute {

// The core asks for no next turn: it waits for something only the runtime
// can bring, an input or an arrival.
inline constexpr double NO_WAKE = -1.0;

} // namespace xpute
