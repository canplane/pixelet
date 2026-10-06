// pxl-kernel/param/door.rs

//! The rings to the host.

// ---- the rings ----

/// Signals one command may raise. A command starts only while the completion
/// ring has room for these and its reply, so a larger bound starts fewer a turn.
pub const MOST_SIGNALS: u32 = 64;
