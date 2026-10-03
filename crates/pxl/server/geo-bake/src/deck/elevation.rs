// pxl-geo-bake/deck/elevation.rs

//! The ground's height at points as the client draws it (the page's elevation
//! with the client's interpolation and its cells' shift), so deck ends and bore
//! floors meet the drawn ground.

use std::collections::HashMap;

use pxl_spec::bake::constant::U_PER_CELL;
use pxl_spec::bake::ground::cell_shift;
use pxl_spec::bake::surface::bicubic;
use pxl_spec::space::adapter::epsg3857;
use pxl_spec::space::adapter::epsg3857::{from_db, u_per_m};
use pxl_spec::space::indexer::L3_IDX;
use pxl_spec::space::pvf3::ivec::IVec;
use pxl_spec::space::pyramid::{GROUND, PAGE_CELLS};

use crate::terrain::artifact::Surface;

/// Heights at points from the pages' surfaces, as the client's
/// `ground_ele_at` reads them; a point's page is `page_of`'s, one not held
/// flat at sea level.
pub fn sample(surfaces: &HashMap<IVec, Surface>, points: &[[f64; 2]], page_of: impl Fn([f64; 2]) -> IVec) -> Vec<f64> {
    points
        .iter()
        .map(|&[x, y]| {
            let page = page_of([x, y]);
            let Some(surface) = surfaces.get(&page) else { return 0.0 };
            let [min, _] = epsg3857::get_page_bbox(page);
            let cell_per_m = u_per_m(from_db(min[1])) / U_PER_CELL;
            let (tx, ty) = (from_db(x - min[0]) / U_PER_CELL, from_db(y - min[1]) / U_PER_CELL);
            let on_page = (0.0..f64::from(PAGE_CELLS)).contains(&tx) && (0.0..f64::from(PAGE_CELLS)).contains(&ty);
            let shift = if on_page { cell_shift(surface.ground.cells[GROUND.cell_index(tx as u32, ty as u32) as usize]) } else { 0 };
            f64::from(bicubic(&surface.ele, tx, ty) + shift) / cell_per_m
        })
        .collect()
}

pub fn page_of([x, y]: [f64; 2]) -> IVec {
    let at = epsg3857::project(x, y, 0.0);
    L3_IDX(at.i)
}
