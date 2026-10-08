// pxl-geo-bake/terrain/pipeline/separation.rs

//! A tile's separation from the solve's rows. A tile keeps whole segments within
//! a line's footprint (pxl-spec bake/separation.rs `footprint`), so tiles sharing a
//! line agree point for point.

use crate::terrain::CELL_U;
use pxl_spec::bake::separation::{self, Builder, Separation, POINT_SCALE};
use pxl_spec::space::coord::adapter::epsg3857::{u_from_mer, G3857};
use pxl_spec::space::layer::lattice::TILE_CELLS;
use xpute_core::math::scalar::round;

use crate::separation::Line;
use crate::terrain::pipeline::overture::policy::segment_type;

/// None where they do not fit a record: refused rather than cut, since a
/// missing line is a gap in a road.
pub fn bake_separation(lines: &[Line], tile_min: G3857) -> Option<Separation> {
    let mut separation = Builder::default();
    for row in lines {
        let kind = segment_type(&row.subtype, row.class.as_deref());
        let width = row.width_u / CELL_U;
        let width_code = round(width * POINT_SCALE).clamp(1.0, f64::from(u8::MAX)) as u8;
        let local: Vec<([f64; 2], i16)> = row
            .points
            .iter()
            .map(|p| {
                let at = [u_from_mer(p[0] - tile_min[0]) / CELL_U, u_from_mer(p[1] - tile_min[1]) / CELL_U];
                (at, round(p[2]) as i16)
            })
            .collect();
        for [a, b] in runs_within(&local, separation::footprint(width)) {
            separation.push(local[a..=b].iter().copied(), width_code, kind, row.form, row.clear);
        }
    }
    separation.fits().then(|| separation.finish())
}

fn runs_within(points: &[([f64; 2], i16)], reach: f64) -> Vec<[usize; 2]> {
    let (lo, hi) = (-reach, f64::from(TILE_CELLS) + reach);
    let near = |a: [f64; 2], b: [f64; 2]| (0..2).all(|k| a[k].max(b[k]) >= lo && a[k].min(b[k]) <= hi);
    let mut runs = Vec::new();
    let mut start: Option<usize> = None;
    for s in 0..points.len().saturating_sub(1) {
        match (near(points[s].0, points[s + 1].0), start) {
            (true, None) => start = Some(s),
            (false, Some(a)) => {
                runs.push([a, s]);
                start = None;
            }
            _ => {}
        }
    }
    if let Some(a) = start {
        runs.push([a, points.len() - 1]);
    }
    runs
}
