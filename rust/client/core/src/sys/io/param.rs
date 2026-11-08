// pxl-core/sys/io/param.rs

//! How records are fetched, retried and written.

// ---- I/O robustness ----

/// A server that fails holds every read back up to `RETRY_BASE_MS` doubled a
/// failure in a row, never past `RETRY_CAP_MS`, and is given up past
/// `RETRY_ATTEMPTS` waits, about a minute in all. A network that is down is
/// waited on.
pub const RETRY_BASE_MS: f64 = 1_000.0;
pub const RETRY_CAP_MS: f64 = 30_000.0;
pub const RETRY_ATTEMPTS: u32 = 6;

/// Most fetches out at once, whatever the runtime grants.
pub const MAX_CREDITS: u32 = 16;

// ---- writes ----

/// Most files written in one turn by number (sys/io/file.rs `write`), an
/// edit batch of the objects' each (domain/layer/object/repository.rs); one
/// past them waits for a later turn. Counted in the files' frames a turn
/// (sys/io/runtime.rs `FILE_FRAMES`).
pub const WRITES_A_TURN: u32 = 8;

// ---- bodies ----

/// The most terrain and Earth bodies out at once, in slots of the heap's
/// runs (sys/io/body.rs): those on a credit until decoded, and the Earth's
/// past it until its texels are taken. Sized to a moving eye, not to the
/// worst.
///
/// A frame grants six credits at most (the client's
/// `bracket.origin.credits`: six on a page and on iOS, five on Android), and
/// the Earth keeps four frames at most. A clip level is a tile's cells a side
/// (pxl-engine `clipmap::CLIP_SIZE`), so it wants four tiles at most, and an
/// eye that moves refills a few clip levels at once: four levels' four.
/// 6 + 4 + 16 is 26, and 28 is four whole runs of seven, as a run's slots
/// past the cap would be borrowed and idle. A jump or a zoom that empties
/// every level wants all their tiles, `CLIP_LEVELS` × 4 = 60: what is past
/// the cap waits for a slot.
///
/// The heap gives the runs only while their slots are out, so a settled view
/// holds none, and at most four, 32 MiB, as `HEAP_FLOWS_BYTES` counts
/// (door/layout.rs). The profiler's `body_slots_peak` measures it.
pub const BODY_SLOTS: u32 = 28;

/// The run the heap gives slots in: a power of two, the buddy's own block,
/// seven slots at today's packets, so a slot with its share of the run costs
/// 1.14 MiB where the buddy's block would be 2.
pub const BODY_RUN_BYTES: u32 = 8 << 20;
