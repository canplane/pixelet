// pxl-kernel/sys/sched/param.rs

//! How a turn's quota is spent.

// ---- the turn's budget ----

/// How far a pass may overrun, so a step that reports nothing done cannot walk the whole list.
pub const HARD_SLACK_MS: f64 = 4.0;

/// Share of the budget held back for content, which would otherwise starve
/// while a load keeps the frame busy: from the tick's visible phase, and past
/// the frame for the turn's tasks (sys/sched/edge.rs `tasks_after_frame`), where
/// it is the frame's share however far the frame overran.
pub const CONTENT_SHARE: f64 = 0.2;
