// pxl-kernel/world/pyramid/window.rs

//! Which part of the pyramid a view holds and draws, from where the eye is
//! (issue/20260925-what-a-view-costs-is-the-eye-s.md). One rule for every
//! layer on the pyramid and every level of it.
//!
//! A chunk of level L is split into its four children while the nearest
//! point of its box — its ground and its height, the objects on it included
//! — is nearer the eye than `split_distance(L)`, the distance at which one of
//! its cells covers the pixels a cell is allowed. A chunk drawn is one the
//! rule did not split under a parent it did, so every cell drawn covers at
//! most that many pixels and at least a fixed share of it, whatever the tilt
//! and wherever the eye is; and the chunks a level draws lie within a fixed
//! reach of the eye, so a level costs a fixed number of chunks and pages.
//! The map centre is only the handle the camera is moved by and plays no
//! part in it.

use std::f64::consts::{PI, SQRT_2};

use pxl_render::look::lod::REFERENCE_THRESHOLD_0;
use pxl_spec::space::indexer::L3_IDX;
use pxl_spec::space::pvf3::ivec::{self, IVec};
use pxl_spec::space::pyramid::{span_of, LevelPage, CHUNK_CELLS, GROUND, PAGE_CELLS, PAGE_CHUNKS, TOP_LEVEL};
use pxl_spec::space::xy::TO_XY;

use crate::layer::environment::fog::{eye_height, reach_at};
use crate::layer::object::store::column_boxes;
use crate::layer::terrain::relief::range_over;
use crate::shell::map_store::map_store;
use crate::world::frame::view::view;

use super::chunk::Chunk;

const _: () = assert!(HELD_AXIS % 2 == 1, "a ring is centred on the eye's page");
const _: () = assert!(WINDOW_LEVELS <= TOP_LEVEL + 1, "a window no wider than the pyramid");

/// The one criterion, as a distance per unit of a cell's side: a cell covers
/// the pixels a cell is allowed at `SPLIT` times its side from the eye
/// (pxl-render look/lod.rs, which says what it was derived from). What those
/// pixels are, and whether a device's work or its picture is what stays
/// equal, is measured rather than decided here.
const SPLIT: f64 = REFERENCE_THRESHOLD_0;

/// Where a level-`level` chunk stops being split: nearer than this, one of
/// its cells covers more than a cell's pixels. Any integer level, the ones
/// under a cell included.
pub const fn split_distance(level: i32) -> f64 {
    SPLIT * GROUND.scale(0).up(level).side()
}

/// How far from the eye's foot a level's chunks can lie, in that level's
/// cells. A chunk is drawn at a level only under a parent the rule split,
/// whose nearest point was inside the parent's split distance — twice the
/// level's own — and whose whole ground lies within its diagonal of that
/// point.
const LEVEL_REACH_CELLS: f64 = 2.0 * SPLIT + 2.0 * SQRT_2 * CHUNK_CELLS as f64;

/// The most chunks one level draws: every chunk with ground inside that
/// reach, a disc widened by a chunk's diagonal. What a level of the chunk
/// table and of the mesh arena is sized for (world/chunk/table.rs,
/// world/chunk/mesh_slot.rs), derived rather than set.
pub const LEVEL_CHUNKS: u32 = {
    let r = LEVEL_REACH_CELLS / CHUNK_CELLS as f64 + SQRT_2;
    (PI * r * r) as u32 + 1
};

/// The most levels a view holds: from the one the ground under the eye wants,
/// this many up. It is the budget's other factor — a view costs at most
/// LEVEL_CHUNKS a level times this — and so it is also how far the ground is
/// drawn: past the coarsest level's split distance there is no level left to
/// draw it, and the fog band ends the ground there where the air has not
/// already (layer/environment/fog.rs `band_at`). `.fixed` is sized for it.
pub const WINDOW_LEVELS: u32 = 5;

/// Pages held along one axis of a level's ring, around the eye's own page.
/// Odd, so the ring is centred, and as wide as a level's chunks can reach
/// from the eye wherever in its page the eye stands: that is what makes the
/// ring the whole of what a level can want, so its records are this squared
/// a level.
pub const HELD_AXIS: u32 = 5;

const _: () = assert!(LEVEL_REACH_CELLS <= (HELD_AXIS / 2 * PAGE_CELLS) as f64, "a level's chunks reach past its held ring");

/// The pages a view holds over its window's top, so that ground whose page
/// has not come always has something coarser to stand in for it: every
/// ancestor of a root's page, up to the pyramid's top (Google Earth holds its
/// root, Cesium preloads ancestors). A root lies inside the reach and a chunk
/// of the top level beyond it, which is inside `split_distance(top + 1)` plus
/// a top chunk — SPLIT plus half a chunk of the level above's cells from the
/// eye's foot, a square under a page across — so each level over the window
/// takes at most 2 × 2 pages, and parents of those no more.
pub const CHAIN_PAGES: u32 = 4 * TOP_LEVEL;

const _: () = assert!(2.0 * SPLIT + CHUNK_CELLS as f64 <= PAGE_CELLS as f64, "a level over the window takes more than 2 × 2 pages");

