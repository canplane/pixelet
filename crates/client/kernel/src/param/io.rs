// pxl-kernel/param/io.rs

//! How records are fetched and retried.

// ---- the pump ----

/// Attempts one pump starts; credits already bound what is out.
pub const INTAKE: u32 = u32::MAX;

// ---- I/O robustness ----

/// A server that fails holds every read back up to `RETRY_BASE_MS` doubled per
/// failure in a row, never past `RETRY_CAP_MS`; past `RETRY_ATTEMPTS` waits,
/// about a minute in all, it is given up. A network that is down is waited on.
pub const RETRY_BASE_MS: f64 = 1_000.0;
pub const RETRY_CAP_MS: f64 = 30_000.0;
pub const RETRY_ATTEMPTS: u32 = 6;

/// Most fetches out at once, whatever the host grants.
pub const MAX_CREDITS: u32 = 16;
