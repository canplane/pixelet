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
use crate::layer::environment::state::globe;
use crate::layer::object::store::{column_boxes, objects_revision};
use crate::layer::terrain::relief::{range_over, relief_revision};
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

/// The view's frustum in world units: its six planes, normals in, as the
/// domain tests ground against them, and the planet's radius, which draws
/// far ground lower than it lies.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Frustum {
    planes: [f64; 24],
    planet: f64,
}

impl Frustum {
    /// The frustum as the view draws it. The view's planes are in its own
    /// frame, which reaches world units as `root · (x − origin)`; a plane
    /// `n · x + d` there is `(Rᵀn) · x + d + n · t − (Rᵀn) · origin` here, the
    /// root being a rotation R and a translation t.
    pub fn of_view() -> Frustum {
        let v = view();
        let r = v.root;
        let o = [ivec::to_real(ivec::X(v.origin)), ivec::to_real(ivec::Y(v.origin)), ivec::to_real(ivec::Z(v.origin))];
        let mut planes = [0.0; 24];
        for (k, p) in v.planes.as_chunks::<4>().0.iter().enumerate() {
            let n = [
                r[0] * p[0] + r[1] * p[1] + r[2] * p[2],
                r[4] * p[0] + r[5] * p[1] + r[6] * p[2],
                r[8] * p[0] + r[9] * p[1] + r[10] * p[2],
            ];
            let d = p[3] + p[0] * r[12] + p[1] * r[13] + p[2] * r[14] - (n[0] * o[0] + n[1] * o[1] + n[2] * o[2]);
            planes[k * 4..k * 4 + 4].copy_from_slice(&[n[0], n[1], n[2], d]);
        }
        Frustum { planes, planet: globe().radius_u() }
    }

    /// Whether any of a sphere at `centre` of `radius` can be on screen from
    /// `eye`. The planet curves ground `d` away down by about `d² / 2R`, which
    /// can bring ground under the frustum's upper plane into it, so the
    /// sphere is widened by that much.
    fn sees(&self, centre: [f64; 3], radius: f64, eye: [f64; 3]) -> bool {
        let d2 = (0..3).map(|k| (centre[k] - eye[k]).powi(2)).sum::<f64>();
        let r = radius + d2 / (2.0 * self.planet);
        self.planes.as_chunks::<4>().0.iter().all(|p| p[0] * centre[0] + p[1] * centre[1] + p[2] * centre[2] + p[3] >= -r)
    }
}

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
        ivec::to_real(ivec::X(v.origin)) + v.eye_local[0],
        ivec::to_real(ivec::Y(v.origin)) + v.eye_local[1],
        ivec::to_real(ivec::Z(v.origin)) + v.eye_local[2],
    ];
    let foot = ivec::gather(at[0].floor() as i64, at[1].floor() as i64, 0);
    Some(Eye { at, foot })
}

/// The level the rule wants ground `dist` from the eye at: the one whose
/// split distance it is past and the next one's it is not. Finer than z16
/// too — negative, z17 and on — where the eye is nearer than z16's split
/// distance: the rule has no floor, and a layer whose data ends coarser
/// draws its finest there.
pub fn level_for(dist: f64) -> i32 {
    (dist / split_distance(0)).log2().floor() as i32
}

/// The levels a view holds, finest first, for an eye `height` over the
/// ground under it: from the level that ground wants, WINDOW_LEVELS up at
/// most. The pages begin at z16, where terrain's data does.
pub fn window_for(height: f64) -> std::ops::Range<u32> {
    let base = level_for(height).clamp(0, TOP_LEVEL as i32) as u32;
    base..(base + WINDOW_LEVELS).min(TOP_LEVEL + 1)
}

/// `window_for` the eye as it stands now; empty before there is a map.
pub fn window() -> std::ops::Range<u32> {
    eye().map_or(0..0, |_| window_for(eye_height()))
}

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
    /// What the view sees, where it is known: ground outside it is held at
    /// the window's top and never refined.
    frustum: Option<Frustum>,
    /// The top level's chunks within the reach (`roots`), found once.
    roots: Vec<Chunk>,
}

