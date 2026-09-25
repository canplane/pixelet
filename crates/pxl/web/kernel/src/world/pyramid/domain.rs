// pxl-kernel/world/pyramid/domain.rs

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
//! and wherever the eye is. The rule starts at the pyramid's top and stops
//! only at terrain's atom, so a hill beside the eye is drawn as finely as
//! the ground under it and the far ground runs as far as the air lets it.
//! The map centre is only the handle the camera is moved by and plays no
//! part in it.
//!
//! What a view holds is a budget, not a count of levels: the chunks and the
//! pages the rule wants are counted before anything is asked for, and where
//! they are more than the device holds the criterion is loosened for the
//! whole view at once (`Domain::scale`), so what is wanted always fits and
//! the view coarsens evenly rather than losing ground.

use std::collections::{BTreeMap, HashMap};
use std::f64::consts::{PI, SQRT_2};

use pxl_render::look::lod::REFERENCE_THRESHOLD_0;
use pxl_spec::space::hierarchy::{share, Scale};
use pxl_spec::space::indexer::{L2, L3_IDX};
use pxl_spec::space::pvf3::ivec::{self, IVec};
use pxl_spec::space::pvf3::pvf3::Pvf3;
use pxl_spec::space::pyramid::{chunk_span_of, span_of, LevelPage, CHUNK_CELLS, GROUND, TOP_LEVEL};
use pxl_spec::space::xy::TO_XY;

use crate::layer::environment::fog::{eye_height, reach_at};
use crate::layer::environment::state::globe;
use crate::layer::object::store::{column_boxes, objects_revision};
use crate::layer::terrain::relief::{range_over, relief_revision};
use crate::shell::map_store::map_store;
use crate::world::chunk::node::MAX_CHUNKS;
use crate::world::frame::view::view;

use super::chunk::Chunk;

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
/// reach, a disc widened by a chunk's diagonal. What z16's own tier is sized
/// for in the mesh arena (world/chunk/mesh_slot.rs), derived rather than
/// set; a loosened criterion only draws fewer.
pub const LEVEL_CHUNKS: u32 = {
    let r = LEVEL_REACH_CELLS / CHUNK_CELLS as f64 + SQRT_2;
    (PI * r * r) as u32 + 1
};

/// The most chunks a view draws at once. A quarter of the chunk table
/// (world/chunk/node.rs `MAX_CHUNKS`): where nothing over a piece of ground
/// is held, the pages a level under it stand in, four chunks for its one
/// (layer/terrain/present.rs), and the table holds even that.
pub const VIEW_CHUNKS: u32 = MAX_CHUNKS as u32 / 4;

/// The most pages a view holds at once, one record each (world/pyramid.rs
/// `RECORDS`): 165, what `.fixed.terrain` held when a view was five levels
/// of 5 × 5 rings and the chain over them, kept when those went so that the
/// device's memory did not move with them (2026-09-25). A product number:
/// more buys farther ground at the reference criterion, at 514 KiB a page.
pub const VIEW_PAGES: u32 = 165;

/// Steps a halving of the criterion is loosened in when a view's wants do
/// not fit: a quarter of an octave, so a view over its budget coarsens by
/// about a fifth of a cell's pixels a step and not by double.
const SCALE_STEPS: i32 = 4;

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

/// Where the eye is: its position in units, and the point under it.
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

/// The level the ground under the eye wants, z16 at the finest: what the
/// floating origin is floored to (world/frame/coord.rs `origin_for`), so a
/// rebase is as far on screen at every height.
pub fn ground_level() -> u32 {
    eye().map_or(0, |_| level_for(eye_height()).clamp(0, TOP_LEVEL as i32) as u32)
}

/// The page `anchor` stands in at `level`.
pub fn centre_page(level: u32, anchor: IVec) -> LevelPage {
    LevelPage::over(level, TO_XY(L3_IDX(anchor)))
}

/// How far `page` lies from the anchor's page at its level, in pages along
/// the further axis: 0 for the anchor's own.
pub fn ring_of(anchor: IVec, page: LevelPage) -> u32 {
    let center = centre_page(page.level, anchor);
    let d = ivec::sub(page.page, center.page);
    ivec::X(d).unsigned_abs().max(ivec::Y(d).unsigned_abs()) as u32
}

