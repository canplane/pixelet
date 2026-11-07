// pxl-kernel/domain/scene/param.rs

//! What is drawn, how finely and how it is lit, and what the stage hands
//! the GPU of the ways: the pool's first size, the records' first room and
//! the scratch, each growing to the bake's bound (pxl-engine
//! way/tree.rs `POOL_CHILD_GROUPS`, `BRICK_CHILD_GROUPS`).

// ---- lighting ----

/// Fresnel rim highlight, applied to everything lit since one shader lights
/// it all, on the shader's linear falloff: how much a rim brightens a face at
/// its edge, as a share of its own light: set at 0.5 by eye, near what the
/// day's terms give, about 0.47, a rim of 0.7 of the sky's light, the sky a
/// fifth of a day's light on a face of albedo 0.3.
pub const RIM_INTENSITY: f64 = 0.5;

/// Edge shading along a cube's creases, convex and concave: its darkening,
/// and its width in units, a 64th of one, about 3 cm at Seoul's latitude:
/// one line alike on cells of every size, thinned by perspective alone.
pub const EDGE_STRENGTH: f64 = 0.3;
pub const EDGE_WIDTH: f64 = 1.0 / 64.0;

/// Corner occlusion's darkening at its deepest, written where the setting
/// asks for it; the shader cannot see a setting.
pub const AO_STRENGTH: f64 = 0.2;

// ---- the drawn resolution ----
// `auto`'s two knobs, the rate and the scale, share the windows, the band and
// the budget, `RES_` (resolution.rs), and the scale's steps are `RES_STEPS`; the
// misses and probes judged where no GPU time is reported are the scale's alone,
// `SCALE_`.

/// The least share of the display's pixels, each way, the world is drawn at.
pub const RES_MIN: f64 = 0.5;

/// The steps from whole down to `RES_MIN`, each a like share of the last:
/// two of them halve the pixels.
pub const RES_STEPS: i32 = 4;

/// The budgets a window of the load sums, in ms: what a step down waits
/// for.
pub const RES_WINDOW_MS: f64 = 500.0;

/// The windows in a row the step above, of either knob, must fit in before
/// it is taken, where the device reports GPU time.
pub const RES_UP_WINDOWS: u32 = 4;

/// A window within this of its budget, either way, holds the step.
pub const RES_BAND: f64 = 0.1;

/// The tightest budget: 60 Hz.
pub const RES_BUDGET_FLOOR_MS: f64 = 1000.0 / 60.0;

/// A change rebuilds the render targets, or the lattice's casts in full, so
/// the windows after one of either, a second of them, are not judged.
pub const RES_SETTLE_WINDOWS: u32 = 2;

/// Where the device reports no GPU time, a window whose frames' gap averages
/// past this many budgets missed frames.
pub const SCALE_MISSED: f64 = 1.5;

/// Where the device reports no GPU time, the quiet windows before a probe
/// up, 5 s.
pub const SCALE_PROBE_WINDOWS: u32 = 10;

/// The windows after a step up, 4 s, within which going over takes it back
/// and doubles the wait before the next; the S23's fell within 3 s where the
/// prediction missed by a few percent.
pub const RES_TRUST_WINDOWS: u32 = 8;

/// The most those doublings make the wait, 60 s.
pub const RES_UP_WINDOWS_MAX: u32 = 120;

/// A character surface is drawn at thumbnail size: its buffer's long side, in pixels.
pub const CHARACTER_LONG_SIDE_MAX: f64 = 256.0;

// ---- the ways, as the stage hands them to the GPU ----

/// The ways' pool on the GPU at first, 1 MiB, 16 Ki child groups: a brick with
/// no ways holds none, and one with ways a child group from its own nodes along
/// z down (pxl-engine `way::tree`), so the ways of a sparse view. It doubles
/// as the kernel's pool passes it (pxl-render gpu/ray/ground.rs
/// `GroundBuffers::grow`), so a dense view reaches 64 MiB in six.
pub const WAYS_POOL_FIRST_BYTES: u32 = 1 << 20;
const _: () = {
    use pxl_engine::way::tree::{CHILD_GROUP_WORDS, POOL_CHILD_GROUPS};
    assert!(
        WAYS_POOL_FIRST_BYTES <= POOL_CHILD_GROUPS * CHILD_GROUP_WORDS * 4
            && WAYS_POOL_FIRST_BYTES >= 2 * CHILD_GROUP_WORDS * 4,
        "the pool's first is no more than its most, nor less than a child group past child group 0"
    );
};
