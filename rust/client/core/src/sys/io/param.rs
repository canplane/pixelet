// pxl-core/sys/io/param.rs

//! The numbers of the core's I/O: how reads are retried and how many run at
//! once, how many files a turn writes, and how much memory bodies may hold.

/// After the server fails, every read waits before it starts. The longest
/// wait is `RETRY_BASE_MS`, doubled for each failure in a row and never more
/// than `RETRY_CAP_MS` (sys/io/policy.rs `backoff`). After `RETRY_ATTEMPTS`
/// waits, about a minute in all, the server is given up. A network that is
/// down is waited on without end.
pub const RETRY_BASE_MS: f64 = 1_000.0;
pub const RETRY_CAP_MS: f64 = 30_000.0;
pub const RETRY_ATTEMPTS: u32 = 6;

/// The most reads out at once, however many credits the runtime grants.
pub const MAX_CREDITS: u32 = 16;

/// The most files `write` writes in one turn (sys/io/file.rs). Each such file
/// is one batch of the objects' edits (domain/layer/object/repository.rs). A
/// write past this number waits for a later turn. The frames the files may
/// post in a turn count these writes (sys/io/runtime.rs `FILE_FRAMES`).
pub const WRITES_A_TURN: u32 = 8;

/// The most terrain and Earth bodies held at once, in slots of the heap's
/// runs (sys/io/body.rs). A body holds its slot while it is on a credit and
/// until it is decoded; an Earth body holds it until its texels are taken.
/// The number is sized for an eye that moves, not for the worst case.
///
/// A frame grants at most six credits (the client's
/// `bracket.origin.credits`: six on a page and on iOS, five on Android), and
/// at most four Earth bodies wait for their texels. A clip level spans one
/// tile's cells on a side (pxl-engine `clipmap::CLIP_SIZE`), so it wants at
/// most four tiles, and a moving eye refills a few clip levels at once: four
/// levels of four tiles. That makes 6 + 4 + 16 = 26. The cap is 28, four
/// whole runs of seven slots, because the slots of a run past the cap would
/// be held and idle. A jump or a zoom that empties every level wants all
/// their tiles, `CLIP_LEVELS` × 4 = 60, and the tiles past the cap wait for
/// a slot.
///
/// The heap holds a run only while one of its slots is out, so a settled
/// view holds none. At most four runs, 32 MiB, are held, as
/// `HEAP_FLOWS_BYTES` counts them (door/layout.rs). The profiler's
/// `body_slots_peak` measures the slots in use.
pub const BODY_SLOTS: u32 = 28;

/// The size of a run, the block of the heap that body slots are cut from. It
/// is a power of two, so it is exactly one block of the buddy allocator.
pub const BODY_RUN_BYTES: u32 = 8 << 20;

/// The slots a run is cut into. A slot with its share of the run costs 1.14
/// MiB, where a buddy block of its own would cost 2 MiB.
pub const BODY_RUN_SLOTS: u32 = 7;
