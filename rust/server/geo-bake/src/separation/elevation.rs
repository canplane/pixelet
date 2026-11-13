// pxl-geo-bake/separation/elevation.rs

//! The ground's elevation at points: the elevation the client draws the cell a
//! point lies in, so bridge ends and tunnel floors meet the drawn surface. A
//! look's shift is no part of it, so a line stands on the elevation whatever
//! is painted under it.

use fnv::FnvHashMap;

use crate::terrain::CELL_U;
use pxl_spec::ground;
use pxl_spec::space::coord::adapter::epsg3857;
use pxl_spec::space::coord::adapter::epsg3857::u_from_merc;
use pxl_spec::space::coord::world::idx_of;
use pxl_spec::space::coord::zoom::LEAF_ZOOM;

/// Elevations at points from the tiles' elevations, in cells, as the client
/// draws the elevation: the elevation of the cell a point lies in (pxl-spec
/// ground.rs `ele_at`); a point's tile must be held.
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
                u_from_merc(x - min[0]) / CELL_U,
                u_from_merc(y - min[1]) / CELL_U,
            );
            let cell = [tx, ty].map(|v| v.floor() as i32);
            Ok(ground::ele_at(|i| i32::from(ele[i]), cell))
        })
        .collect()
}

pub fn tile_of([x, y]: [f64; 2]) -> [i32; 2] {
    let at = epsg3857::project(x, y, 0.0);
    let [x, y, _] = idx_of(LEAF_ZOOM, &at);
    [x, y]
}
