// pxl-core/domain/scene/param.rs

//! The scene's tuning: how the world is lit, how finely it is drawn, and the
//! first size of the ways' pool on the GPU.

// ---- lighting ----

/// How much the Fresnel rim highlight brightens a face at its edge, as a
/// share of the face's own light, on the shader's linear falloff. One shader
/// lights everything, so the rim applies to everything lit. The value was set
/// by eye. It is near the 0.47 the day's terms give: a rim of 0.7 of the sky's
/// light, where the sky is a fifth of a day's light, on a face of albedo 0.3.
pub const RIM_INTENSITY: f64 = 0.5;

/// How much edge shading darkens a cube's creases, convex and concave.
pub const EDGE_STRENGTH: f64 = 0.3;

/// How much corner occlusion darkens at its deepest. The stage hands it only
/// when the setting asks for occlusion, because the shader cannot read a
/// setting.
pub const AO_STRENGTH: f64 = 0.2;

// ---- the drawn resolution ----
// These tune resolution.rs. A `RES_` constant serves both of `auto`'s knobs,
// the rate and the scale; `RES_MIN` and `RES_STEPS` set the scale's steps. A
// `SCALE_` constant serves only the judging done where the device reports no
// GPU time, when the scale alone moves.

/// The least share of the display's pixels on each axis that the world is
/// drawn at.
pub const RES_MIN: f64 = 0.5;

/// The number of steps from whole down to `RES_MIN`. Each step draws the same
/// fraction of the step before, so two steps halve the pixel count.
pub const RES_STEPS: i32 = 4;

/// How long a judged window lasts, as the sum of its frames' budgets in ms.
/// A step down waits for the end of a window.
pub const RES_WINDOW_MS: f64 = 500.0;

/// Where the device reports GPU time, the windows in a row in which the step
/// above, of either knob, must fit before it is taken.
pub const RES_UP_WINDOWS: u32 = 4;

/// A window whose load is within this share of its budget, above or below,
/// keeps the step it is at.
pub const RES_BAND: f64 = 0.1;

/// The tightest frame budget, one frame at 60 Hz.
pub const RES_BUDGET_FLOOR_MS: f64 = 1000.0 / 60.0;

/// The windows after a change that are not judged, a second of them. A change
/// rebuilds the render targets or casts the lattice again in full, so the
/// frames just after it do not show the load at the new step.
pub const RES_SETTLE_WINDOWS: u32 = 2;

/// Where the device reports no GPU time, a window whose frames' gaps average
/// more than this many budgets has missed frames.
pub const SCALE_MISSED: f64 = 1.5;

/// Where the device reports no GPU time, the quiet windows before the scale
/// tries a step up: 5 s.
pub const SCALE_PROBE_WINDOWS: u32 = 10;

/// The windows after a step up, 4 s, within which going over takes the step
/// back and doubles the wait before the next. On a Galaxy S23, a step up whose
/// prediction missed by a few percent went over within 3 s.
pub const RES_TRUST_WINDOWS: u32 = 8;

/// The longest the doublings make the wait for a step up: 60 s.
pub const RES_UP_WINDOWS_MAX: u32 = 120;

/// The long side, in pixels, of a character surface's buffer, which is drawn
/// at thumbnail size.
pub const CHARACTER_LONG_SIDE_MAX: f64 = 256.0;

// ---- the ways, as the stage hands them to the GPU ----

/// The first size of the ways' pool on the GPU: 1 MiB, or 16 Ki child groups.
/// A brick without ways takes no child group, and a brick with ways takes
/// child groups starting with one for its own nodes along z (pxl-engine
/// `way::tree`), so this holds the ways of a sparse view. The pool doubles
/// whenever the core's pool outgrows it (pxl-render gpu/ray/ground.rs
/// `GroundBuffer::grow`), so a dense view reaches 64 MiB in six doublings.
pub const WAYS_POOL_FIRST_BYTES: u32 = 1 << 20;
const _: () = {
    use pxl_engine::way::tree::{CHILD_GROUP_WORDS, POOL_CHILD_GROUPS};
    assert!(
        WAYS_POOL_FIRST_BYTES <= POOL_CHILD_GROUPS * CHILD_GROUP_WORDS * 4
            && WAYS_POOL_FIRST_BYTES >= 2 * CHILD_GROUP_WORDS * 4,
        "the pool's first is no more than its most, nor less than a child group past child group 0"
    );
};
