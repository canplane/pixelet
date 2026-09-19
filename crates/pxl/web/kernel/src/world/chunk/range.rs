// pxl-kernel/world/chunk/range.rs

//! What a view needs: which L3 pages, and which 64x64 chunks inside one.

use pxl_spec::bake::constant::{CELL_SHAMT, CHUNK_CELL_SPAN, PAGE_CELL_SPAN, U_PER_CELL};
use pxl_spec::space::indexer::{FROM_L3_IDX, L3_IDX, L3_SHAMT};
use pxl_spec::space::pvf3::ivec::{self, IVec};
use pxl_spec::space::xy::TO_XY;
use xpute_core::math::scalar::round;

use crate::layer::terrain::fixed::KEEP_PAGES;

pub const PAGE_CHUNK_AXIS: u32 = PAGE_CELL_SPAN / CHUNK_CELL_SPAN;
pub const CHUNK_WORLD_SPAN: f64 = CHUNK_CELL_SPAN as f64 * U_PER_CELL;
/// One page's span in world units.
pub const PAGE_SPAN: f64 = PAGE_CELL_SPAN as f64 * U_PER_CELL;
/// A page's side as a shift: the L3 level's, less the cell's.
pub const PAGE_SHAMT: u32 = L3_SHAMT - CELL_SHAMT as u32;

const _: () = assert!(1 << PAGE_SHAMT == PAGE_CELL_SPAN, "the shift is the span's");
/// How far past the render range something already built is kept.
///
/// Building is decided at CHUNK_RENDER_DIST and discarding only out here, so
/// the two thresholds cannot trade a chunk back and forth. Without it, a page
/// whose nearest corner sat 1680u away against a 1664u range was unmounted
/// with every chunk in it, and panning 16u back rebuilt all of them from the
/// scan up, which reads on screen as the side you are
/// moving away from being cut off and redrawn
/// (issue/20260912-what-to-render-when.md).
///
/// Four chunks, which is far more than a frame of panning can cross.
pub const RANGE_HYSTERESIS: f64 = 4.0 * CHUNK_WORLD_SPAN;
const CHUNK_ALIGN: i64 = !(CHUNK_CELL_SPAN as i64 - 1);

const _: () = assert!(PAGE_CHUNK_AXIS * CHUNK_CELL_SPAN == PAGE_CELL_SPAN, "render: an L3 page must contain a whole number of chunks");

/// Squared distance from the origin to an axis-aligned XY rectangle.
pub fn rect_dist2(x0: f64, y0: f64, x1: f64, y1: f64) -> f64 {
    let dx = if 0.0 < x0 {
        x0
    } else if 0.0 > x1 {
        -x1
    } else {
        0.0
    };
    let dy = if 0.0 < y0 {
        y0
    } else if 0.0 > y1 {
        -y1
    } else {
        0.0
    };
    dx * dx + dy * dy
}

/// The position a view's ranges are measured from: the view centre, snapped
/// down to a chunk.
///
/// Chunk-aligned and not page-aligned, which is the whole point. Keyed on the
/// page the centre happened to be in, every range stepped by a whole page as a
/// boundary was crossed, and a page's worth of terrain entered or left in one
/// frame (issue/20260912-what-to-render-when.md). Snapped rather than exact so
/// that moving within one chunk re-derives nothing.
pub fn range_anchor_of(center: IVec) -> IVec {
    ivec::gather(ivec::X(center) & CHUNK_ALIGN, ivec::Y(center) & CHUNK_ALIGN, 0)
}

