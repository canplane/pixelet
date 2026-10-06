// pxl-kernel/param/io.rs

//! How records are fetched and retried.

// ---- the pump ----

/// Attempts one pump starts; credits already bound what is out.
pub const INTAKE: u32 = u32::MAX;

// ---- I/O robustness ----

pub const RETRY_MS: f64 = 2_000.0;

/// Most fetches out at once, whatever the host grants.
pub const MAX_CREDITS: u32 = 16;
