// pxl-engine/eye.rs

//! wgsl-port: the layout of the eye a cast is made from, as the words the core writes
//! (pxl-core domain/view/lens.rs `gpu_cast` and domain/scene/cast.rs `eye`) and every GPU
//! reader takes (pxl-render eye_bind.wgsl). Each constant is a word's place in that layout,
//! and this file is the only place the layout is written.

use crate::beam;
use crate::object::{Cut, CUT_WORDS};

/// The eye's position in Q3 steps, X, Y and Z.
pub const ORIGIN: usize = 0;
/// 1 where the frame shows the ray steps view: each ray's third word then holds the steps taken
/// on its way from the eye, not its asset ids (`ray`).
pub const STEPS: usize = 3;
/// The unit forward vector, X, Y and Z, then how far every ray goes (`FAR`), as f32 bits.
pub const FORWARD: usize = 4;
pub const FAR: usize = 7;
/// The unit right vector, then the unit up vector, as f32 bits, each followed by a word no
/// reader reads, so that the next starts a vec4.
pub const RIGHT: usize = 8;
pub const UP: usize = 12;
/// The tangents of the screen's half-angles across and up, as f32 bits.
pub const TAN: usize = 16;
/// The zooms a cell is asked at, coarsest then finest.
pub const RANGE: usize = 18;
/// A preview's cut to air (`CARVE`) and its left-out leaf (`HIDE`), each as X, Y, Z and the
/// zoom (`object::CUT_WORDS`), both as one run (`object::CUTS`); a zoom of `NO_CUT` means none
/// (`object::Cut`).
pub const CARVE: usize = 20;
pub const HIDE: usize = 24;
pub const NO_CUT: u32 = Cut::NONE.zoom;
/// The light's cast into the shadow map: its frame's constants, taken from the light's box
/// (`light::Light::words`). First come five vec4s of f32 bits: the start of texel (0, 0)'s
/// ray with every ray's reach, the step to the next texel across, the step down, the rays'
/// direction, and a hit's push in depth by its face's axis. The fourth words of the last four
/// hold the clipmap's corners (`light::CORNER`), the share of the box a receiver reads
/// (`light::REACH_NDC`) and a texel's width (`light::W_WIDTH`). Then come the reciprocals the
/// light's rules multiply by, each rounded once by the renderer: of the rays' direction and
/// of the reach, as a vec4, and of the squared steps across and down (`light::W_INVERSE` to
/// `light::W_DOWN2_INVERSE`). Last is the width squared over the split distance squared
/// (`light::W_BAND_SCALE`). The renderer writes these words as it aims the map (pxl-render
/// pass/shadow.rs `aim`); the core leaves them zero, and they stay zero where no shadow is
/// cast.
pub const LIGHT: usize = HIDE + CUT_WORDS;
pub const LIGHT_WORDS: usize = 27;
/// Where each pixel's ray passes through the pixel this frame: its offset from the pixel's
/// middle in drawn pixels, X right and Y down, as f32 bits. It starts at the first multiple of
/// four past the light's words and is followed by two unused words, because the eye is laid
/// out in fours. The renderer writes it; the core leaves it zero. The beams, cells and samples
/// are cast on the camera grid and read none of it.
pub const JITTER: usize = (LIGHT + LIGHT_WORDS).next_multiple_of(4);
pub(crate) const JITTER_WORDS: usize = 4;
/// The camera grid the frame casts on (`beam::Grid::words`), worked out once from the
/// tangents above when the core writes them. Every thread of the cast, and the renderer's
/// dispatches and buffers, read it here. It lies past the words a cast is made from, those
/// before `LIGHT`, because the frames that stand on a cast's samples keep that cast's grid,
/// while the next cast lays down this one (pxl-render ray/cast.rs `prepare`).
pub const GRID: usize = JITTER + JITTER_WORDS;
pub const GRID_WORDS: usize = beam::GRID_WORDS;
pub const WORDS: usize = GRID + GRID_WORDS;
const _: () = assert!(
    STEPS == ORIGIN + 3
        && FORWARD == STEPS + 1
        && FAR == FORWARD + 3
        && UP == RIGHT + 4
        && TAN == UP + 4
        && RANGE == TAN + 2
        && CARVE == RANGE + 2
        && HIDE == CARVE + CUT_WORDS
        && LIGHT.is_multiple_of(4)
        && JITTER.is_multiple_of(4)
        && GRID.is_multiple_of(4)
        && WORDS.is_multiple_of(4)
);

/// Whether a ball of `radius` units around `v`, in units from the eye, lies at all in the
/// eye's view. The eye looks along `forward`, with `right` across and `up`, and `tan` holds
/// the tangents of the screen's half-angles. The ball is in view where it is not behind the
/// eye and lies past none of the screen's sides by more than its radius. A point's distance
/// to a side is its offset divided by the side's slope, `√(1 + tan²)`, which the host rounds
/// once (`beam::Grid::slope`).
pub fn sees(
    [forward, right, up]: [[f32; 3]; 3],
    tan: [f32; 2],
    slope: [f32; 2],
    v: [f32; 3],
    radius: f32,
) -> bool {
    let dot = |a: [f32; 3], b: [f32; 3]| a[0] * b[0] + a[1] * b[1] + a[2] * b[2];
    let f = dot(v, forward);
    let side = |o: f32, t: f32, s: f32| libm::fabsf(o) - t * f <= radius * s;
    f >= -radius && side(dot(v, right), tan[0], slope[0]) && side(dot(v, up), tan[1], slope[1])
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A ball before the eye and within its view is seen; one past a side, or behind the eye, by
    /// more than its radius is not.
    #[test]
    fn eye_sees_view() {
        let axes = [[0.0, 1.0, 0.0], [1.0, 0.0, 0.0], [0.0, 0.0, 1.0]];
        let tan = [1.0, 0.5];
        let slope = beam::Grid::of(tan).slope();
        let sees = |v: [f32; 3], r: f32| sees(axes, tan, slope, v, r);
        assert!(sees([0.0, 10.0, 0.0], 0.0));
        assert!(sees([9.9, 10.0, 0.0], 0.0));
        assert!(!sees([12.0, 10.0, 0.0], 1.0));
        assert!(sees([12.0, 10.0, 0.0], 2.0));
        assert!(!sees([0.0, 10.0, 6.0], 0.5));
        assert!(!sees([0.0, -3.0, 0.0], 2.0));
        assert!(sees([0.0, -1.0, 0.0], 2.0));
    }
}
