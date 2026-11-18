// pxl-core/sys/sched/param.rs

//! The numbers that decide how an entry spends its time, and the longest gap
//! a frame steps the world by.

/// The share of a frame's target interval that is the margin: what an entry
/// leaves unspent before the next frame's start, for the host's own work. What
/// it leaves of the interval is the frame's budget (sys/sched.rs `Budget`).
pub(crate) const MARGIN_SHARE: f64 = 0.15;

/// The share of a frame's budget that is one slice: what an entry takes from
/// its own start where the core knows no start of the next frame.
pub(crate) const SLICE_SHARE: f64 = 0.2;

/// The longest gap between frames, in seconds, that the world steps as
/// motion. A longer gap (a hidden tab, a stall) is stepped as this long, so
/// the world pauses instead of leaping.
pub(crate) const MAX_STEP_S: f64 = 0.1;
