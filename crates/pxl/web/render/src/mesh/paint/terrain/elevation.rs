// pxl-render/mesh/paint/terrain/elevation.rs

//! Page elevation in cells (1 cell = 1 texel). Prefixes: c chunk index,
//! d loop delta, t texel, s padded source sample.

use pxl_spec::bake::format::{Ground, GroundChannelKey};
use pxl_spec::bake::ground::cell_shift;
use pxl_spec::bake::pack::{channel_of, GroundCell};
use pxl_spec::bake::surface::bicubic;
use pxl_spec::space::pyramid::GROUND;

/// Signed because the padded raster reaches ELE_PAD samples past the page, so a
/// chunk at the edge can read its neighbor's ground.
pub fn sample_ele(page_ele: &[i16], tx: i32, ty: i32) -> i16 {
    bicubic(page_ele, tx as f64 + 0.5, ty as f64 + 0.5)
}

/// One answer for every way a caller holds a page's ground cells.
#[derive(Clone, Copy)]
pub struct GroundLanes<'a> {
    pub cells: &'a [GroundCell],
}

impl<'a> GroundLanes<'a> {
    pub fn of(ground: &'a Ground) -> GroundLanes<'a> {
        GroundLanes { cells: &ground.cells }
    }

    pub fn get(&self, key: GroundChannelKey, i: usize) -> u8 {
        channel_of(self.cells[i], key)
    }

    /// The type's shift in units, so a shoreline steps down to the water. A real
    /// height every consumer takes through `ground_ele`, so it is asked with no
    /// channel hidden.
    pub fn shift(&self, i: usize) -> i16 {
        cell_shift(self.cells[i])
    }
}

/// The one ground height every consumer takes: sampled elevation plus the
/// cell's shift. `ground` is none outside the page, where the padded raster
/// still has a height but the channels stop, and the shift is not applied.
pub fn ground_ele(page_ele: &[i16], ground: Option<GroundLanes>, tx: i32, ty: i32) -> i16 {
    ground_ele_at(page_ele, ground, tx as f64 + 0.5, ty as f64 + 0.5)
}

/// At a point in texels, with the shift of the page cell it lies in.
pub fn ground_ele_at(page_ele: &[i16], ground: Option<GroundLanes>, tx: f64, ty: f64) -> i16 {
    let ele = bicubic(page_ele, tx, ty);
    ground.map_or(ele, |g| ele + g.shift(GROUND.cell_index(tx.floor() as u32, ty.floor() as u32) as usize))
}

#[cfg(test)]
#[path = "elevation.test.rs"]
mod test;
