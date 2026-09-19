// xpute-runtime/sched/quantum.rs

//! The turn's outermost level: what the guest declares about its quantum, and
//! what its falling edge answers with.
//!
//! The central scheduler runs guests the way an OS runs processes: it owns the
//! clock and the resources, and a turn alternates like a clock's two edges. On
//! the rising edge the host hands the guest one quota, the time it may take;
//! the guest spends it on its own tasks cooperatively (sched/tick.rs,
//! sched/edge.rs) and, on the falling edge, hands the turn back with when it
//! wants the next one. The host knows nothing of what runs inside. Memory is
//! granted once (mem.rs) and I/O by credits (io/queue.rs), on the same
//! footing.
//!
//! Sizing that quota is the host's alone (sched/quantum.ts): the frame interval
//! it measures rather than assumes, the overrun a turn carries into its own
//! next quota, whether behaviour makes a turn interactive. None of it is
//! mirrored here — the two sides of `runtime` are the two ends of one
//! arrangement, not one thing implemented twice, and a guest that could size
//! its own quota would not be under a scheduler. What must agree is the
//! format, and the format is this file: three numbers the guest declares in
//! its boot block, one quota in, one wake out.

/// A falling edge that asks for no next edge: the guest waits on something
/// only the host can bring, an input or an arrival.
pub const NO_WAKE: f64 = -1.0;

/// Every number that decides how much, declared by the guest to the central
/// scheduler and never chosen by it.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct QuantumPolicy {
    /// What is left of the frame unspent, as a share of it: the host's own
    /// work, its collector, and the turns the estimate is wrong about.
    pub margin_share: f64,
    /// The frames a turn that is not interactive may take.
    pub batch_frames: f64,
    /// How long after the last input a turn is still interactive.
    pub settle_ms: f64,
}
