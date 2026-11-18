// pxl-core/sys/io/param.rs

//! The numbers of the core's I/O: how reads are retried, how many run at
//! once, and how much memory bodies may hold.

/// After the server fails, every read waits before it starts. The longest
/// wait is `RETRY_BASE_MS`, doubled for each failure in a row and never more
/// than `RETRY_CAP_MS` (sys/io/policy.rs `backoff`). After `RETRY_ATTEMPT_CNT`
/// waits, about a minute in all, the server is given up. A network that is
/// down is waited on without end.
pub(crate) const RETRY_BASE_MS: f64 = 1_000.0;
pub(crate) const RETRY_CAP_MS: f64 = 30_000.0;
pub(crate) const RETRY_ATTEMPT_CNT: u32 = 6;

/// The credits: the most reads at the host at once, the same on every
/// platform: six, as a browser runs to a host over HTTP/1.1 and as
/// `BODY_SLOT_CNT` is sized for. A read holds its credit from its start until
/// it has ended at the host, stopped or not (sys/io/request.rs `Slot`).
/// A read a platform's fetch cannot start at once waits in its pool, which
/// knows nothing of what the core wants first, so the count is kept near what
/// a platform runs.
pub(crate) const CREDIT_CNT: u32 = 6;

/// The body slots in `.fixed` (sys/io/body.rs), the most terrain and imagery
/// bodies held at once. A body holds its slot while it is on a credit and
/// until it is decoded; an imagery body until its texels are taken. The number
/// is sized for an eye that moves, not for the worst case.
///
/// At most six reads are out (`CREDIT_CNT`), and
/// at most four imagery bodies wait for their texels. A clip level spans one
/// tile's cells on a side (pxl-engine `clipmap::CLIP_SIZE`), so it wants at
/// most four tiles, and a moving eye refills a few clip levels at once: four
/// levels of four tiles. That makes 6 + 4 + 16 = 26, and the cap is 32. A
/// jump or a zoom that empties every level wants all their tiles,
/// `CLIP_LEVEL_CNT` × 4 = 60, and the tiles past the cap wait for a slot. The
/// profiler's `body_slots_peak` measures the slots in use.
pub(crate) const BODY_SLOT_CNT: u32 = 32;
