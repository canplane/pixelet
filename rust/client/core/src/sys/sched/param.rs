// pxl-core/sys/sched/param.rs

//! The numbers that decide how an entry spends its time, and the longest gap
//! a frame steps the world by.

/// The time, in ms, an entry's tasks may take from its start before they
/// yield (sys/sched.rs `spent`).
pub(crate) const BUDGET_MS: f64 = 8.0;

/// The longest gap between frames, in seconds, that the world steps as
/// motion. A longer gap (a hidden tab, a stall) is stepped as this long, so
/// the world pauses instead of leaping.
pub(crate) const MAX_STEP_S: f64 = 0.1;
