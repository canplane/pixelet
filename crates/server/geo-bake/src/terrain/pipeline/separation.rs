// pxl-geo-bake/terrain/pipeline/separation.rs

//! A tile's separation from the solve's rows. A tile keeps whole segments within
//! a line's footprint (pxl-spec bake/separation.rs `footprint`, `runs_within`),
//! each point quantized once in the world's grid of leaf cells and the tile's
//! integer origin then taken off, so tiles sharing a line hold the same
//! numbers for its points: what the fold takes each segment once by.

use crate::terrain::CELL_U;
use pxl_spec::bake::separation::{self, Builder, Separation, POINT_SCALE};
use pxl_spec::space::coord::adapter::epsg3857::{get_tile_bbox, u_from_mer};
use pxl_spec::space::layer::lattice::TILE_CELLS;
use pxl_spec::space::layer::tile::LEAF_ZOOM;
use xpute_kit::math::scalar::round;

use crate::separation::Line;
use crate::terrain::pipeline::overture::policy::segment_type;

/// All of them: a tile whose lines a record's room does not hold is refused
/// where it is encoded rather than cut, since a missing line is a gap in a
/// road.
pub fn bake_separation(lines: &[Line], tile: [i64; 2]) -> Separation {
    let mut separation = Builder::default();
    // The world's corner, where its grid of leaf cells starts; a point in that
    // grid, in 1/POINT_SCALE of a cell, the tile's own whole cells taken off.
    let corner = get_tile_bbox(0, [0, 0])[0];
    let grid = |m: f64, k: usize| round(u_from_mer(m - corner[k]) / CELL_U * POINT_SCALE);
    let origin = [0, 1].map(|k| (tile[k] * i64::from(TILE_CELLS)) as f64 * POINT_SCALE);
    debug_assert!({
        let min = get_tile_bbox(LEAF_ZOOM, tile)[0];
        [0, 1].iter().all(|&k| grid(min[k], k) == origin[k])
    });
    for row in lines {
        let kind = segment_type(&row.subtype, row.class.as_deref());
        let width = row.width_u / CELL_U;
        let width_code = separation::width_byte(width);
        let local: Vec<([f64; 2], i16)> = row
            .points
            .iter()
            .map(|p| {
                let at = [0, 1].map(|k| (grid(p[k], k) - origin[k]) / POINT_SCALE);
                (at, round(p[2]) as i16)
            })
            .collect();
        let at: Vec<[f64; 2]> = local.iter().map(|p| p.0).collect();
        for [a, b] in separation::runs_within(&at, separation::footprint(width)) {
            separation.push(local[a..=b].iter().copied(), width_code, kind, row.form, row.clear);
        }
    }
    separation.finish()
}
