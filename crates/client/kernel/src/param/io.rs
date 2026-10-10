// pxl-kernel/param/io.rs

//! How records are fetched and retried.

// ---- the pump ----

/// Attempts one pump starts; credits already bound what is out.
pub const INTAKE: u32 = u32::MAX;

// ---- I/O robustness ----

/// A failed read waits up to `RETRY_BASE_MS` doubled per failure, never past
/// `RETRY_CAP_MS`; a server that fails `RETRY_ATTEMPTS` times is given up,
/// about a minute of waits at most. A network that is down is waited on.
pub const RETRY_BASE_MS: f64 = 1_000.0;
pub const RETRY_CAP_MS: f64 = 30_000.0;
pub const RETRY_ATTEMPTS: u32 = 6;

/// Most fetches out at once, whatever the host grants.
pub const MAX_CREDITS: u32 = 16;
