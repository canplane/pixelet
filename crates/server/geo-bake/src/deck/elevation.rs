// pxl-geo-bake/deck/elevation.rs

//! The ground's height at points: the tile's elevation with the client's
//! interpolation, so deck ends and bore floors meet the drawn surface. A
//! look's shift is no part of it, so a deck stands on the elevation whatever
//! is painted under it.

use std::collections::HashMap;

use crate::terrain::CELL_U;
use pxl_spec::bake::surface::bicubic;
use pxl_spec::space::coord::adapter::epsg3857;
use pxl_spec::space::coord::adapter::epsg3857::u_from_mer;
use pxl_spec::space::coord::pvf3::ivec::IVec;
use pxl_spec::space::coord::zoom::idx;
use pxl_spec::space::layer::tile::LEAF_ZOOM;

/// Heights at points from the tiles' elevations, in cells, as the client's
/// `ground_ele_at` reads the elevation; a point's tile must be held.
pub fn sample_cells(elevations: &HashMap<IVec, Vec<i16>>, points: &[[f64; 2]]) -> Result<Vec<i32>, String> {
    points
        .iter()
        .map(|&[x, y]| {
            let tile = tile_of([x, y]);
            let ele = elevations.get(&tile).ok_or_else(|| format!("tile {tile} is not held"))?;
            let [min, _] = epsg3857::get_tile_bbox(LEAF_ZOOM, tile);
            let (tx, ty) = (u_from_mer(x - min[0]) / CELL_U, u_from_mer(y - min[1]) / CELL_U);
            Ok(i32::from(bicubic(ele, tx, ty)))
        })
        .collect()
}

pub fn tile_of([x, y]: [f64; 2]) -> IVec {
    let at = epsg3857::project(x, y, 0.0);
    idx(LEAF_ZOOM, at.i)
}
