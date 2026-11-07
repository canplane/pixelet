// xpute-core/sched/wake.rs

//! What a falling edge answers besides a delay. When the next turn runs, and
//! how much it gets, is the runtime's scheduling policy's.

/// No next edge: the core waits on an input or an arrival.
pub const NO_WAKE: f64 = -1.0;
