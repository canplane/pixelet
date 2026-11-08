// xpute-core/sched/wake.rs

//! What a falling edge may answer in place of a delay (`sched::edge`). When
//! the next turn runs, and how long it may run, is up to the runtime's
//! scheduling policy.

/// The core asks for no next turn: it waits for something only the runtime
/// can bring, an input or an arrival.
pub const NO_WAKE: f64 = -1.0;
