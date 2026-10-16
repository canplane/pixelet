// pxl-kernel/param/sched.rs

//! How a turn's quota is spent.

// ---- the turn's budget ----

/// Share of the frame left unspent for the host's own work; a share, so a
/// fast display does not lose most of its frame to it.
pub const QUANTUM_MARGIN_SHARE: f64 = 0.15;

/// Frames a non-interactive turn may take: with nothing moving, two vsyncs
/// only show the same image twice.
pub const QUANTUM_BATCH_FRAMES: f64 = 2.0;

/// How long after the last input a turn is still interactive, so a pause
/// mid-drag does not count.
pub const QUANTUM_SETTLE_MS: f64 = 150.0;

/// How far a pass may overrun, so a step that reports nothing done cannot walk the whole list.
pub const HARD_SLACK_MS: f64 = 4.0;

/// Share of the budget held back for content, which would otherwise starve
/// while a load keeps the frame busy: from the tick's visible phase, and past
/// the frame for the turn's tasks (sched/edge.rs `tasks_after_frame`), where
/// it is the frame's share however far the frame overran.
pub const CONTENT_SHARE: f64 = 0.2;
