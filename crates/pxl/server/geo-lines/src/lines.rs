// pxl-geo-bake/terrain/pipeline/deck.rs

//! A page's decks (pxl-spec bake/deck.rs) from the rows the offline solve
//! wrote (deck/): each deck's line in the page's cells, its tops in the
//! page's z16 cells, its width as the solve took it from its way
//! (overture/width.rs) and its surface as its way shows on the ground
//! (overture/policy.rs), so a way reads the same on a bridge as off it.
//!
//! A deck's row is its whole line, and a page keeps the runs of it whose
//! segments come within reach of the page — whole segments, so every point
//! a page keeps is one the solve placed and two pages that share a deck agree
//! on it point for point. A segment is within reach where it comes within
//! the deck's reach of the page (pxl-spec bake/way.rs), past which nothing
//! of it can be drawn there.

use pxl_spec::bake::constant::{CELL_PER_M, XY_CELL_PER_M};
use pxl_spec::bake::deck::{Builder, Decks, Form, POINT_SCALE};
use pxl_spec::bake::format::DeckRow;
use pxl_spec::bake::way;
use pxl_spec::space::adapter::epsg3857::G3857;
use pxl_spec::space::pyramid::PAGE_CELLS;

use crate::terrain::pipeline::overture::policy::segment_type;
use crate::wkb::wkb_to_geometries;

/// The page's decks, or none where they do not fit a record — which the
/// bake refuses rather than cuts, since a deck left out is a gap in a road.
pub fn bake_decks(rows: &[DeckRow], page_min: G3857) -> Option<Decks> {
    let cell_per_m = CELL_PER_M(page_min[1]);
    let mut decks = Builder::default();
    for row in rows {
        let kind = segment_type(&row.subtype, row.class.as_deref());
        let width = row.width_m * cell_per_m;
        let width_code = (width * POINT_SCALE).round().clamp(1.0, f64::from(u8::MAX)) as u8;
        for g in wkb_to_geometries(&row.geom_wkb) {
            if g.points.len() != row.piers.len() {
                continue;
            }
            let local: Vec<([f64; 2], i16, u8)> = g
                .points
                .iter()
                .zip(row.piers.iter())
                .map(|(p, &pier)| {
                    let at = [(p[0] - page_min[0]) * XY_CELL_PER_M, (p[1] - page_min[1]) * XY_CELL_PER_M];
                    (at, (p[2] * cell_per_m).round() as i16, pier)
                })
                .collect();
            for run in runs_within(&local, way::reach(width)) {
                decks.push(run.iter().copied(), width_code, kind, Form::of(row.form));
            }
        }
    }
    decks.fits().then(|| decks.finish())
}

/// The runs of consecutive segments of a line that come within `reach`
/// cells of the page.
fn runs_within(points: &[([f64; 2], i16, u8)], reach: f64) -> Vec<&[([f64; 2], i16, u8)]> {
    let (lo, hi) = (-reach, f64::from(PAGE_CELLS) + reach);
    let near = |a: [f64; 2], b: [f64; 2]| (0..2).all(|k| a[k].max(b[k]) >= lo && a[k].min(b[k]) <= hi);
    let mut runs = Vec::new();
    let mut start: Option<usize> = None;
    for s in 0..points.len().saturating_sub(1) {
        match (near(points[s].0, points[s + 1].0), start) {
            (true, None) => start = Some(s),
            (false, Some(a)) => {
                runs.push(&points[a..=s]);
                start = None;
            }
            _ => {}
        }
    }
    if let Some(a) = start {
        runs.push(&points[a..]);
    }
    runs
}