/// What a view wants drawn, before anything is known of what is resident:
/// every piece of the ground within its reach once, each at the level the
/// rule wants it at, and the pages that takes. Every layer on the pyramid is
/// asked for this same ground at these same levels; what each can draw of
/// it is its own (`Pyramid::cover`).
pub struct Domain {
    eye: [f64; 3],
    foot: IVec,
    /// The height of the ground under the eye: what ground no page over it is
    /// held for is taken to stand at, until one is.
    ground: f64,
    /// How far the ground is drawn: where the air ends it, or past the
    /// pyramid top's reach, whichever is nearer.
    pub far: f64,
    /// What the view sees, where it is known: ground outside it is held at
    /// the pyramid's top and never refined.
    frustum: Option<Frustum>,
    /// What the criterion is loosened by for this view: 1 where the rule's
    /// wants fit the budget as they are, and a quarter of an octave less a
    /// step until they do (`VIEW_CHUNKS`, `VIEW_PAGES`). Every split
    /// distance is multiplied by it, so the view is coarser everywhere alike.
    scale: f64,
    /// The top level's chunks within the reach (`roots`), found once.
    roots: Vec<Chunk>,
    /// Every page the view wants, and whether some of its ground is drawn at
    /// its level: the pages the rule draws from, and every ancestor of those
    /// up to the pyramid's top, so ground whose page has not come always has
    /// something coarser held over it (Google Earth holds its root, Cesium
    /// preloads ancestors).
    pages: BTreeMap<LevelPage, bool>,
    /// How near the eye each chunk the rule walked comes and whether the view
    /// sees it, measured once while the budget was fitted: the cover walks
    /// the same chunks, and a measure reads every placed column.
    measured: HashMap<Chunk, (f64, bool)>,
}

