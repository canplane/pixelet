// pxl-core/sys/sched/param.rs

//! The numbers that decide how an entry spends its time, and the longest gap
//! a frame steps the world by.

/// The share of a frame's target interval left unspent, for the host's own
/// work and for error in the estimate. What it leaves is the frame's budget
/// (sys/sched.rs `deadline`).
pub(crate) const MARGIN_SHARE: f64 = 0.15;

/// The share of the frame's budget kept for content: the visible phase may
/// not take it (`reserve`), and an entry gets it from its start however far
/// the frame ran past its deadline.
pub(crate) const CONTENT_SHARE: f64 = 0.2;

/// The longest gap between frames, in seconds, that the world steps as
/// motion. A longer gap (a hidden tab, a stall) is stepped as this long, so
/// the world pauses instead of leaping.
pub(crate) const MAX_STEP_S: f64 = 0.1;
