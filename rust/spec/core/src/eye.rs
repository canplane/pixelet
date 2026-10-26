// pxl-spec-core/eye.rs

//! gpu-portable: the eye a cast is from, as the words the kernel writes
//! (pxl-kernel domain/view/lens.rs `gpu_cast`, domain/scene/cast.rs `eye`) and every GPU
//! reader takes (pxl-shader `Eye`, pxl-render gpu/ray/cast.rs and
//! gpu/grown.rs, the screen pass). Each is a word's place among them, one
//! layout written here alone.

/// The eye's position in Q3 steps, X, Y and Z.
pub const ORIGIN: usize = 0;
/// Bits: each ray reports the nodes its descent asked, not what it met
/// (`SHOW_STEPS`); and from `ORACLE_SHIFT`, where each stop is walked again
/// from (pxl-shader cast.rs `oracle_cs`, `oracle`): not at all
/// (`ORACLE_OFF`), the world's root (`ORACLE_ROOT`), or, `ORACLE_CUT` and
/// past it, the box over the stop that the proxy's cut draws, `coarser`
/// (pxl-spec-core `proxy::cut`) one for each past `ORACLE_CUT`; with
/// `ORACLE_LOCATE`, that box only found, not walked from; with
/// `ORACLE_LOOSE`, walked from its front as it is, not from just short of
/// it in the node that holds the ray there; with `ORACLE_FROM_EYE`, the box
/// over the stop the eye's own walk makes, not the cast's.
pub const STEPS: usize = 3;
pub const SHOW_STEPS: u32 = 1;
pub const ORACLE_SHIFT: u32 = 8;
pub const ORACLE_OFF: u32 = 0;
pub const ORACLE_ROOT: u32 = 1;
pub const ORACLE_CUT: u32 = 2;
pub const ORACLE_LOCATE: u32 = 1 << 16;
pub const ORACLE_LOOSE: u32 = 1 << 17;
pub const ORACLE_FROM_EYE: u32 = 1 << 18;

/// Where the oracle starts, off the steps' word (`STEPS`).
pub const fn oracle(steps: u32) -> u32 {
    steps >> ORACLE_SHIFT & 0xff
}
/// The unit forward, X, Y and Z, then the reach, f32 bits.
pub const FORWARD: usize = 4;
pub const REACH: usize = 7;
/// The unit right, then τ, f32 bits.
pub const RIGHT: usize = 8;
pub const TAU: usize = 11;
/// The unit up, f32 bits, then a word no reader reads, so the next starts a
/// vec4.
pub const UP: usize = 12;
/// The tangents of the screen's half-angles across and up, f32 bits.
pub const TAN: usize = 16;
/// The zooms a block is asked at, coarsest then finest.
pub const RANGE: usize = 18;
/// A preview's cut to air and its left-out leaf, each X, Y, Z and the zoom,
/// the zoom `NO_CUT` for none (pxl-spec-core `overwrite::Cut`).
pub const CARVE: usize = 20;
pub const HIDE: usize = 24;
pub const CUT_WORDS: usize = 4;
/// Both cuts, carve then hide, as one run of words.
pub const CUTS: usize = 2 * CUT_WORDS;
pub const NO_CUT: u32 = crate::overwrite::Cut::NONE.zoom;
/// The camera grid the frame casts on (pxl-spec-core `beam::Grid`, its
/// `words`), worked out once off the tangents and τ above where the kernel
/// writes them: every thread of the cast and the renderer's dispatches and
/// buffer read it here, none works it out again.
pub const GRID: usize = HIDE + CUT_WORDS;
pub const GRID_WORDS: usize = crate::beam::GRID_WORDS;
/// The sun's cast into the shadow map, its frame's constants off the light
/// box (pxl-spec-core `sun::Sun`, its `words`): texel (0, 0)'s ray's start
/// and every ray's reach, the step to the next texel across and down, the
/// rays' direction, and a stop's push in depth by its face's axis, a vec4
/// each, f32 bits, the last four's fourth words the clipmap's corners
/// (`sun::CORNER`, `sun::KEPT`). All zero where no shadow is cast. The renderer writes
/// them, as it aims the map (pxl-render gpu/shadow.rs `aim`); the kernel
/// leaves them zero.
pub const SUN: usize = GRID + GRID_WORDS;
pub const SUN_WORDS: usize = 20;
/// Every word, the sun's light box last.
pub const WORDS: usize = SUN + SUN_WORDS;
const _: () = assert!(
    STEPS == ORIGIN + 3
        && REACH == FORWARD + 3
        && TAU == RIGHT + 3
        && TAN == UP + 4
        && RANGE == TAN + 2
        && CARVE == RANGE + 2
        && HIDE == CARVE + CUT_WORDS
        && GRID.is_multiple_of(4)
        && SUN.is_multiple_of(4)
);

/// The slope a caster's depth in the sun's map is pushed back by, as a
/// rasterizer's polygon offset: its greatest slope, in depth a texel
/// (`sun::slope`), times this. The raster casters' depth pipelines take it
/// (pxl-render record.rs `depth_only`), and the world's cast adds it
/// (pxl-shader `sun_cs`), so the two kinds of caster shadow alike. A
/// receiver compares each tap at its own plane's depth there (pxl-render
/// shadow.wgsl), so the push covers only the texels a tap filters, a texel
/// each way, along the steeper axis; the receiver's texel of normal offset
/// covers the other. At 2, with each tap compared at the receiver's own
/// depth, a step a unit high shadowed the ground below it in stripes a
/// texel apart: the taps up the slope saw past the step.
pub const SLOPE_BIAS: f32 = 1.0;