/// The domain of an eye `height` over the ground under it, seeing
/// `frustum`, or everything around it where that is none.
pub fn domain_for(eye: &Eye, height: f64, frustum: Option<Frustum>) -> Domain {
    let window = window_for(height);
    let far = reach_at(height).min(split_distance(window.end as i32));
    let mut d = Domain {
        window,
        eye: eye.at,
        foot: eye.foot,
        ground: eye.at[2] - height,
        far,
        chain: Vec::new(),
        frustum,
        roots: Vec::new(),
    };
    d.roots = d.find_roots();
    let mut level: Vec<LevelPage> = d.roots.iter().map(|c| c.page).collect();
    while level.first().is_some_and(|p| p.level < TOP_LEVEL) {
        level = level.iter().map(|p| p.parent()).collect();
        level.sort_unstable();
        level.dedup();
        d.chain.extend(&level);
    }
    d
}

/// Moves whenever what a domain measures heights from does — a terrain
/// page's relief came or went, an object was written — so a domain made
/// before is known to be stale whichever layer made it: POI's pages carry no
/// heights of their own and read the terrain's and the objects'.
pub fn heights_revision() -> (u32, u32) {
    (relief_revision(), objects_revision())
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
    /// objects standing on it. `heights_revision` says when any answer this
    /// gives may have changed.
    fn heights(page: LevelPage, (x0, y0, side): (f64, f64, f64)) -> Option<(f64, f64)> {
        let mut z = range_over(page);
        for b in column_boxes() {
            if b.x0 < x0 + side && x0 < b.x0 + b.side && b.y0 < y0 + side && y0 < b.y0 + b.side {
                z = Some(z.map_or(b.z, |(lo, hi)| (lo.min(b.z.0), hi.max(b.z.1))));
            }
        }
        z
    }

    /// Whether the view can see any of a box: `x0, y0, side` on the ground
    /// and the heights it spans, the ground under the eye's where none is
    /// known.
    fn in_view(&self, (x0, y0, side): (f64, f64, f64), z: Option<(f64, f64)>) -> bool {
        let Some(f) = self.frustum else { return true };
        let (zlo, zhi) = z.unwrap_or((self.ground, self.ground));
        let centre = [x0 + side / 2.0, y0 + side / 2.0, (zlo + zhi) / 2.0];
        let radius = (2.0 * side * side + (zhi - zlo).powi(2)).sqrt() / 2.0;
        f.sees(centre, radius, self.eye)
    }

    /// Whether the view can see any of `c`'s ground: everything where the
    /// frustum is not known.
    pub fn sees(&self, c: Chunk) -> bool {
        let rect = c.rect();
        self.in_view(rect, Self::heights(c.page, rect))
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
    pub fn roots(&self) -> &[Chunk] {
        &self.roots
    }

    fn find_roots(&self) -> Vec<Chunk> {
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
        self.roots.iter().map(|c| c.rect()).collect()
    }

    /// Whether the rule wants `c`'s ground finer than `c`: it comes nearer the
    /// eye than its level's split distance, the window holds a level under
    /// it, and the view sees it. Ground the view does not see is held at the
    /// level it is reached at and refined only once it is seen, as Cesium
    /// refines only what is in the frustum.
    pub fn splits(&self, c: Chunk) -> bool {
        if c.level() <= self.window.start {
            return false;
        }
        let rect = c.rect();
        let z = Self::heights(c.page, rect);
        self.nearest(rect, z) < split_distance(c.level() as i32) && self.in_view(rect, z)
    }

    /// Whether some of `p`'s ground is drawn at `p`'s level: it lies under a
    /// parent the rule splits, which the view sees, or is a root. A held page this says no of is
    /// margin against a move that has not happened.
    pub fn reaches(&self, p: LevelPage) -> bool {
        let top = self.window.end - 1;
        let within = if p.level == top { self.far } else { split_distance(p.level as i32 + 1) };
        if !self.window.contains(&p.level) {
            return false;
        }
        let span = span_of(p.level);
        let rect = (ivec::X(p.page) as f64 * span, ivec::Y(p.page) as f64 * span, span);
        let z = Self::heights(p, rect);
        self.nearest(rect, z) < within && (p.level == top || self.in_view(rect, z))
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
