// pxl-geo-bake/deck/elevation.rs

//! The ground's height at points as the client draws it (the page grids with
//! the client's interpolation), so deck ends and bore floors meet the drawn ground.

use std::collections::HashMap;

use pxl_spec::bake::constant::{ELE_PADDED_SPAN, U_PER_CELL};
use pxl_spec::space::adapter::epsg3857::{from_db, u_per_m};
use pxl_spec::bake::surface::bicubic;
use pxl_spec::space::adapter::epsg3857;
use pxl_spec::space::indexer::L3_IDX;
use pxl_spec::space::pvf3::ivec::IVec;
use pxl_spec::space::pvf3::pvf3::Pvf3;


/// Heights at points from page grids, as the client interpolates them; a
/// point's page is `page_of`'s, a page with no grid flat at sea level.
pub fn sample(grids: &HashMap<IVec, Vec<i16>>, points: &[[f64; 2]], page_of: impl Fn([f64; 2]) -> IVec) -> Vec<f64> {
    let span = ELE_PADDED_SPAN as usize;
    let flat = vec![0; span * span];
    points
        .iter()
        .map(|&[x, y]| {
            let page = page_of([x, y]);
            let [min, _] = epsg3857::get_page_bbox(page);
            let grid = grids.get(&page).unwrap_or(&flat);
            let cell_per_m = u_per_m(from_db(min[1])) / U_PER_CELL;
            let (tx, ty) = (from_db(x - min[0]) / U_PER_CELL, from_db(y - min[1]) / U_PER_CELL);
            f64::from(bicubic(grid, tx, ty)) / cell_per_m
        })
        .collect()
}

pub fn page_of([x, y]: [f64; 2]) -> IVec {
    let mut at = Pvf3::default();
    epsg3857::project(&mut at, x, y, 0.0);
    L3_IDX(at.i)
}
