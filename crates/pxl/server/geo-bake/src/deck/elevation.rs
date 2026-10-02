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

pub fn surface(client: &mut Client, url: &str, points: &[[f64; 2]]) -> Result<Vec<f64>, postgres::Error> {
    let raster = Raster::open(client)?;
    let mut by_page: HashMap<IVec, Vec<usize>> = HashMap::new();
    for (i, p) in points.iter().enumerate() {
        let mut at = Pvf3::default();
        epsg3857::project(&mut at, p[0], p[1], 0.0);
        by_page.entry(L3_IDX(at.i)).or_default().push(i);
    }
    // Pages in rows, so a raster tile is read about once.
    let mut pages: Vec<(&IVec, &Vec<usize>)> = by_page.iter().collect();
    pages.sort_by_key(|(&p, _)| (ivec::Y(p), ivec::X(p)));
    let span = ELE_PADDED_SPAN as usize;
    let heights = fan(url, |client, k, n| {
        let mut out = Vec::new();
        for &(&page, at) in pages.iter().skip(k).step_by(n) {
            let bbox = epsg3857::get_page_bbox(page);
            let [min, _] = bbox;
            // Drawn flat at sea level, as the bake draws it.
            let grid = raster.page_grid(client, bbox)?.unwrap_or_else(|| vec![0; span * span]);
            let cell_per_m = CELL_PER_M(min[1]);
            out.extend(at.iter().map(|&i| {
                let [x, y] = points[i];
                let (tx, ty) = ((x - min[0]) * XY_CELL_PER_M, (y - min[1]) * XY_CELL_PER_M);
                (i, f64::from(bicubic(&grid, tx, ty)) / cell_per_m)
            }));
        }
        Ok(out)
    })?;
    let mut out = vec![0.0; points.len()];
    for (i, h) in heights {
        out[i] = h;
    }
    Ok(out)
}
