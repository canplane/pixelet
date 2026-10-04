// pxl-geo-bake/deck/elevation.rs

//! The ground's height at points as the client draws it (the page grids with
//! the client's interpolation), so deck ends and bore floors meet the drawn ground.

use std::collections::HashMap;

use postgres::Client;
use pxl_spec::bake::constant::{CELL_PER_M, ELE_PADDED_SPAN, XY_CELL_PER_M};
use pxl_spec::bake::surface::bicubic;
use pxl_spec::space::adapter::epsg3857;
use pxl_spec::space::indexer::L3_IDX;
use pxl_spec::space::pvf3::ivec::{self, IVec};
use pxl_spec::space::pvf3::pvf3::Pvf3;

use crate::deck::source::fan;
use crate::elevation::Raster;

/// Heights at points from the pages' surfaces, as the client's
/// `ground_ele_at` reads them; a point's page must be held.
pub fn sample(surfaces: &HashMap<IVec, Surface>, points: &[[f64; 2]]) -> Result<Vec<f64>, String> {
    points
        .iter()
        .map(|&[x, y]| {
            let page = page_of([x, y]);
            let surface = surfaces.get(&page).ok_or_else(|| format!("page {page} is not held"))?;
            let [min, _] = epsg3857::get_page_bbox(page);
            let cell_per_m = u_per_m(from_db(min[1])) / U_PER_CELL;
            let (tx, ty) = (from_db(x - min[0]) / U_PER_CELL, from_db(y - min[1]) / U_PER_CELL);
            let on_page = (0.0..f64::from(PAGE_CELLS)).contains(&tx) && (0.0..f64::from(PAGE_CELLS)).contains(&ty);
            let shift = if on_page {
                cell_shift(surface.ground.cells[GROUND.cell_index(tx as u32, ty as u32) as usize])
            } else {
                0
            };
            Ok(f64::from(bicubic(&surface.ele, tx, ty) + shift) / cell_per_m)
        })
        .collect()
}

pub fn page_of([x, y]: [f64; 2]) -> IVec {
    let at = epsg3857::project(x, y, 0.0);
    L3_IDX(at.i)
}
