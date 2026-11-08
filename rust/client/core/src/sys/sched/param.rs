// pxl-core/sys/sched/param.rs

//! The numbers that decide how a turn spends its time up to the deadline,
//! and the longest gap it steps the world by.

// ---- the turn's budget ----

/// The most, in ms, a pass of the tick may run past what is left of the
/// budget. It keeps steps that report no work from walking the whole list.
pub const HARD_SLACK_MS: f64 = 4.0;

/// The share of the budget held back for content, which would otherwise
/// starve while a load keeps the frame busy. The tick's visible phase may not
/// take it (sys/sched/tick.rs `HELD_BACK`). After the frame, the turn's tasks
/// get this share of the frame however far the frame overran
/// (sys/sched/edge.rs `tasks_after_frame`).
pub const CONTENT_SHARE: f64 = 0.2;

// ---- the world's step ----

/// The longest gap between frames, in seconds, that the world steps as
/// motion. A longer gap (a hidden tab, a stall) is stepped as this long, so
/// the world pauses instead of leaping.
pub const MAX_STEP_S: f64 = 0.1;
