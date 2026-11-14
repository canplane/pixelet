// pxl-core/sys/io/param.rs

//! The numbers of the core's I/O: how reads are retried and how many run at
//! once, how many files a frame writes, and how much memory bodies may hold.

/// After the server fails, every read waits before it starts. The longest
/// wait is `RETRY_BASE_MS`, doubled for each failure in a row and never more
/// than `RETRY_CAP_MS` (sys/io/policy.rs `backoff`). After `RETRY_ATTEMPT_CNT`
/// waits, about a minute in all, the server is given up. A network that is
/// down is waited on without end.
pub const RETRY_BASE_MS: f64 = 1_000.0;
pub const RETRY_CAP_MS: f64 = 30_000.0;
pub const RETRY_ATTEMPT_CNT: u32 = 6;

/// The most reads out at once, however many credits are granted.
pub const CREDIT_CAP: u32 = 16;

/// How many reads a frame grants: what the platform's fetch runs at once. A
/// browser opens six connections to a host over HTTP/1.1, and a fetch past them
/// waits in the browser's pool, which knows nothing of what the core wants
/// first. On Android, Expo's OkHttp client runs at most five calls to a host
/// (`Dispatcher` `maxRequestsPerHost`, which React Native's OkHttpClientProvider
/// leaves at its default). On iOS, the origin speaks HTTP/2 over one
/// connection, so nothing caps the reads below the six `BODY_SLOT_CNT` is sized
/// for.
pub const CREDIT_CNT: u32 = if cfg!(target_os = "android") { 5 } else { 6 };

/// The body slots in `.fixed` (sys/io/body.rs), the most terrain and Earth
/// bodies held at once. A body holds its slot while it is on a credit and
/// until it is decoded; an Earth body until its texels are taken. The number
/// is sized for an eye that moves, not for the worst case.
///
/// A frame grants at most six credits (`CREDIT_CNT`), and
/// at most four Earth bodies wait for their texels. A clip level spans one
/// tile's cells on a side (pxl-engine `clipmap::CLIP_SIZE`), so it wants at
/// most four tiles, and a moving eye refills a few clip levels at once: four
/// levels of four tiles. That makes 6 + 4 + 16 = 26, and the cap is 32. A
/// jump or a zoom that empties every level wants all their tiles,
/// `CLIP_LEVEL_CNT` × 4 = 60, and the tiles past the cap wait for a slot. The
/// profiler's `body_slots_peak` measures the slots in use.
pub const BODY_SLOT_CNT: u32 = 32;
