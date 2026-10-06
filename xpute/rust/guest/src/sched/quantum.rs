// xpute-guest/sched/quantum.rs

//! What a guest declares about its quantum, and what its falling edge
//! answers. The host alone sizes the quota.

/// No next edge: the guest waits on an input or an arrival.
pub const NO_WAKE: f64 = -1.0;

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct QuantumPolicy {
    /// The frame's share left for the host's own work and misestimates.
    pub margin_share: f64,
    /// The frames a non-interactive turn may take.
    pub batch_frames: f64,
    /// How long after the last input a turn is still interactive.
    pub settle_ms: f64,
}
