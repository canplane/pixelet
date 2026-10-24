// pxl-kernel/param/view.rs

//! What the eye observes and how finely the world is drawn for it.

// ---- the drawn resolution ----

/// Device pixels per CSS pixel the world is drawn at: the display's, no
/// finer than labels' sheets are drawn, so a label is never shown smaller
/// than drawn, and under load a share of them (view/frame/resolution.rs).
pub const RESOLUTION_MAX: f64 = pxl_spec::space::camera::grid::RATIO_MAX;
const _: () = assert!(RESOLUTION_MAX <= pxl_render::gpu::sheet::SHEET_SCALE, "a label's sheet is drawn at least as fine as the world");

/// The least share of the display's pixels, each way, the world is drawn at.
pub const SCALE_MIN: f64 = 0.5;

/// The steps from whole down to `SCALE_MIN`, each a like share of the last:
/// two of them halve the pixels.
pub const SCALE_STEPS: i32 = 4;

/// The budgets a window of the load sums, in ms: what a step down waits
/// for.
pub const SCALE_WINDOW_MS: f64 = 500.0;

/// The windows in a row the step above must fit in before it is taken,
/// where the device reports GPU time.
pub const SCALE_UP_WINDOWS: u32 = 4;

/// A window within this of its budget, either way, holds the step.
pub const SCALE_BAND: f64 = 0.1;

/// The tightest budget: 60 Hz. Faster displays would trade sharpness for
/// frames the eye gains less from.
pub const SCALE_BUDGET_FLOOR_MS: f64 = 1000.0 / 60.0;

/// A change rebuilds the render targets, so the windows after one, a second
/// of them, are not judged.
pub const SCALE_SETTLE_WINDOWS: u32 = 2;

/// Where the device reports no GPU time, a window whose frames' gap averages
/// past this many budgets missed frames.
pub const SCALE_MISSED: f64 = 1.5;

/// Where the device reports no GPU time, the quiet windows before the first
/// probe up, 5 s, and the most a failed probe doubles them to, 60 s.
pub const SCALE_PROBE_WINDOWS: u32 = 10;
pub const SCALE_PROBE_WINDOWS_MAX: u32 = 120;

/// The windows after a probe up, 2 s, within which going over takes it back
/// and doubles the wait before the next.
pub const SCALE_TRUST_WINDOWS: u32 = 4;

/// The picker shows a case at thumbnail size: its buffer's long side, in pixels.
pub const CASE_LONG_SIDE_MAX: f64 = 256.0;

// ---- the projection ----

pub const FOV: f64 = 50.0;
pub const NEAR: f64 = 0.1;

// ---- the ways' pool ----

/// The most the ways' pool takes on each side, the GPU's buffer and the
/// CPU's mirror (layer/terrain/mirror.rs), 64 MiB: the bake's bound,
/// `ways::tree::POOL_BLOCKS` blocks of 64 B, which every set fits wherever
/// the eye is (pxl-spec bake/tile/terrain.rs `pool_needs`), so at this no
/// square of a baked set is refused. Less refuses a dense view's last
/// squares their ways' trees, each left absent to the coarser window
/// (layer/terrain/window.rs `REFUSED`); the mirror at it is a heap flow
/// (door/layout.rs `HEAP_FLOWS_BYTES`).
///
/// A known gap: the GPU's pool at it binds within WebGPU's floor, asserted
/// (pxl-render gpu/renderer.rs `pool_bytes`), so no device's limits refuse
/// it, but a device out of memory may fail to make the doubled buffer, and
/// nothing hears it: the host keeps no error scope, so the renderer reads
/// on with a buffer the device holds as an error. The last doubling holds
/// the old pool beside the new, 32 MiB with 64, until the copy is submitted.
pub const WAYS_POOL_BYTES: u32 = 64 << 20;
const _: () = assert!(
    WAYS_POOL_BYTES <= pxl_spec_core::ways::tree::POOL_BLOCKS * 64 && WAYS_POOL_BYTES >= 2 * 64,
    "the pool's cap is no more than the bake's bound, nor less than a block past block 0"
);

/// The ways' pool on the GPU at first, 1 MiB, 16 Ki blocks: a square with
/// no ways holds its root block alone, so the 960 squares' roots and some
/// ways. It doubles each time a build runs out (pxl-render
/// gpu/renderer.rs `GroundBuffers::grow`), the square that ran out filled
/// again a turn later, so a dense view reaches 64 MiB in six. Not yet
/// measured against a dense view.
pub const WAYS_POOL_FIRST_BYTES: u32 = 1 << 20;
const _: () = assert!(WAYS_POOL_FIRST_BYTES <= WAYS_POOL_BYTES && WAYS_POOL_FIRST_BYTES >= 2 * 64);

/// A square's ways' records' room in each of its reads at first, 16 KiB,
/// 240 blocks: a square with no ways reads 72 B. Every read's room doubles
/// past a square's records that outgrew it, that square filled again
/// (pxl-render gpu/renderer.rs `GroundBuffers::room`), up to a square's
/// most, `ways::tree::RECORDS_BYTES`: the GPU's read buffers and their
/// rooms on the heap together, `window::READS` of each.
pub const WAYS_RECORDS_FIRST_BYTES: u32 = 16 << 10;
