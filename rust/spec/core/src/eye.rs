// pxl-spec-core/eye.rs

//! gpu-portable: the eye a cast is from, as the words the kernel writes
//! (pxl-kernel domain/view/lens.rs `gpu_cast`, domain/scene/cast.rs `eye`) and every GPU
//! reader takes (pxl-shader `Eye`, pxl-render gpu/ray/cast.rs and
//! gpu/grown.rs, the screen pass). Each is a word's place among them, one
//! layout written here alone.

/// The eye's position in Q3 steps, X, Y and Z.
pub const ORIGIN: usize = 0;
/// Whether each ray reports the nodes its descent asked, not what it met.
pub const STEPS: usize = 3;
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
/// The zooms a cell is asked at, coarsest then finest.
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

/// Whether a ball `radius` units wide about `v`, units from the eye, lies
/// at all in the view of the eye looking along `forward`, `right` across
/// and `up`, the screen's half-angles' tangents `tan`: before it, and past
/// none of its sides by more than its radius, each side's distance to a
/// point its offset over the side's slope (`√(1 + tan²)`).
pub fn sees(
    [forward, right, up]: [[f32; 3]; 3],
    tan: [f32; 2],
    v: [f32; 3],
    radius: f32,
) -> bool {
    let dot = |a: [f32; 3], b: [f32; 3]| a[0] * b[0] + a[1] * b[1] + a[2] * b[2];
    let f = dot(v, forward);
    let side = |o: f32, t: f32| libm::fabsf(o) - t * f <= radius * libm::sqrtf(1.0 + t * t);
    f >= -radius && side(dot(v, right), tan[0]) && side(dot(v, up), tan[1])
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A ball before the eye within its view is seen, one past a side or
    /// behind it by more than its radius not.
    #[test]
    fn the_eye_sees_what_lies_within_its_view() {
        let axes = [[0.0, 1.0, 0.0], [1.0, 0.0, 0.0], [0.0, 0.0, 1.0]];
        let tan = [1.0, 0.5];
        assert!(sees(axes, tan, [0.0, 10.0, 0.0], 0.0));
        assert!(sees(axes, tan, [9.9, 10.0, 0.0], 0.0));
        assert!(!sees(axes, tan, [12.0, 10.0, 0.0], 1.0));
        assert!(sees(axes, tan, [12.0, 10.0, 0.0], 2.0));
        assert!(!sees(axes, tan, [0.0, 10.0, 6.0], 0.5));
        assert!(!sees(axes, tan, [0.0, -3.0, 0.0], 2.0));
        assert!(sees(axes, tan, [0.0, -1.0, 0.0], 2.0));
    }
}
