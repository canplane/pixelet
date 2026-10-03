// pxl-geo-bake/terrain/pipeline/deck.rs

//! A page's decks from the solve's rows. A page keeps whole segments within
//! the deck footprint (pxl-spec bake/deck.rs `footprint`), so pages sharing a
//! deck agree point for point.

use pxl_spec::bake::constant::U_PER_CELL;
use pxl_spec::space::adapter::epsg3857::{from_db, u_per_m};
use pxl_spec::bake::deck::{self, Builder, Decks, Form, POINT_SCALE};
use pxl_spec::bake::format::DeckRow;
use pxl_spec::space::adapter::epsg3857::G3857;
use pxl_spec::space::pyramid::PAGE_CELLS;

use crate::terrain::pipeline::overture::policy::segment_type;
use crate::wkb::wkb_to_geometries;
use xpute_core::math::scalar::round;

/// None where they do not fit a record: refused rather than cut, since a
/// missing deck is a gap in a road.
pub fn bake_decks(rows: &[DeckRow], page_min: G3857) -> Option<Decks> {
    let cell_per_m = u_per_m(from_db(page_min[1])) / U_PER_CELL;
    let mut decks = Builder::default();
    for row in rows {
        let kind = segment_type(&row.subtype, row.class.as_deref());
        let width = row.width_m * cell_per_m;
        let width_code = round(width * POINT_SCALE).clamp(1.0, f64::from(u8::MAX)) as u8;
        let form = Form::of(row.form);
        for g in wkb_to_geometries(&row.geom_wkb) {
            let local: Vec<([f64; 2], i16)> = g
                .points
                .iter()
                .map(|p| {
                    let at = [from_db(p[0] - page_min[0]) / U_PER_CELL, from_db(p[1] - page_min[1]) / U_PER_CELL];
                    (at, round(p[2] * cell_per_m) as i16)
                })
                .collect();
            for [a, b] in runs_within(&local, deck::footprint(width)) {
                decks.push(local[a..=b].iter().copied(), width_code, kind, form);
            }
        }
    }
    decks.fits().then(|| decks.finish())
}

fn runs_within(points: &[([f64; 2], i16)], reach: f64) -> Vec<[usize; 2]> {
    let (lo, hi) = (-reach, f64::from(PAGE_CELLS) + reach);
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
