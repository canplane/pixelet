// pxl-engine/grid.rs

//! gpu-portable: the camera grid, the knob of what is observed: a cell is an
//! angle about the eye, and the zoom a node is drawn at, at a distance, is
//! the one at which it spans a cell there. What the descent asks at a
//! distance and how far the ground's clip levels reach both follow from it.

use crate::{Zoom, FINEST_ZOOM, UNIT_ZOOM};

// ---- the view's knobs: each set by itself, and nothing else here ----
// What is drawn and how finely follows from these, with the field of view
// (pxl-spec space/camera/grid.rs `HALF_TAN`) and the display's pixels.

/// The camera grid, the knob of what is observed: the view's cells a radian,
/// a cell 1/`CELLS_A_RAD` radians about the eye whatever the screen. An
/// angle, so what is observed is the display's pixels' to draw.
pub const CELLS_A_RAD: f32 = 96.0;

/// How many times finer than the view's a grid the bands and clip levels are
/// sized for (`CLIP_CELLS_A_RAD`, `clipmap::RADIUS`): a clip level holds its
/// zoom's cells out to `CLIP_MARGIN` times where the view stops drawing them.
pub const CLIP_MARGIN: f32 = 2.0;

// ---- the split's rule ----

/// The view's cells a zoom's cell spans where it begins to be drawn, going out:
/// its split distance is `CELLS_A_RAD / SPANS` of its sides (pxl-spec
/// space/camera/grid.rs `split_dist`).
pub const SPANS: f32 = 2.0;

// ---- derived from the knobs ----

/// The grid the bands and clip levels are sized for, cells a radian: the view's
/// `CLIP_MARGIN` times finer.
pub const CLIP_CELLS_A_RAD: f32 = CELLS_A_RAD * CLIP_MARGIN;

/// The view's cell, an angle in radians: the angle every eye's ray is asked
/// at (`cell_zoom_at`), and the samples' lattice, `param::beam::SAMPLE` a
/// cell a side, which a pixel takes the nearest stop of. The beam rays and
/// the cells their starts are found for lie on a lattice of their own, a
/// power of two of cells a side up to `LATTICE_MAX` (`beam::Grid::of`), the
/// drawn resolution's second knob (pxl-kernel
/// domain/view/frame/resolution.rs): the samples are the very rays at every
/// lattice, so what each screen observes and draws is the cell's alone. The
/// cell spaced the beam rays too until the lattice came apart from it
/// (doc/minutes/20261102-auto-judges-the-pixels-and-the-lattice.md).
pub const CELL: f32 = 1.0 / CELLS_A_RAD;

/// The coarsest lattice the beam rays and cells are laid on, in `CELL`s a
/// side, which lays a quarter of the cell's beam rays and cells. A lattice is a power of two of cells, so the
/// sample's spacing, the lattice's side over its samples a side, is the
/// very f32 at every one (`beam::Grid::sample`).
pub const LATTICE_MAX: u32 = 2;

/// One over the split distance squared, `CELLS_A_RAD / SPANS` sides: what a
/// distance squared is scaled by to read its zoom (`cell_zoom_at`).
pub const INV_SPLIT2: f32 = 1.0 / ((CELLS_A_RAD / SPANS) * (CELLS_A_RAD / SPANS));

/// The camera grid's cell zoom at `r2`, a distance from the eye squared, in
/// units, on a face met head-on (`cell_zoom_on`).
pub fn cell_zoom_at(r2: f32) -> Zoom {
    cell_zoom_on(r2 * INV_SPLIT2, 0)
}

/// The camera grid's cell zoom at `x2`, a distance squared in split
/// distances squared, on a face met head-on (`cell_zoom_on`), for a rule that
/// scales it by a word of its own (pxl-engine light.rs `Light::band`).
pub fn cell_zoom_in_splits(x2: f32) -> Zoom {
    cell_zoom_on(x2, 0)
}