/// Squared distance from `anchor` to the nearest point of `page_idx`'s own
/// square. Zero while the anchor stands inside that page.
pub fn page_dist2(anchor: IVec, page_idx: IVec) -> f64 {
    let delta = ivec::sub(FROM_L3_IDX(page_idx), anchor);
    let x0 = ivec::to_real(ivec::X(delta));
    let y0 = ivec::to_real(ivec::Y(delta));
    rect_dist2(x0, y0, x0 + PAGE_SPAN, y0 + PAGE_SPAN)
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct PageAt {
    pub page_idx: IVec,
    pub dist2: f64,
}

/// Every page whose square comes within `radius` of `anchor`, nearest first,
/// and never further than KEEP_PAGES from the anchor's own page.
///
/// The page stays the streaming unit — one query, one bake, one decode, one
/// cache key, one eviction — and this is the *set* of them. Which pages is
/// still a distance: a page enters when the disc reaches it, which happens
/// continuously as the view moves, and `radius` past KEEP_DIST is how a page
/// already being fetched stays wanted while the view drifts.
///
/// How far the set can reach is not a distance. `.fixed.terrain` holds one record per
/// page the load radius reaches and addresses it by the page's own coordinate
/// (terrain/fixed.rs), so two pages KEEP_PAGES*2+1 apart are one record. The
/// disc alone would hand out both: with the anchor on a page boundary it
/// reaches four pages back and three forward, and those two are exactly a range
/// axis apart. So the scan is the block the range has records for, and the disc
/// chooses inside it.
pub fn pages_in_radius(anchor: IVec, radius: f64) -> Pages {
    let center_page = TO_XY(L3_IDX(anchor));
    let span = ((radius / PAGE_SPAN).ceil() as i64 + 1).min(KEEP_PAGES as i64);
    let radius2 = radius * radius;
    let mut out = Pages::new();
    for dy in -span..=span {
        for dx in -span..=span {
            let page_idx = ivec::add(center_page, ivec::gather(dx, dy, 0));
            let dist2 = page_dist2(anchor, page_idx);
            if dist2 <= radius2 {
                out.push(PageAt { page_idx, dist2 });
            }
        }
    }
    // Stable, as Array.prototype.sort is: equal distances keep scan order.
    // An insertion sort, over a list a few dozen long.
    for i in 1..out.len() {
        let mut j = i;
        while j > 0 && out[j - 1].dist2.total_cmp(&out[j].dist2).is_gt() {
            out.swap(j - 1, j);
            j -= 1;
        }
    }
    out
}

/// A set of pages, nearest first.
pub type Pages = Vec<PageAt>;

/// The pages a view holds: the nearest `count` of those whose square comes
/// within `radius`, nearest first.
///
/// One definition serves both what is fetched and what is kept, and that is
/// the point rather than a convenience. When the two were separate — fetch a
/// radius, keep a count — a page could be requested and then fall out of the
/// fetch set a moment later while its request was still in flight, and
/// io/scheduler sheds demands its category no longer wants. Panning
/// therefore cancelled its own tiles before they arrived, the page stayed
/// unmounted because a page with no tile is not mounted at all, and the view
/// lost a square of terrain on whichever side it was moving away from
/// (issue/20260912-what-to-render-when.md).
///
/// Because membership now changes only when a page crosses the nearest-`count`
/// ranking, a page being fetched is by construction one worth keeping.
pub fn pages_to_hold(anchor: IVec, radius: f64, count: u32) -> Pages {
    let mut pages = pages_in_radius(anchor, radius);
    pages.truncate(count as usize);
    pages
}

/// How far past the held set a page whose request is already in flight is
/// still asked for. Without it, the 32nd-nearest page changed with every 64u
/// of panning, and io/scheduler yields what a category no longer wants —
/// so panning aborted its own requests at the edge of the set and refetched
/// them a moment later. One
/// page: a pan cannot carry a page that far out before its request settles,
/// while a search jump, which moves by many pages, still yields as designed.
pub const FETCH_HYSTERESIS: f64 = PAGE_SPAN;

/// Fetch order for a page, as a scheduler priority: its distance in whole
/// pages, doubled so a second subsystem can interleave at equal distance
/// (POI sits one behind terrain — see fetcher.rs).
pub fn page_priority(dist2: f64) -> f64 {
    round(dist2.sqrt() / PAGE_SPAN) * 2.0
}

#[cfg(test)]
#[path = "range.test.rs"]
mod test;

#[cfg(test)]
#[path = "view_range.test.rs"]
mod view_range_test;
