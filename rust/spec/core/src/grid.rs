// pxl-spec-core/grid.rs

//! gpu-portable: the camera grid, the one knob of resolution
//! (doc/minutes/20261013-a-ray-asks-the-world-whether-a-node-is-empty.md,
//! "The descent"): a cell is an angle about the eye, and the zoom a node is
//! drawn at, at a distance, is the one at which it spans a cell there. What the
//! descent asks at a distance and how far the ground's windows reach both
//! follow from it.

use crate::{Zoom, FINEST_ZOOM, UNIT_ZOOM};

/// Drawn pixels per CSS pixel at most: the finest ratio a surface is drawn at,
/// and where more pixels stop being seen. The CSS reference pixel subtends
/// 0.0213° (CSS Values and Units: 96 dpi at 28 inches), 46.9 a degree, and
/// the fovea resolves 94 a degree at most (Ashraf, Chapiro & Mantiuk,
/// "Resolution limit of the eye: how many pixels can we see?", Nature
/// Communications, 2025, doi:10.1038/s41467-025-64679-2): 94 / 46.9 is 2.
pub const RATIO_MAX: f32 = 2.0;

/// The camera grid, the one knob of resolution: a cell is an angle of
/// 1/`SPLIT` radians about the eye (pxl-spec space/camera/grid.rs), stated
/// at `RATIO_MAX`: 96 a radian a CSS pixel asks, in drawn pixels. Every
/// view is cast at τ `RATIO_MAX` whatever its ratio (pxl-spec
/// space/camera/grid.rs `Tau::VIEW`), its cells as wide as a ratio of one
/// asks, so what is observed is the display's pixels' to draw, never theirs
/// to decide; whatever is sized by `SPLIT` holds at every τ.
pub const SPLIT: f32 = 96.0 * RATIO_MAX;

/// The camera grid's cell zoom at `r2`, a distance from the eye squared,
/// in units: the widest cell whose split distance (`SPLIT` of its sides,
/// halved) is within the distance, held to what a zoom can name. The power
/// of two is the float's own exponent, halved, so neither a logarithm nor a
/// square root rounds it: a distance over the half split, `m·2^e` with `m`
/// in [1, 2), squares to `m²·2^2e` with `m²` in [1, 4), whose exponent is
/// `2e` or `2e + 1`, so halving it, rounding down, is `e`; and rounding the
/// square never reaches 4, since the widest `m` short of 2, `2 − 2⁻²³`,
/// squares to `4 − 2⁻²¹` and a bit, which rounds down to it. Every asker
/// passes a square (pxl-spec space/camera/grid.rs, `scatter::drawn_at`), so
/// a distance on a power of two falls the same side everywhere.
pub fn cell_at(r2: f32) -> Zoom {
    let x2 = r2 / (SPLIT * SPLIT / 4.0);
    let e = (((x2.to_bits() >> 23) & 0xff) as i32 - 127) >> 1;
    (UNIT_ZOOM as i32 - e).clamp(0, FINEST_ZOOM as i32) as Zoom
}