/// The camera grid's cell zoom at `x2`, a distance from the eye squared in
/// split distances squared (`INV_SPLIT2`), on a face `slant` octaves
/// foreshortened (`slant`): the widest cell
/// whose face, as the eye sees it, spans the view's cells a cell at its split
/// distance (`CELLS_A_RAD / SPANS` of its sides) spans, held to what a zoom
/// can name. A face seen at the cosine `c` shows `c` of its side squared, so it
/// is asked as if `1 / c` as far squared: its distance squared doubled a time
/// an octave. The power of two is the float's own exponent, the octaves added
/// to it, halved, so neither a logarithm nor a square root rounds it: a
/// distance over the half split, `m·2^e` with `m` in [1, 2), squares to
/// `m²·2^2e` with `m²` in [1, 4), whose exponent is `2e` or `2e + 1`, so
/// halving it, rounding down, is `e`; and rounding the square never reaches 4,
/// since the widest `m` short of 2, `2 − 2⁻²³`, squares to `4 − 2⁻²¹` and a
/// bit, which rounds down to it.
///
/// The square is scaled by a product, not divided: a product rounds half to
/// even on every device, where a GPU's division need not round at all (Metal's
/// is a few ulps off). The split squared is 9 · 2⁸, and
/// 9 · fl(1/9) rounds to 1, so a square on a split distance, `2304 · 4^k`,
/// scales to `4^k` exactly and begins its zoom (pxl-spec
/// space/camera/grid.test.rs `zoom_cell_dist`). Zero, or under the least
/// normal float, it is the finest.
pub fn cell_zoom_on(x2: f32, slant: u32) -> Zoom {
    let field = (x2.to_bits() >> 23) & 0xff;
    let field = if field == 0 { 0 } else { field + slant };
    let e = (field as i32 - 127) >> 1;
    (UNIT_ZOOM as i32 - e).clamp(0, FINEST_ZOOM as i32) as Zoom
}

/// The whole octaves a face is foreshortened by where the ray meets it at the
/// cosine `c` to its normal, a ray's direction along the face's axis: the most
/// `k` with `2^k · |c| <= 1`, so a face asked by it (`cell_zoom_at`) is never
/// asked coarser than its cosine says, and a face met within 60° of head-on
/// is asked as one met head-on. Read off the float's bits, the same on every
/// device.
pub fn slant(c: f32) -> u32 {
    let bits = c.to_bits() & 0x7fff_ffff;
    // Its exponent rounded up: one over its floor where any mantissa bit is set.
    let up = ((bits >> 23) as i32 - 127) + (bits & 0x7f_ffff != 0) as i32;
    (-up).max(0) as u32
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A face within 60° of head-on is asked as one met head-on; past it, an
    /// octave a halving of its cosine, never coarser than the cosine says.
    #[test]
    fn slant_rounds_toward_the_finer() {
        assert_eq!(slant(1.0), 0);
        assert_eq!(slant(-1.0), 0);
        assert_eq!(slant(0.500_001), 0);
        assert_eq!(slant(0.5), 1);
        assert_eq!(slant(0.3), 1);
        assert_eq!(slant(0.25), 2);
        assert_eq!(slant(0.0), 127);
        assert_eq!(slant(f32::from_bits(1)), 126);
        for k in 0..1000 {
            let c = (k as f32 + 0.5) / 1000.0;
            let s = slant(c);
            assert!(
                (1u64 << s) as f32 * c <= 1.0 && (2u64 << s) as f32 * c > 1.0,
                "{c}"
            );
        }
    }

    /// A slant's octaves are its distance squared doubled as many times,
    /// exactly.
    #[test]
    fn slant_doubles_the_square() {
        for x2 in [1.0e-6f32, 0.3, 1.0, 4.0, 1.7e5] {
            for s in 0..8 {
                assert_eq!(
                    cell_zoom_on(x2, s),
                    cell_zoom_on(x2 * (1u32 << s) as f32, 0)
                );
            }
            assert_eq!(cell_zoom_on(x2, slant(0.0)), 0);
        }
        assert_eq!(cell_zoom_on(0.0, slant(0.0)), FINEST_ZOOM);
    }
}
