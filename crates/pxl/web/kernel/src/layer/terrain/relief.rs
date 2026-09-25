// pxl-kernel/layer/terrain/relief.rs

//! How high the ground under a piece of the view stands: the lowest and
//! highest point of the surface drawn over its footprint. A view measures how
//! near it is to ground from this (world/pyramid/domain.rs `Domain`), in three
//! dimensions, so a hill is as near as it stands and the flat ground beside
//! it is as far as it lies.
//!
//! Terrain is stored as a heightfield, a 2D page of elevation samples; this
//! is where a node of the view, which is 3D, is pointed at it. The footprint
//! is read from the page the ground is drawn from — its own, or the nearest
//! resident one over it, upsampled (world/pyramid.rs `drawn_from`) — so the
//! height a node is measured at is the height of the surface on the screen.
//!
//! Only the samples under the footprint are read, never a page's whole
//! range. A mountain in one corner of a page then makes its own ground near
//! and no other. And ground read from a page over it comes out no wider than
//! it will once its own page lands, but for the samples at its rim: a
//! parent's sample is the mean of an aligned 2 × 2 of its children's
//! (pxl-spec bake/fold.rs `fold_ele`), and a mean lies inside what it
//! averages. An arrival widens a footprint's range rather than narrowing it,
//! so a view refines as pages land and does not coarsen back.

use pxl_spec::bake::constant::{ELE_PAD, ELE_PADDED_SPAN, PAGE_ELE_SPAN, U_PER_CELL};
use pxl_spec::bake::format::Terrain;
use pxl_spec::space::hierarchy::Scale;
use pxl_spec::space::pyramid::{LevelPage, GROUND, PAGE_CELLS};

use crate::world::pyramid::Layer as _;

/// Cells a sample stands for along an axis.
const CELL_STEP: f64 = PAGE_CELLS as f64 / PAGE_ELE_SPAN as f64;

/// The heights, in units, the ground over a node of `page`'s level spans:
/// `node`'s footprint, read from the page it is drawn from. None where
/// nothing over it is held.
///
/// A cell's height is the Catmull-Rom interpolation of the 4 × 4 samples
/// about it (pxl-render mesh/paint/terrain/elevation.rs). Its weights are
/// negative by at most an eighth along an axis; the positive part of the two
/// axes' products then sums to at most (9/8)² + (1/8)² = 41/32, so an
/// interpolated height lies within 9/32 of those sixteen samples' span past
/// either end of it. The bound is each window's, and taken window by window:
/// over the whole footprint's span it reaches past ground that does not move
/// by as much as a quarter of the tallest relief anywhere in the footprint,
/// water at sea level beside a hill read that far under the sea, and a chunk
/// holding only what the bound reached into is made one all the same
/// (world/pyramid/domain.rs `Domain::occupied`).
pub fn ground_under(page: LevelPage, (side, at): (Scale, [i64; 2])) -> Option<(f64, f64)> {
    let pyramid = Terrain::pyramid();
    let src = pyramid.drawn_from(page)?;
    let ele = &pyramid.get(src)?.ele;
    let corner = GROUND.cell_of(page.level, &side.base_of([at[0], at[1], 0]));
    // The footprint in the source page's cells: its own cells, n of them to
    // one of the source's.
    let n = GROUND.scale(page.level).count_in(GROUND.scale(src.level)) as f64;
    let [ox, oy] = corner.in_page_above(src.level - page.level);
    let cells = GROUND.scale(page.level).count_in(side) as f64 / n;
    let span = ELE_PADDED_SPAN as i64;
    let samples = |t0: f64| {
        let s = |t: f64| (t / CELL_STEP + ELE_PAD as f64 - 0.5).floor() as i64;
        ((s(t0) - 1).clamp(0, span - 1), (s(t0 + cells) + 2).clamp(0, span - 1))
    };
    let ((x0, x1), (y0, y1)) = (samples(ox as f64 / n), samples(oy as f64 / n));
    // Every 4 × 4 window a cell of the footprint is interpolated from.
    let window = |a0: i64, a1: i64, k: i64| (k.max(a0), (k + 3).min(a1));
    let (mut lo, mut hi) = (f64::INFINITY, f64::NEG_INFINITY);
    for wy in y0..=(y1 - 3).max(y0) {
        for wx in x0..=(x1 - 3).max(x0) {
            let ((ax, bx), (ay, by)) = (window(x0, x1, wx), window(y0, y1, wy));
            let (mut wlo, mut whi) = (i16::MAX, i16::MIN);
            for y in ay..=by {
                for &e in &ele[(y * span + ax) as usize..=(y * span + bx) as usize] {
                    (wlo, whi) = (wlo.min(e), whi.max(e));
                }
            }
            let past = (whi - wlo) as f64 * 9.0 / 32.0;
            lo = lo.min(wlo as f64 - past);
            hi = hi.max(whi as f64 + past);
        }
    }
    Some((lo * U_PER_CELL, hi * U_PER_CELL))
}
