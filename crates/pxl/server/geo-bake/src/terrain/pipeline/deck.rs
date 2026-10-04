// pxl-geo-bake/terrain/pipeline/deck.rs

//! A page's decks from the solve's rows. A page keeps whole segments within
//! the deck footprint (pxl-spec bake/deck.rs `footprint`), so pages sharing a
//! deck agree point for point.

use pxl_spec::bake::constant::{CELL_PER_M, XY_CELL_PER_M};
use pxl_spec::bake::deck::{self, Builder, Decks, Form, NO_PORTAL, POINT_SCALE};
use pxl_spec::bake::format::DeckRow;
use pxl_spec::space::adapter::epsg3857::G3857;
use pxl_spec::space::pyramid::PAGE_CELLS;

use crate::terrain::pipeline::overture::policy::segment_type;
use crate::wkb::wkb_to_geometries;

/// None where they do not fit a record: refused rather than cut, since a
/// missing deck is a gap in a road.
pub fn bake_decks(rows: &[DeckRow], page_min: G3857) -> Option<Decks> {
    let cell_per_m = CELL_PER_M(page_min[1]);
    let mut decks = Builder::default();
    for row in rows {
        let kind = segment_type(&row.subtype, row.class.as_deref());
        let width = row.width_m * cell_per_m;
        let width_code = (width * POINT_SCALE).round().clamp(1.0, f64::from(u8::MAX)) as u8;
        let form = Form::of(row.form);
        let portal_m: Vec<f32> = row.portal_m.as_chunks::<4>().0.iter().map(|&b| f32::from_le_bytes(b)).collect();
        for g in wkb_to_geometries(&row.geom_wkb) {
            let from_portal = |i: usize| match portal_m.get(i) {
                Some(&m) if portal_m.len() == g.points.len() => (f64::from(m) * cell_per_m).round().min(f64::from(NO_PORTAL - 1)) as u16,
                _ => NO_PORTAL,
            };
            let local: Vec<([f64; 2], i16, u16)> = g
                .points
                .iter()
                .enumerate()
                .map(|(i, p)| {
                    let at = [(p[0] - page_min[0]) * XY_CELL_PER_M, (p[1] - page_min[1]) * XY_CELL_PER_M];
                    (at, (p[2] * cell_per_m).round() as i16, from_portal(i))
                })
                .collect();
            for [a, b] in runs_within(&local, deck::footprint(width)) {
                decks.push(local[a..=b].iter().copied(), width_code, kind, form);
            }
        }
    }
    decks.fits().then(|| decks.finish())
}

fn runs_within(points: &[([f64; 2], i16, u16)], reach: f64) -> Vec<[usize; 2]> {
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
