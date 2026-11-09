// pxl-spec/space/camera/grid.rs

//! wgsl-twin: the camera grid, the one resolution, which sets how finely the world is
//! observed. A camera cell is a fixed angle about the eye, and a node at a given distance is
//! drawn at the zoom at which it spans a cell there. The node a cell is drawn with, drawn
//! whole, spans at least one cell from any direction; one cell of the grid is one thing drawn,
//! and what lies inside it is for its layer to fold. What the descent asks at a distance, and
//! how far the ground's clip levels reach, both follow from it.

use crate::space::coord::zoom::{units, Zoom, FINEST_ZOOM, UNIT_ZOOM};

// Each knob here is set on its own. What is drawn, and how finely, follows from them, from
// the field of view and from the display's pixels.

/// The camera grid's cells a radian: a cell is 1/`CELLS_A_RAD` radians about the eye,
/// whatever the screen. The grid is an angle, not a count of pixels, so what is observed does
/// not depend on the display; the display's pixels only draw it.
pub const CELLS_A_RAD: f32 = 96.0;

/// The tangent of half the vertical field of view (pxl-core domain/view/frame/pose.rs
/// `ray_of`): the view's height spans one unit at unit distance, a field of 53.13 degrees.
pub const HALF_TAN: f32 = 0.5;

/// The widest view drawn, as its width over its height: an ultrawide display's 21:9. Whatever is
/// sized by the view's cells (pxl-engine `node_cache::ENTRIES`) holds them at this aspect or
/// narrower.
pub const ASPECT_MOST: f32 = 21.0 / 9.0;

/// How many times finer than the view's grid is the grid that the bands and the clip levels are
/// sized for (`CLIP_CELLS_A_RAD`, pxl-engine `clipmap::RADIUS`). A clip level holds its zoom's
/// cells out to `CLIP_MARGIN` times the distance where the view stops drawing them.
pub const CLIP_MARGIN: f32 = 2.0;

/// How many of the view's cells a zoom's cell spans at the distance where, going outward, it
/// begins to be drawn. Its split distance is therefore `CELLS_A_RAD / SPANS` times its side
/// (`split_dist`).
pub const SPANS: f32 = 2.0;

// What follows is derived from the knobs above and is not set on its own.

pub const CLIP_CELLS_A_RAD: f32 = CELLS_A_RAD * CLIP_MARGIN;

/// The view's cell, an angle in radians. Every eye ray is asked at this angle (`cell_zoom_at`), the
/// beam rays and the cells are spaced by it (pxl-engine `beam::Grid::of`), and the samples lie on a
/// lattice of pxl-engine `param::beam::SAMPLE` a cell a side, of which a pixel takes the nearest
/// stop.
pub const CELL: f32 = 1.0 / CELLS_A_RAD;
/// The reciprocal of `CELL`, by which a tangent is multiplied to count cells. The CPU and the
/// GPU use this one constant, so both count alike.
pub const CELL_INVERSE: f32 = 1.0 / CELL;

/// The factor that turns a squared distance into squared split distances (`cell_zoom_at`).
pub const INV_SPLIT2: f32 = 1.0 / ((CELLS_A_RAD / SPANS) * (CELLS_A_RAD / SPANS));

/// The camera grid's cell zoom at `r2`, a squared distance from the eye in units: the widest
/// cell whose split distance (`CELLS_A_RAD / SPANS` times its side) lies within the distance,
/// held to the zooms that exist. The power of two is the float's own exponent, halved, so no
/// logarithm or square root rounds it. A distance past half a split, `m·2^e` with `m` in
/// [1, 2), squares to `m²·2^2e` with `m²` in [1, 4), whose exponent is `2e` or `2e + 1`;
/// halved and rounded down, that is `e`. Rounding the square never reaches 4: the widest `m`
/// short of 2, `2 − 2⁻²³`, squares to a little over `4 − 2⁻²¹`, which rounds down to it.
///
/// The square is multiplied by a constant, not divided: a product rounds half to even on
/// every device, while a GPU's division need not be correctly rounded (Metal's is a few ulps
/// off). The split squared is 9 · 2⁸, and 9 · fl(1/9) rounds to 1, so the square of a split
/// distance, `2304 · 4^k`, scales to exactly `4^k` and begins its zoom (grid.test.rs
/// `zoom_cell_dist`).
pub fn cell_zoom_at(r2: f32) -> Zoom {
    cell_zoom_in_splits(r2 * INV_SPLIT2)
}

/// The cell zoom at `x2`, a squared distance already counted in squared split distances, for
/// a rule that scales the distance by a factor of its own (pxl-engine light.rs `Light::band`).
pub fn cell_zoom_in_splits(x2: f32) -> Zoom {
    let e = (((x2.to_bits() >> 23) & 0xff) as i32 - 127) >> 1;
    (UNIT_ZOOM as i32 - e).clamp(0, FINEST_ZOOM as i32) as Zoom
}

/// The vertical field of view, in radians (`HALF_TAN`).
pub fn fov() -> f64 {
    2.0 * libm::atan(f64::from(HALF_TAN))
}

/// Where, going outward, ground starts being drawn with cells at `cell_zoom`: where a cell's
/// side spans `SPANS` of the view's cells. A clip level holds those cells out to
/// `CLIP_MARGIN` times as far.
pub const fn split_dist(cell_zoom: Zoom) -> f64 {
    CELLS_A_RAD as f64 * units(cell_zoom) / SPANS as f64
}

#[cfg(test)]
#[path = "grid.test.rs"]
mod test;
