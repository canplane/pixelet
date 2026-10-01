// cpp/xpute/runtime/sched/quantum.hpp

// The turn's outermost level: what the guest declares about its quantum, and
// what its falling edge answers with.
//
// The host runs guests the way an OS runs processes: it owns the clock and
// the resources, and a turn alternates like a clock's two edges. On the
// rising edge it hands the guest one quota, the time it may take; the guest
// spends it on its own tasks cooperatively (tick.hpp, edge.hpp) and, on the
// falling edge, hands the turn back with when it wants the next one. Sizing
// that quota is the host's alone; what must agree is the format, and the
// format is this file: three numbers the guest declares in its directory,
// one quota in, one wake out.

#pragma once

namespace xpute {

// A falling edge that asks for no next edge: the guest waits on something
// only the host can bring, an input or an arrival.
inline constexpr double NO_WAKE = -1.0;

// Every number that decides how much, declared by the guest to the host and
// never chosen by it.
struct QuantumPolicy {
    // What is left of the frame unspent, as a share of it: the host's own
    // work, its collector, and the turns the estimate is wrong about.
    double margin_share = 0;
    // The frames a turn that is not interactive may take.
    double batch_frames = 0;
    // How long after the last input a turn is still interactive.
    double settle_ms = 0;

    bool operator==(const QuantumPolicy &) const = default;
};

} // namespace xpute
