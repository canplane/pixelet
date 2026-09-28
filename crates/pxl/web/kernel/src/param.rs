// pxl-kernel/param.rs

//! The numbers the view's residency and I/O are tuned by that nothing
//! derives: each is a choice, named here once and handed to whatever uses
//! it, so tuning one moves no concern and no code. A number that follows
//! from the structure — the fault bound's pages a level, the frames it
//! takes, the credits a host grants — is not here but beside what it is
//! derived from.
//!
//! None of them is a correctness bound. Residency's capacity is asserted
//! from the structure (world/paging.rs), and every value here can be set to
//! its least and the view still converges to exact, only later.

/// How much wider than the reach prefetch asks, as a factor of each
/// level's reach radius (world/prefetch.rs `Reach`, which the kernel runs
/// only once it is chosen in world/pyramid.rs `PREFETCH`). 1 is the reach
/// itself.
pub const PREFETCH_SLACK: f64 = 1.0;

/// How far past the reach prefetch asks for the eye's motion, in units:
/// what a round trip at the eye's speed would carry it (world/prefetch.rs
/// `Reach`). Nothing measures the speed yet, so none.
pub const MOTION_MARGIN: f64 = 0.0;

/// The most speculative pages a layer asks for at once, past what free
/// frames already limit them to (world/pyramid.rs `Pyramid::observe`).
pub const HINT_ALLOWANCE: u32 = 128;

/// How long an identity whose attempt failed waits before another, in
/// milliseconds: long enough not to ask straight back into whatever turned
/// it away (io/policy.rs).
pub const RETRY_MS: f64 = 2_000.0;

/// The most attempts one pump starts (io/policy.rs). The credits already
/// bound what is out; this bounds only how fast they are taken up.
pub const INTAKE: u32 = u32::MAX;
