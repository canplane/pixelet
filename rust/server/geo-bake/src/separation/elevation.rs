// pxl-geo-bake/separation/elevation.rs

//! The ground's height at points: the tile's elevation with the client's
//! interpolation, so bridge ends and tunnel floors meet the drawn surface. A
//! look's shift is no part of it, so a line stands on the elevation whatever
//! is painted under it.

use fnv::FnvHashMap;

use crate::terrain::CELL_U;
use pxl_spec::space::coord::adapter::epsg3857;
use pxl_spec::space::coord::adapter::epsg3857::u_from_mer;
use pxl_spec::space::coord::zoom::index_of;
use pxl_spec_core::param::tile::LEAF_ZOOM;
use pxl_spec_core::ground::bicubic;

/// Heights at points from the tiles' elevations, in cells, as the client's
/// `ground_ele_at` reads the elevation; a point's tile must be held.
pub fn sample_cells(
    eles: &FnvHashMap<[i32; 2], Vec<i16>>,
    points: &[[f64; 2]],
) -> Result<Vec<i32>, String> {
    points
        .iter()
        .map(|&[x, y]| {
            let tile = tile_of([x, y]);
            let ele = eles
                .get(&tile)
                .ok_or_else(|| format!("tile {tile:?} is not held"))?;
            let [min, _] = epsg3857::get_tile_bbox(LEAF_ZOOM, tile);
            let (tx, ty) = (
                u_from_mer(x - min[0]) / CELL_U,
                u_from_mer(y - min[1]) / CELL_U,
            );
            Ok(bicubic(|i| i32::from(ele[i]), tx as f32, ty as f32))
        })
        .collect()
}

pub fn tile_of([x, y]: [f64; 2]) -> [i32; 2] {
    let at = epsg3857::project(x, y, 0.0);
    let [x, y, _] = index_of(LEAF_ZOOM, &at);
    [x, y]
}
