// cpp/guest/sched/quantum.hpp

// What the guest declares about its quantum, and what its falling edge
// answers. The host alone sizes the quota; only this format must agree.

#pragma once

namespace xpute {

// The guest waits on something only the host can bring, an input or an arrival.
inline constexpr double NO_WAKE = -1.0;

// Declared by the guest, never chosen by the host.
struct QuantumPolicy {
    // The frame left unspent for the host's own work and misestimates.
    double margin_share = 0;
    // The frames a turn that is not interactive may take.
    double batch_frames = 0;
    // How long after the last input a turn is still interactive.
    double settle_ms = 0;

    bool operator==(const QuantumPolicy &) const = default;
};

} // namespace xpute
