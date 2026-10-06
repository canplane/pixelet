// pxl-geo-bake/terrain/pipeline/deck.rs

//! A tile's decks from the solve's rows. A tile keeps whole segments within
//! the deck footprint (pxl-spec bake/deck.rs `footprint`), so tiles sharing a
//! deck agree point for point.

use pxl_spec::bake::constant::U_PER_CELL;
use pxl_spec::bake::deck::{self, Builder, Decks, POINT_SCALE};
use pxl_spec::space::adapter::epsg3857::{from_db, G3857};
use pxl_spec::space::pyramid::TILE_CELLS;
use xpute_core::math::scalar::round;

use crate::deck::Line;
use crate::terrain::pipeline::overture::policy::segment_type;

/// None where they do not fit a record: refused rather than cut, since a
/// missing deck is a gap in a road.
pub fn bake_decks(lines: &[Line], tile_min: G3857) -> Option<Decks> {
    let mut decks = Builder::default();
    for row in lines {
        let kind = segment_type(&row.subtype, row.class.as_deref());
        let width_code = round(row.width_cells * POINT_SCALE).clamp(1.0, f64::from(u8::MAX)) as u8;
        let local: Vec<([f64; 2], i16)> = row
            .points
            .iter()
            .map(|p| {
                let at = [from_db(p[0] - tile_min[0]) / U_PER_CELL, from_db(p[1] - tile_min[1]) / U_PER_CELL];
                (at, round(p[2]) as i16)
            })
            .collect();
        for [a, b] in runs_within(&local, deck::footprint(row.width_cells)) {
            decks.push(local[a..=b].iter().copied(), width_code, kind, row.form, row.clear);
        }
    }
    decks.fits().then(|| decks.finish())
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