/// The domain of an eye `height` over the ground under it, seeing
/// `frustum`, or everything around it where that is none.
pub fn domain_for(eye: &Eye, height: f64, frustum: Option<Frustum>) -> Domain {
    let mut d = Domain {
        eye: eye.at,
        foot: eye.foot,
        ground: eye.at[2] - height,
        far: reach_at(height).min(split_distance(TOP_LEVEL as i32 + 1)),
        frustum,
        scale: 1.0,
        roots: Vec::new(),
        pages: BTreeMap::new(),
        measured: HashMap::new(),
    };
    d.roots = d.find_roots();
    // The rule's wants at the criterion, and a quarter of an octave looser
    // each time they do not fit. The measures do not depend on the
    // criterion, so each piece of ground is measured once however many
    // steps it takes; and past the top's split distance nothing splits, the
    // roots alone, which fit.
    let mut measured = HashMap::new();
    for step in 0..=SCALE_STEPS * (TOP_LEVEL as i32 + 1) {
        d.scale = (-step as f64 / SCALE_STEPS as f64).exp2();
        let (chunks, pages) = d.select(&mut measured);
        d.pages = pages;
        if chunks <= VIEW_CHUNKS && d.pages.len() as u32 <= VIEW_PAGES {
            break;
        }
    }
    d.measured = measured;
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
    /// The point under the eye.
    pub fn foot(&self) -> IVec {
        self.foot
    }

    /// What the criterion is loosened by for this view (`Domain::scale`).
    pub fn scale(&self) -> f64 {
        self.scale
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
    fn heights(page: LevelPage, node: (Scale, [i64; 2])) -> Option<(f64, f64)> {
        let mut z = range_over(page);
        for b in column_boxes() {
            if share((L2, b.col), node) {
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

    /// How near the eye `c` comes, and whether the view sees any of it.
    fn measure(&self, c: Chunk) -> (f64, bool) {
        if let Some(&m) = self.measured.get(&c) {
            return m;
        }
        let rect = c.rect();
        let z = Self::heights(c.page(), c.node());
        (self.nearest(rect, z), self.in_view(rect, z))
    }

    /// Whether the view can see any of `c`'s ground: everything where the
    /// frustum is not known.
    pub fn sees(&self, c: Chunk) -> bool {
        self.measure(c).1
    }

    /// How near the eye `c` comes.
    pub fn nearest_to(&self, c: Chunk) -> f64 {
        self.measure(c).0
    }

    /// How near the eye `p` comes.
    pub fn page_nearest_to(&self, p: LevelPage) -> f64 {
        let span = span_of(p.level);
        let rect = (ivec::X(p.page) as f64 * span, ivec::Y(p.page) as f64 * span, span);
        self.nearest(rect, Self::heights(p, p.node()))
    }

    /// The level the rule wants `c`'s nearest ground at, under this view's
    /// criterion (`level_for`).
    pub fn wanted_level(&self, c: Chunk) -> i32 {
        level_for(self.nearest_to(c) / self.scale)
    }

    /// Where the partition starts: every chunk of the pyramid's top within
    /// the reach.
    pub fn roots(&self) -> &[Chunk] {
        &self.roots
    }

    /// The ground the view draws: the roots.
    pub fn ground(&self) -> &[Chunk] {
        &self.roots
    }

    fn find_roots(&self) -> Vec<Chunk> {
        let n = (self.far / chunk_span_of(TOP_LEVEL)).ceil() as i64 + 1;
        let foot = Pvf3 { i: self.foot, ..Pvf3::default() };
        let centre = Chunk::of(TOP_LEVEL, &foot);
        let mut out = Vec::new();
        for dy in -n..=n {
            for dx in -n..=n {
                let c = centre.beside(dx, dy);
                if self.nearest_to(c) < self.far {
                    out.push(c);
                }
            }
        }
        out
    }

    /// The chunks the rule draws under this view's criterion, counted, and
    /// the pages that takes (`pages`), each chunk's measure kept in
    /// `measured` for the next count.
    fn select(&self, measured: &mut HashMap<Chunk, (f64, bool)>) -> (u32, BTreeMap<LevelPage, bool>) {
        let mut chunks = 0;
        let mut pages = BTreeMap::new();
        let mut open = self.roots.clone();
        while let Some(c) = open.pop() {
            let (near, seen) = *measured.entry(c).or_insert_with(|| self.measure(c));
            if self.split_at(c, near, seen) {
                open.extend(c.children());
                continue;
            }
            chunks += 1;
            pages.insert(c.page(), true);
        }
        let drawn: Vec<LevelPage> = pages.keys().copied().collect();
        for p in drawn {
            let mut up = p;
            while up.level < TOP_LEVEL {
                up = up.parent();
                if pages.contains_key(&up) {
                    break;
                }
                pages.insert(up, false);
            }
        }
        (chunks, pages)
    }

    /// Every page the view wants, and whether some of its ground is drawn at
    /// its level rather than it being held over ground drawn finer.
    pub fn pages(&self) -> impl Iterator<Item = (LevelPage, bool)> + '_ {
        self.pages.iter().map(|(&p, &drawn)| (p, drawn))
    }

    /// Whether the view wants `p` held.
    pub fn wants(&self, p: LevelPage) -> bool {
        self.pages.contains_key(&p)
    }

    /// Whether some of `p`'s ground is drawn at `p`'s level.
    pub fn reaches(&self, p: LevelPage) -> bool {
        self.pages.get(&p) == Some(&true)
    }

    /// The rule's split for a chunk measured `near` the eye, `seen` or not.
    fn split_at(&self, c: Chunk, near: f64, seen: bool) -> bool {
        c.level() > 0 && near < split_distance(c.level() as i32) * self.scale && seen
    }

    /// Whether the rule wants `c`'s ground finer than `c`: it comes nearer the
    /// eye than its level's split distance under this view's criterion, there
    /// is a level under it, and the view sees it. Ground the view does not
    /// see is held at the level it is reached at and refined only once it is
    /// seen, as Cesium refines only what is in the frustum.
    pub fn splits(&self, c: Chunk) -> bool {
        let (near, seen) = self.measure(c);
        self.split_at(c, near, seen)
    }

    /// Whether `c` is a root the view does not refine: nothing coarser would
    /// draw that ground's far steps.
    pub fn outermost(&self, c: Chunk) -> bool {
        c.level() >= TOP_LEVEL
    }

    /// The chunk over `c`'s ground at the level the view wants that ground
    /// at: `c` itself where the view wants it at `c`'s level or finer.
    pub fn wanted_over(&self, c: Chunk) -> Chunk {
        let chain: Vec<Chunk> = std::iter::once(c).chain(c.ancestors()).collect();
        chain.into_iter().rev().find(|&k| !self.splits(k)).unwrap_or(c)
    }
}

#[cfg(test)]
#[path = "domain.test.rs"]
mod test;
