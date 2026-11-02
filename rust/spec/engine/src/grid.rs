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

/// The camera grid's cell zoom at `r2`, a distance from the eye squared, in
/// units: the widest cell whose split distance (`CELLS_A_RAD / SPANS` of its
/// sides) is within the distance, held to what a zoom can name. The power of
/// two is the float's own exponent, halved, so neither a logarithm nor a square
/// root rounds it: a distance over the half split, `m·2^e` with `m` in [1, 2),
/// squares to `m²·2^2e` with `m²` in [1, 4), whose exponent is `2e` or `2e +
/// 1`, so halving it, rounding down, is `e`; and rounding the square never
/// reaches 4, since the widest `m` short of 2, `2 − 2⁻²³`, squares to `4 −
/// 2⁻²¹` and a bit, which rounds down to it. Every asker passes a square
/// (pxl-spec space/camera/grid.rs, `generation::drawn_at`), so a distance on a
/// power of two falls the same side everywhere.
pub fn cell_zoom_at(r2: f32) -> Zoom {
    let split = CELLS_A_RAD / SPANS;
    let x2 = r2 / (split * split);
    let e = (((x2.to_bits() >> 23) & 0xff) as i32 - 127) >> 1;
    (UNIT_ZOOM as i32 - e).clamp(0, FINEST_ZOOM as i32) as Zoom
}