/// Where the eye is: its position in units, and the point under it the rings
/// are centred on.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Eye {
    pub at: [f64; 3],
    pub foot: IVec,
}

/// The eye as the view draws it, once there is a map to stand over.
pub fn eye() -> Option<Eye> {
    map_store().map_center?;
    let v = view();
    let at = [
        ivec::to_real(ivec::X(v.origin)) + v.eye_local.x,
        ivec::to_real(ivec::Y(v.origin)) + v.eye_local.y,
        ivec::to_real(ivec::Z(v.origin)) + v.eye_local.z,
    ];
    let foot = ivec::gather(at[0].floor() as i64, at[1].floor() as i64, 0);
    Some(Eye { at, foot })
}

/// The level the rule draws ground `dist` from the eye at: the one whose
/// split distance it is past and the next one's it is not.
fn level_at(dist: f64) -> u32 {
    ((dist / split_distance(0)).log2().floor().max(0.0) as u32).min(TOP_LEVEL)
}

/// The levels a view holds, finest first, for an eye `height` over the
/// ground under it: from the level that ground wants, WINDOW_LEVELS up at
/// most.
pub fn window_for(height: f64) -> std::ops::Range<u32> {
    let base = level_at(height);
    base..(base + WINDOW_LEVELS).min(TOP_LEVEL + 1)
}

/// `window_for` the eye as it stands now; empty before there is a map.
pub fn window() -> std::ops::Range<u32> {
    eye().map_or(0..0, |_| window_for(eye_height()))
}

/// How far the view draws the ground as it stands now (`Domain::far`).
pub fn drawn_far() -> Option<f64> {
    eye().map(|e| domain(&e).far)
}

/// How far the leaves are drawn for certain, and so decoration, which is
/// scattered from them: level 1's split distance, inside which every chunk
/// the window reaches is split to the leaves.
pub const DECOR_REACH: f64 = split_distance(1);

/// The page `anchor` stands in at `level`: what every ring is centred on.
pub fn centre_page(level: u32, anchor: IVec) -> LevelPage {
    LevelPage::over(level, TO_XY(L3_IDX(anchor)))
}

/// A level's ring around the page the anchor stands in at that level: every
/// page within `axis / 2` of it in both axes, nearest first by Chebyshev
/// ring, then by row.
pub fn ring(level: u32, anchor: IVec, axis: u32) -> Vec<LevelPage> {
    let center = centre_page(level, anchor);
    let r = (axis / 2) as i64;
    let mut out: Vec<(i64, LevelPage)> = Vec::with_capacity((axis * axis) as usize);
    for dy in -r..=r {
        for dx in -r..=r {
            let page = ivec::add(center.page, ivec::gather(dx, dy, 0));
            out.push((dx.abs().max(dy.abs()), LevelPage::new(level, page)));
        }
    }
    out.sort_by_key(|(ring, _)| *ring);
    out.into_iter().map(|(_, p)| p).collect()
}

/// How far `page` lies from the anchor's page at its level, in pages along
/// the further axis: 0 for the anchor's own, 1 for the ring drawn from.
pub fn ring_of(anchor: IVec, page: LevelPage) -> u32 {
    let center = centre_page(page.level, anchor);
    let d = ivec::sub(page.page, center.page);
    ivec::X(d).unsigned_abs().max(ivec::Y(d).unsigned_abs()) as u32
}

/// A level's held pages: its ring whole, the part the level below also
/// holds included. Those are what a chunk draws where the finer page under
/// it has not come, so the ground coarsens there rather than opening; a
/// clipmap keeps every level whole for the same reason, and `.fixed` has a
/// record for each (mem/fixed.rs).
pub fn held(level: u32, anchor: IVec) -> Vec<LevelPage> {
    ring(level, anchor, HELD_AXIS)
}

/// What a view wants drawn, before anything is known of what is resident:
/// every piece of the ground within its reach once, each at the level the
/// rule wants it at. Every layer on the pyramid is asked for this same
/// ground at these same levels; what each can draw of it is its own
/// (`Pyramid::cover`).
pub struct Domain {
    window: std::ops::Range<u32>,
    eye: [f64; 3],
    foot: IVec,
    /// The height of the ground under the eye: what ground no page over it is
    /// held for is taken to stand at, until one is.
    ground: f64,
    /// How far the ground is drawn: where the air ends it, or past the
    /// coarsest level's reach, whichever is nearer.
    pub far: f64,
    /// The pages held over the window's top (`CHAIN_PAGES`), coarsest last.
    chain: Vec<LevelPage>,
}

/// The domain of an eye `height` over the ground under it.
pub fn domain_for(eye: &Eye, height: f64) -> Domain {
    let window = window_for(height);
    let far = reach_at(height).min(split_distance(window.end as i32));
    let mut d = Domain {
        window,
        eye: eye.at,
        foot: eye.foot,
        ground: eye.at[2] - height,
        far,
        chain: Vec::new(),
    };
    let mut level: Vec<LevelPage> = d.roots().into_iter().map(|c| c.page).collect();
    while level.first().is_some_and(|p| p.level < TOP_LEVEL) {
        level = level.iter().map(|p| p.parent()).collect();
        level.sort_unstable();
        level.dedup();
        d.chain.extend(&level);
    }
    d
}

