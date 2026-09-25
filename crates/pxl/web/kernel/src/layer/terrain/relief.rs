// pxl-kernel/layer/terrain/relief.rs

//! How high the ground a page draws stands: the lowest and highest point of
//! the surface its own elevation samples make, for every resident page. A
//! view measures how near it is to ground from these (world/pyramid/window.rs
//! `Domain`), in three dimensions, so a hill is as near as it stands.
//!
//! A level's chunks are drawn from that level's own samples, so for a
//! resident page the range is the drawn geometry's and exact. A page not yet
//! resident takes its nearest resident ancestor's, which is an estimate and
//! not a bound: a parent's samples average its children's, and an average
//! does not reach its extremes. The estimate is replaced the moment the page
//! arrives.

use std::collections::HashMap;

use pxl_spec::bake::constant::U_PER_CELL;
use pxl_spec::bake::format::Terrain;
use pxl_spec::space::pyramid::{LevelPage, TOP_LEVEL};

use crate::global::Global;

/// Every resident page's range, and how many times the set has changed.
#[derive(Default)]
struct Relief {
    pages: HashMap<LevelPage, (f64, f64)>,
    revision: u32,
}

static RELIEF: Global<Relief> = Global::new();
fn relief() -> &'static mut Relief {
    RELIEF.get(Relief::default)
}

/// The surface a page's samples make, from their range: a cell's height is
/// their Catmull-Rom interpolation (pxl-render mesh/paint/terrain/
/// elevation.rs), whose weights are negative by at most an eighth along an
/// axis. The positive part of the two axes' products then sums to at most
/// (9/8)² + (1/8)² = 41/32, so an interpolated height lies within 9/32 of
/// the samples' span past either end of it.
fn surface_of(page: &Terrain) -> (f64, f64) {
    let (lo, hi) = page.ele.iter().fold((i16::MAX, i16::MIN), |(lo, hi), &e| (lo.min(e), hi.max(e)));
    let (lo, hi) = (lo as f64 * U_PER_CELL, hi as f64 * U_PER_CELL);
    let past = (hi - lo) * 9.0 / 32.0;
    (lo - past, hi + past)
}

/// A page came or went (page.rs `Layer::changed`).
pub fn page_changed(p: LevelPage, page: Option<&Terrain>) {
    let r = relief();
    match page {
        Some(t) => r.pages.insert(p, surface_of(t)),
        None => r.pages.remove(&p),
    };
    r.revision = r.revision.wrapping_add(1);
}

/// Moves whenever a page's range comes or goes, so a reader that decided
/// something from `range_over` can tell whether that answer still stands.
pub fn relief_revision() -> u32 {
    relief().revision
}

/// The heights the ground `p` draws spans, in units: its own where it is
/// resident, its nearest resident ancestor's where it is not, and none where
/// nothing over it is held.
pub fn range_over(p: LevelPage) -> Option<(f64, f64)> {
    let mut at = p;
    loop {
        if let Some(&r) = relief().pages.get(&at) {
            return Some(r);
        }
        if at.level >= TOP_LEVEL {
            return None;
        }
        at = at.parent();
    }
}
