// pxl-geo-bake/deck/elevation.rs

//! The ground's height at points as the client draws it (the tile's elevation
//! with the client's interpolation and its cells' shift), so deck ends and bore
//! floors meet the drawn ground.

use std::collections::HashMap;

use pxl_spec::bake::constant::U_PER_CELL;
use pxl_spec::bake::ground::cell_shift;
use pxl_spec::bake::surface::bicubic;
use pxl_spec::space::adapter::epsg3857;
use pxl_spec::space::adapter::epsg3857::from_db;
use pxl_spec::space::pvf3::ivec::IVec;
use pxl_spec::space::pyramid::{GROUND, LEAF_Z, TILE_CELLS};
use pxl_spec::space::zoom::idx;

use crate::terrain::bake::Surface;

/// Heights at points from the tiles' surfaces, in cells, as the client's
/// `ground_ele_at` reads them; a point's tile must be held.
pub fn sample_cells(surfaces: &HashMap<IVec, Surface>, points: &[[f64; 2]]) -> Result<Vec<i32>, String> {
    points
        .iter()
        .map(|&[x, y]| {
            let tile = tile_of([x, y]);
            let surface = surfaces.get(&tile).ok_or_else(|| format!("tile {tile} is not held"))?;
            let [min, _] = epsg3857::get_tile_bbox(tile);
            let (tx, ty) = (from_db(x - min[0]) / U_PER_CELL, from_db(y - min[1]) / U_PER_CELL);
            let on_tile = (0.0..f64::from(TILE_CELLS)).contains(&tx) && (0.0..f64::from(TILE_CELLS)).contains(&ty);
            let shift = if on_tile {
                cell_shift(surface.ground.cells[GROUND.cell_index(tx as u32, ty as u32) as usize])
            } else {
                0
            };
            Ok(i32::from(bicubic(&surface.ele, tx, ty) + shift))
        })
        .collect()
}

pub fn tile_of([x, y]: [f64; 2]) -> IVec {
    let at = epsg3857::project(x, y, 0.0);
    idx(LEAF_Z, at.i)
}