/// The domain of the eye as it stands.
pub fn domain(eye: &Eye) -> Domain {
    domain_for(eye, eye_height())
}

impl Domain {
    pub fn window(&self) -> std::ops::Range<u32> {
        self.window.clone()
    }

    /// The point under the eye the rings are centred on.
    pub fn foot(&self) -> IVec {
        self.foot
    }

    /// The nearest distance from the eye to a box: `x0, y0, side` on the
    /// ground and the heights it spans, or the ground under the eye's where
    /// none is known.
    fn nearest(&self, (x0, y0, side): (f64, f64, f64), z: Option<(f64, f64)>) -> f64 {
        let [ex, ey, ez] = self.eye;
        let gap = |e: f64, lo: f64, hi: f64| (lo - e).max(e - hi).max(0.0);
        let (zlo, zhi) = z.unwrap_or((self.ground, self.ground));
        gap(ex, x0, x0 + side).hypot(gap(ey, y0, y0 + side)).hypot(gap(ez, zlo, zhi))
    }

    /// The heights a box of `page`'s ground spans: its terrain's, and the
    /// objects standing on it.
    fn heights(page: LevelPage, (x0, y0, side): (f64, f64, f64)) -> Option<(f64, f64)> {
        let mut z = range_over(page);
        for b in column_boxes() {
            if b.x0 < x0 + side && x0 < b.x0 + b.side && b.y0 < y0 + side && y0 < b.y0 + b.side {
                z = Some(z.map_or(b.z, |(lo, hi)| (lo.min(b.z.0), hi.max(b.z.1))));
            }
        }
        z
    }

    /// How near the eye `c` comes.
    pub fn nearest_to(&self, c: Chunk) -> f64 {
        let rect = c.rect();
        self.nearest(rect, Self::heights(c.page, rect))
    }

    /// How near the eye `p` comes.
    pub fn page_nearest_to(&self, p: LevelPage) -> f64 {
        let span = span_of(p.level);
        let rect = (ivec::X(p.page) as f64 * span, ivec::Y(p.page) as f64 * span, span);
        self.nearest(rect, Self::heights(p, rect))
    }

    /// Where the partition starts: every chunk of the window's top level
    /// within the reach.
    pub fn roots(&self) -> Vec<Chunk> {
        let mut out = Vec::new();
        for page in ring(self.window.end - 1, self.foot, HELD_AXIS) {
            for cy in 0..PAGE_CHUNKS {
                for cx in 0..PAGE_CHUNKS {
                    let c = Chunk {
                        page,
                        bx: cx * CHUNK_CELLS,
                        by: cy * CHUNK_CELLS,
                    };
                    if self.nearest_to(c) < self.far {
                        out.push(c);
                    }
                }
            }
        }
        out
    }

    /// The pages held over the window's top (`CHAIN_PAGES`).
    pub fn chain(&self) -> &[LevelPage] {
        &self.chain
    }

    /// The ground the view draws, as the roots' squares: a corner and a side
    /// each, in units.
    pub fn ground(&self) -> Vec<(f64, f64, f64)> {
        self.roots().into_iter().map(Chunk::rect).collect()
    }

    /// Whether the rule wants `c`'s ground finer than `c`: it comes nearer the
    /// eye than its level's split distance, and the window holds a level
    /// under it.
    pub fn splits(&self, c: Chunk) -> bool {
        c.level() > self.window.start && self.nearest_to(c) < split_distance(c.level() as i32)
    }

    /// Whether some of `p`'s ground is drawn at `p`'s level: it lies under a
    /// parent the rule splits, or is a root. A held page this says no of is
    /// margin against a move that has not happened.
    pub fn reaches(&self, p: LevelPage) -> bool {
        let top = self.window.end - 1;
        let within = if p.level == top { self.far } else { split_distance(p.level as i32 + 1) };
        self.window.contains(&p.level) && self.page_nearest_to(p) < within
    }

    /// Whether the view wants `c`'s ground at the window's top, or `c` is an
    /// ancestor over it standing in: nothing coarser would draw that ground's
    /// far steps.
    pub fn outermost(&self, c: Chunk) -> bool {
        if c.level() >= self.window.end {
            return true;
        }
        let top = std::iter::once(c).chain(c.ancestors()).find(|a| a.level() + 1 == self.window.end);
        top.is_some_and(|t| !self.splits(t))
    }

    /// The chunk over `c`'s ground at the level the view wants that ground
    /// at: `c` itself where the view wants it at `c`'s level or finer.
    pub fn wanted_over(&self, c: Chunk) -> Chunk {
        let chain: Vec<Chunk> = std::iter::once(c).chain(c.ancestors().take_while(|a| a.level() < self.window.end)).collect();
        chain.into_iter().rev().find(|&k| !self.splits(k)).unwrap_or(c)
    }
}

#[cfg(test)]
#[path = "window.test.rs"]
mod test;
