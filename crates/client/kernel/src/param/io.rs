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

// ---- bodies ----

/// Terrain's and the Earth's packets kept past their decode at once, which
/// `.fixed.bodies` holds beside a body on every credit (io/body.rs
/// `SLOTS`): sized to a moving eye, not to the worst.
///
/// A terrain packet is kept only while a window wants a square of its tile
/// still absent (layer/terrain/window.rs `fill`), and a window is a tile's
/// cells a side (pxl-spec-core `window::WINDOW`), so it wants four tiles at
/// most. The fill hands `HANDED` squares a frame, about a row of one
/// window's, so an eye that moves keeps the tiles of the few windows
/// refilling at once: four windows' four is 16. An Earth packet is kept
/// only until its texels are taken, a frame or a few, inside that margin.
/// A jump or a zoom that empties every window wants all their tiles,
/// `WINDOWS` × 4 = 60, past this: the bodies past the slots wait for one
/// given back while the fill goes on from the packets held.
///
/// Each slot is its longest packet's pages of `.fixed`, about a mebibyte
/// whether used or not, against the phone's headroom (mem.json
/// MIN_HEAP_BYTES); fewer makes a moving eye's tiles wait on slots, and at
/// none the view still converges, a tile at a time. The profiler's
/// `body_slots_peak` is what it is measured by.
pub const KEPT_BODIES: u32 = 16;
