// pxl-spec-core/grid.rs

//! gpu-portable: the camera grid, the one knob of resolution
//! (doc/minutes/20261013-a-ray-asks-the-world-whether-a-node-is-empty.md,
//! "The descent"): a cell is an angle about the eye, and the zoom a node is
//! drawn at, at a distance, is the one at which it spans a cell there. What the
//! descent asks at a distance and how far the ground's windows reach both
//! follow from it.

use crate::{Zoom, FINEST_ZOOM, UNIT_ZOOM};

// ---- the view's knobs: each set by itself, and nothing else here ----
// What is drawn and how finely follows from these, with the field of view
// and the reference view's CSS pixels (pxl-spec space/camera/grid.rs
// `FOV_DEG`, `REFERENCE_VIEWPORT_H`, `RATIO_MAX`).

/// The camera grid, the one knob of resolution: the view's cells a radian,
/// a cell 1/`CELLS_A_RADIAN` radians about the eye whatever the screen
/// (pxl-spec space/camera/grid.rs `cell_css_px`: about twelve CSS pixels at
/// the reference view). An angle, so what is observed is the display's
/// pixels' to draw, never theirs to decide.
pub const CELLS_A_RADIAN: f32 = 96.0;

/// How many times finer than the view's a grid the bands and windows are
/// sized for (`SPLIT`, `window::RADIUS`): a window holds its zoom's cells
/// out to `WINDOW_MARGIN` times where the view stops drawing them, so the
/// view casts at τ `WINDOW_MARGIN` and the finest a view may ask, τ 1
/// (pxl-spec space/camera/grid.rs `Tau::FINEST`), is held too. Whether a
/// margin of 1 keeps up with a moving eye is unmeasured.
pub const WINDOW_MARGIN: f32 = 2.0;

// ---- derived from the knobs ----

/// The grid the bands and windows are sized for, cells a radian: the view's
/// `WINDOW_MARGIN` times finer. A cell of the view is 1/`SPLIT` radians
/// scaled by τ (pxl-spec space/camera/grid.rs `Tau::VIEW`, which is
/// `WINDOW_MARGIN`), so whatever is sized by `SPLIT` holds at every τ.
pub const SPLIT: f32 = CELLS_A_RADIAN * WINDOW_MARGIN;

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
