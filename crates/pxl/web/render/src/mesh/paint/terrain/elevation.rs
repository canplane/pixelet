// pxl-render/mesh/paint/terrain/elevation.rs

//! Coordinate prefix rules:
//! - c (chunk)  : chunk lattice index (e.g. cx, cy)
//! - d (delta)  : local loop delta (e.g. dx, dy)
//! - t (texel)  : continuous tile texel / pixel coordinates (e.g. tx, ty)
//! - s (source) : source-grid coordinates in the padded low-res array (e.g. sx, sy)
//!
//! Unit convention:
//! - [u]      : Canonical world unit (Standard coordinate space).
//! - [cell]   : Discrete raster unit. 1u = CELL.count_in(L1) cells (pxl-spec bake/constant.rs).
//! - [texel]  : Continuous coordinate within the PAGE_CELLS grid [0, PAGE_CELLS].
//! - [sample] : Low-res elevation grid index (before padding).
//!
//! Scale relation:
//! - 1 [cell] = 1 [texel]
//! - Elevation stored in the i16 lane is in [cell] units.

use pxl_spec::bake::format::{Ground, GroundChannelKey};
use pxl_spec::bake::ground::cell_shift;
use pxl_spec::bake::pack::{channel_of, GroundCell};
use pxl_spec::bake::surface::bicubic;
use pxl_spec::space::pyramid::GROUND;

/// `sample_ele`: the bicubic sample (pxl-spec bake/surface.rs `bicubic`).
/// `tx`/`ty` are signed because the padded raster reaches ELE_PAD samples
/// past the page's own span (pxl-spec's ELE_PAD): a cell just outside the
/// page has a height here, which is what lets a chunk at the page's edge
/// read the ground its neighbor stands on.
pub fn sample_ele(page_ele: &[i16], tx: i32, ty: i32) -> i16 {
    bicubic(page_ele, tx as f64 + 0.5, ty as f64 + 0.5)
}

/// One page's ground cells, however the caller holds them: a page's own
/// `Ground`, or the lane viewed out of the block it was read into. The
/// question below is asked of this and not of either shape, so there is one
/// answer and not one per caller.
#[derive(Clone, Copy)]
pub struct GroundLanes<'a> {
    pub cells: &'a [GroundCell],
}

impl<'a> GroundLanes<'a> {
    pub fn of(ground: &'a Ground) -> GroundLanes<'a> {
        GroundLanes { cells: &ground.cells }
    }

    /// The catalog number channel `key` holds at cell `i`.
    pub fn get(&self, key: GroundChannelKey, i: usize) -> u8 {
        channel_of(self.cells[i], key)
    }

    /// How far off its sampled height cell `i` stands, in units, up
    /// positive: the shift of the type it shows (pxl-spec's `shift`), so a
    /// shoreline reads as water meeting land a step lower, and a block's edge
    /// against its street, rather than as surfaces at one level. A real height and not a mesh offset — every consumer
    /// (the chunk mesh, and the kernel's query of the ground, which the pick
    /// and the camera's ground-snap ask) takes it through `ground_ele` — and
    /// so it is asked with no channel hidden: a structure covers the water
    /// whatever the dev toggle says, since the query reads it with the
    /// toggle nowhere in sight.
    pub fn shift(&self, i: usize) -> i16 {
        cell_shift(self.cells[i])
    }
}

/// The ground's height at cell (tx, ty) — the one answer every consumer of a
/// page's elevation takes, so that none of them reasons about a height the
/// others do not see. The sampled elevation plus the cell's shift, applied
/// here, once, rather than at each place that asks.
///
/// `ground` is none where the channels are not to hand, and the shift is
/// then not applied. That is a cell outside the page: the elevation raster
/// is padded and still has its height (pxl-spec's ELE_PAD), while the ground
/// channels are the page's own span and stop at its edge. The one place this
/// policy is not whole, and it is written here rather than left for each
/// caller to discover.
pub fn ground_ele(page_ele: &[i16], ground: Option<GroundLanes>, tx: i32, ty: i32) -> i16 {
    ground_ele_at(page_ele, ground, tx as f64 + 0.5, ty as f64 + 0.5)
}

/// `ground_ele` at a point in texels rather than a cell's center: where a
/// finer cell read from this page stands, its height the surface's there
/// and its shift the page cell's it lies in.
pub fn ground_ele_at(page_ele: &[i16], ground: Option<GroundLanes>, tx: f64, ty: f64) -> i16 {
    let ele = bicubic(page_ele, tx, ty);
    ground.map_or(ele, |g| ele + g.shift(GROUND.cell_index(tx.floor() as u32, ty.floor() as u32) as usize))
}

#[cfg(test)]
#[path = "elevation.test.rs"]
mod test;
