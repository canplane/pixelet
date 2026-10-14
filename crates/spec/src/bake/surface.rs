// pxl-spec/bake/surface.rs

//! The surface a leaf's elevation draws, once, at its cells: what every
//! zoom's heights are folded from (bake/fold.rs), so no reader of a tile
//! interpolates.

use pxl_portable::ground::height;

use super::constant::ELE_PADDED_SPAN;
use crate::space::layer::lattice::TILE_CELLS;

/// Each cell's height off a leaf's padded elevation samples
/// (pxl-portable `ground::height`), row by row.
pub fn heights(ele: &[i16]) -> Vec<i16> {
    xpute_kit::ensure!(ele.len() == (ELE_PADDED_SPAN * ELE_PADDED_SPAN) as usize, ENOTRECOVERABLE);
    (0..TILE_CELLS * TILE_CELLS).map(|i| height(|k| i32::from(ele[k]), [i % TILE_CELLS, i / TILE_CELLS]) as i16).collect()
}
