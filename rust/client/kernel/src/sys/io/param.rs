// pxl-kernel/sys/io/param.rs

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

// ---- bodies ----

/// The most terrain and Earth bodies out at once, in slots of the heap's
/// runs (sys/io/body.rs): those on a credit until their decode, and the packets
/// kept past it, a terrain tile's while a window wants one of its squares
/// still absent (domain/layer/terrain/window.rs `fill`), the Earth's until its
/// texels are taken. Sized to a moving eye, not to the worst.
///
/// Each frame grants six credits (`NET_CREDITS` in the client's
/// abi/control/input.ts), and the
/// Earth keeps four frames at most. A window is a tile's cells a side
/// (pxl-spec-core `window::WINDOW`), so it wants four tiles at most, and
/// the fill hands `HANDED` squares a frame, about a row of one window's: an
/// eye that moves keeps the tiles of the few windows refilling at once,
/// four windows' four. 6 + 4 + 16 is 26, and 28 is four whole runs of
/// seven, since a run's slots past the cap would be borrowed and idle. A
/// jump or a zoom that empties every window wants all their tiles,
/// `WINDOWS` × 4 = 60, past this: what is past it waits for a slot.
///
/// The heap gives the runs only while their slots are out, so a settled
/// view holds none, and at most four runs, 32 MiB, as `HEAP_FLOWS_BYTES`
/// counts (door/layout.rs). Fewer makes a moving eye's tiles wait; the
/// profiler's `body_slots_peak` is what it is measured by.
pub const BODY_SLOTS: u32 = 28;

/// The run the heap gives slots in: a power of two, the buddy's own block,
/// seven slots at today's packets, so a slot with its share of the run
/// costs 1.14 MiB where the buddy's block would be 2.
pub const BODY_RUN_BYTES: u32 = 8 << 20;
