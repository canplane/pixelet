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
//! The map center is only the handle the camera is moved by and plays no
//! part in it: the eye is the one center of everything a view decides.
//!
//! What a view holds is a budget, not a count of levels: the chunks and the
//! pages the rule wants are counted before anything is asked for, and where
//! they are more than the device holds the criterion is loosened for the
//! whole view at once (`Domain::scale`), so what is wanted always fits and
//! the view coarsens evenly rather than losing ground.

use std::cmp::Ordering;
use std::collections::{BTreeMap, HashMap};
use std::f64::consts::{PI, SQRT_2};

use pxl_render::look::lod::REFERENCE_THRESHOLD_0;
use pxl_spec::space::hierarchy::Scale;
use pxl_spec::space::pvf3::ivec::{self, IVec};
use pxl_spec::space::pvf3::pvf3::Pvf3;
use pxl_spec::space::pyramid::{chunk_span_of, LevelPage, CHUNK_CELLS, GROUND, PAGE_CELLS, TOP_LEVEL};

use crate::global::Global;
use crate::kernel::kernel;
use crate::layer::environment::fog::{eye_height, reach_at};
use crate::layer::environment::state::globe;
use crate::layer::object::store::objects_revision;
use crate::layer::terrain::relief::ground_under;
use crate::shell::map_store::map_store;
use crate::world::chunk::build::MAX_CHUNKS;
use crate::world::frame::view::view;
use crate::world::pyramid::Layer as _;
use pxl_spec::bake::format::Terrain;

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

/// The chunks one level draws over flat ground: every chunk with ground
/// inside that reach, a disc widened by a chunk's diagonal. What z16's own
/// tier is sized for in the mesh arena (world/chunk/mesh_slot.rs). It is not
/// a bound the rule keeps by itself: a chunk is a cube, and ground steep
/// enough to cross a chunk's floor or ceiling is two of them a column, so a
/// mountain wants more. The view is held to it instead — the seen z16 chunks
/// are counted with the budget, and where they are more the criterion is
/// loosened (`domain_for`) — so z16's pool is fixed and never waited on.
pub const LEVEL_CHUNKS: u32 = {
    let r = LEVEL_REACH_CELLS / CHUNK_CELLS as f64 + SQRT_2;
    (PI * r * r) as u32 + 1
};

/// The most chunks a view draws at once. A quarter of the chunk table
/// (world/chunk/build.rs `MAX_CHUNKS`), which holds the view, the view it is
/// replacing — drawn until the new one's chunks are built (world/chunk/
/// table.rs `retire`) — and room past both; the mesh arena is sized from it
/// (world/chunk/mesh_slot.rs).
pub const VIEW_CHUNKS: u32 = MAX_CHUNKS as u32 / 4;

/// The most pages a view holds at once, one record each (world/pyramid.rs
/// `RECORDS`), at 514 KiB a page. A product number, said aloud (2026-09-26):
/// a view wanted at most 41 over flat ground from 2u to 250,000u up, with and
/// without a frustum, and this is that with room. It is not a bound the rule
/// keeps by itself — the pages a level's reach can touch, 12 to 16 a level
/// over eleven levels, come to more than the old 165 — but one the view is
/// held to: where its wants are more, the criterion is loosened until they
/// fit (`domain_for`), and the pyramid's top alone is nine pages at most.
/// Records past what a view wants hold the pages it has left, which a pan
/// back finds without fetching again.
pub const VIEW_PAGES: u32 = 48;

// The top's reach is its split distance, a disc 2·SPLIT of its cells across,
// and a disc touches at most the pages its bounding square spans.
const _: () = {
    let across = (2 * SPLIT as u32).div_ceil(PAGE_CELLS) + 1;
    assert!(VIEW_PAGES >= across * across, "a view's page budget below what the pyramid's top alone can want");
};

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

    /// Whether any of a sphere at `center` of `radius` can be on screen from
    /// `eye`. The planet curves ground `d` away down by about `d² / 2R`, which
    /// can bring ground under the frustum's upper plane into it, so the
    /// sphere is widened by that much.
    fn sees(&self, center: [f64; 3], radius: f64, eye: [f64; 3]) -> bool {
        let d2 = (0..3).map(|k| (center[k] - eye[k]).powi(2)).sum::<f64>();
        let r = radius + d2 / (2.0 * self.planet);
        self.planes.as_chunks::<4>().0.iter().all(|p| p[0] * center[0] + p[1] * center[1] + p[2] * center[2] + p[3] >= -r)
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
    map_store().map_center.as_ref()?;
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

/// Where a piece of the view falls in its one order: what the view sees
/// before what it does not, and within each the nearer the eye first — the
/// shells the rule draws its levels in, taken from the inside out. Every
/// layer's work is taken in it: a page fetched, a chunk mounted and built,
/// and, the other way round, a record or a slot given up.
#[derive(Clone, Copy, Debug)]
pub struct Order {
    pub seen: bool,
    /// How near the eye its box comes, in units.
    pub near: f64,
}

impl Order {
    /// Nothing is ordered after this: what no view measured.
    pub const LAST: Order = Order { seen: false, near: f64::INFINITY };
}

impl Ord for Order {
    fn cmp(&self, other: &Order) -> Ordering {
        other.seen.cmp(&self.seen).then(self.near.total_cmp(&other.near))
    }
}

impl PartialOrd for Order {
    fn partial_cmp(&self, other: &Order) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl PartialEq for Order {
    fn eq(&self, other: &Order) -> bool {
        self.cmp(other).is_eq()
    }
}

impl Eq for Order {}

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
    /// What the criterion is loosened by for this view: the byte budget's
    /// pressure, an octave a level (world/pyramid/pressure.rs), and a
    /// quarter of an octave more a step until the rule's wants fit the
    /// counts (`VIEW_CHUNKS`, `VIEW_PAGES`). Every split distance is
    /// multiplied by it, so the view is coarser everywhere alike.
    scale: f64,
    /// The top level's chunks within the reach (`roots`), found once.
    roots: Vec<Chunk>,
    /// Every page the view wants, and where the ground drawn at its level
    /// falls in the view's order, none where none is: the pages the rule draws from,
    /// and every ancestor of those up to the pyramid's top, so ground whose
    /// page has not come always has something coarser held over it (Google
    /// Earth holds its root, Cesium preloads ancestors).
    pages: BTreeMap<LevelPage, Option<Order>>,
    /// How near the eye each chunk the rule walked comes and whether the view
    /// sees it — how near chooses its level, and the two are its place in
    /// the order — measured once while the budget was fitted: the cover
    /// walks the same chunks, and a measure reads every placed column.
    measured: HashMap<Chunk, Order>,
    /// The chunks under each chunk the rule split (`children`), found once
    /// while the budget was fitted, as the measures are.
    kids: HashMap<Chunk, Vec<Chunk>>,
    /// The chunks the rule draws under this view's criterion.
    chunks: u32,
}

/// The domain of an eye `height` over the ground under it, seeing
/// `frustum`, or everything around it where that is none, under the byte
/// budget's `pressure`.
pub fn domain_for(eye: &Eye, height: f64, frustum: Option<Frustum>, pressure: u32) -> Domain {
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
        kids: HashMap::new(),
        chunks: 0,
    };
    d.roots = d.find_roots();
    // The rule's wants at the criterion, and a quarter of an octave looser
    // each time they do not fit. The measures do not depend on the
    // criterion, so each piece of ground is measured once however many
    // steps it takes; and past the top's split distance nothing splits, the
    // roots alone, which fit.
    let (mut measured, mut kids) = (HashMap::new(), HashMap::new());
    let mut at = |d: &mut Domain, step: i32| {
        d.scale = (-(pressure as f64) - step as f64 / SCALE_STEPS as f64).exp2();
        let z16;
        (d.chunks, z16, d.pages) = d.select(&mut measured, &mut kids);
        d.chunks <= VIEW_CHUNKS && z16 <= LEVEL_CHUNKS && d.pages.len() as u32 <= VIEW_PAGES
    };
    let last = *STEP.get(|| 0);
    let fits = (0..MAX_STEP).find(|&step| at(&mut d, step)).unwrap_or(MAX_STEP);
    // A view loosens its criterion the moment its wants do not fit, but
    // tightens it only as far as the step one finer would fit too. The
    // wants move with the heights — a page landing widens the ground it
    // draws, and more of it splits — so a view on the edge of its budget
    // would otherwise step a quarter of an octave back and forth as pages
    // land, and every chunk near its split distance with it. The margin is
    // the step itself: one step is what the budget flips across.
    let step = if fits < last { (fits + 1).min(last) } else { fits };
    if step != fits {
        at(&mut d, step);
    }
    *STEP.get(|| 0) = step;
    (d.measured, d.kids) = (measured, kids);
    d
}

/// The last step past which the budget's criterion stops being loosened:
/// the pyramid's top's split distance, past which nothing splits and the
/// roots alone are wanted, which fit.
const MAX_STEP: i32 = SCALE_STEPS * (TOP_LEVEL as i32 + 1);

/// Every column's heights as `Domain::heights` read them, and the revision
/// they were read at.
type HeightsCache = (Option<(u32, u32)>, HashMap<(u32, (Scale, [i64; 2])), Spans>);

/// What a column holds, as heights in units: its ground's surface, and the
/// span from its lowest object to its highest.
type Spans = (Option<(f64, f64)>, Option<(f64, f64)>);
static HEIGHTS: Global<HeightsCache> = Global::new();

/// The step the last view's criterion was loosened by (`domain_for`). One
/// for every layer: each makes its domain from the same eye and the same
/// heights, and they have to agree on the ground they draw.
static STEP: Global<i32> = Global::new();

/// Moves whenever what a domain measures heights from does — a terrain
/// page came or went, and its relief with it, or an object was written — so
/// a domain made before is known to be stale whichever layer made it: POI's
/// pages carry no heights of their own and read the terrain's and the
/// objects'.
pub fn heights_revision() -> (u32, u32) {
    (Terrain::pyramid().revision(), objects_revision())
}

impl Domain {
    /// The nearest distance from the eye to a box: `x0, y0, side` on the
    /// ground and the heights it spans, or the ground under the eye's where
    /// none is known.
    fn nearest(&self, (x0, y0, side): (f64, f64, f64), z: Option<(f64, f64)>) -> f64 {
        let [ex, ey, ez] = self.eye;
        let gap = |e: f64, lo: f64, hi: f64| (lo - e).max(e - hi).max(0.0);
        let (zlo, zhi) = z.unwrap_or((self.ground, self.ground));
        gap(ex, x0, x0 + side).hypot(gap(ey, y0, y0 + side)).hypot(gap(ez, zlo, zhi))
    }

    /// The heights anything in a column of `page`'s ground spans: its
    /// terrain's surface under the column, and the objects standing in it.
    /// This is where a node of the view, which is 3D, is pointed at what is
    /// stored, which is 2D: the terrain's heightfield and the objects'
    /// columns, each read by the column (`Chunk::column`). `heights_revision`
    /// says when any answer this gives may have changed.
    ///
    /// Read once for each column until the revision moves: a view is made
    /// again every frame the eye moves, and every chunk it walks asks, eight
    /// children at a time over every step of the budget's criterion, while
    /// the answer only moves with the heights. Asked afresh, it was most of
    /// a zoom's turn (measured 2026-09-26).
    pub fn heights(page: LevelPage, column: (Scale, [i64; 2])) -> Option<(f64, f64)> {
        match Self::spans(page, column) {
            (Some(g), Some(o)) => Some((g.0.min(o.0), g.1.max(o.1))),
            (g, o) => g.or(o),
        }
    }

    /// The column's ground and objects apart (`Spans`), read once a
    /// revision as `heights` says.
    fn spans(page: LevelPage, column: (Scale, [i64; 2])) -> Spans {
        let revision = heights_revision();
        let cache = HEIGHTS.get(Default::default);
        if cache.0 != Some(revision) {
            *cache = (Some(revision), HashMap::new());
        }
        *cache.1.entry((page.level, column)).or_insert_with(|| (ground_under(page, column), kernel().objects.reach_over(column)))
    }

    /// What of `c` holds anything: the heights its ground's surface and the
    /// objects in it span, cut to its own, or none where both pass it by. A
    /// chunk that holds nothing is no node of the view — never split, counted,
    /// fetched for or drawn — and there is no slot for it anywhere: its
    /// emptiness is this answer and not a thing kept. The objects are asked
    /// by the bands they are in (`Store::holds`), not by their span, so a
    /// chunk between two things stacked apart holds nothing. Where nothing
    /// over its column is held yet, the ground under the eye stands in for the
    /// surface, as it does for the distance (`nearest`).
    pub fn occupied(&self, c: Chunk) -> Option<(f64, f64)> {
        let (z0, z1) = c.z_range();
        let cut = |(lo, hi): (f64, f64)| (lo < z1 && hi >= z0).then(|| (lo.max(z0), hi.min(z1)));
        let (ground, objects) = match Self::spans(c.page(), c.column()) {
            (None, None) => (Some((self.ground, self.ground)), None),
            spans => spans,
        };
        let ground = ground.and_then(cut);
        let objects = objects.and_then(cut).filter(|_| kernel().objects.holds(c.column(), (z0, z1)));
        match (ground, objects) {
            (Some(g), Some(o)) => Some((g.0.min(o.0), g.1.max(o.1))),
            (g, o) => g.or(o),
        }
    }

    /// Whether the view can see any of a box: `x0, y0, side` on the ground
    /// and the heights it spans, the ground under the eye's where none is
    /// known.
    fn in_view(&self, (x0, y0, side): (f64, f64, f64), z: Option<(f64, f64)>) -> bool {
        let Some(f) = self.frustum else { return true };
        let (zlo, zhi) = z.unwrap_or((self.ground, self.ground));
        let center = [x0 + side / 2.0, y0 + side / 2.0, (zlo + zhi) / 2.0];
        let radius = (2.0 * side * side + (zhi - zlo).powi(2)).sqrt() / 2.0;
        f.sees(center, radius, self.eye)
    }

    /// How near the eye `c` comes, and whether the view sees any of it: its
    /// footprint, and of its heights the ones that hold something.
    fn measure(&self, c: Chunk) -> Order {
        if let Some(&m) = self.measured.get(&c) {
            return m;
        }
        self.box_order(c.rect(), Some(self.occupied(c).unwrap_or(c.z_range())))
    }

    /// Where a box falls in the view's order: `x0, y0, side` on the ground,
    /// and the heights it spans.
    fn box_order(&self, rect: (f64, f64, f64), z: Option<(f64, f64)>) -> Order {
        Order {
            seen: self.in_view(rect, z),
            near: self.nearest(rect, z),
        }
    }

    /// Whether the view can see any of `c`'s ground: everything where the
    /// frustum is not known.
    pub fn sees(&self, c: Chunk) -> bool {
        self.measure(c).seen
    }

    /// How near the eye `c` comes.
    pub fn nearest_to(&self, c: Chunk) -> f64 {
        self.measure(c).near
    }

    /// Where `c` falls in the view's order (`Order`).
    pub fn order_of(&self, c: Chunk) -> Order {
        self.measure(c)
    }

    /// Where `p` falls in the view's order, by its whole ground: for a page
    /// held only for finer ground to fall back on, and for one weighed
    /// against another for a record.
    pub fn page_order(&self, p: LevelPage) -> Order {
        self.box_order(p.rect(), Self::heights(p, p.node()))
    }

    /// The largest drawn cell per unit of distance under this view's
    /// criterion: past a level's split distance its cells are drawn
    /// averaged 2 × 2, so a cell there is twice the level's over the
    /// distance, and half that just before the next split. For whoever has
    /// to tolerate a drawn cell's height — the badge occlusion test.
    pub fn cell_per_distance(&self) -> f64 {
        2.0 / (split_distance(0) * self.scale)
    }

    /// The level the rule wants `c`'s nearest ground at, under this view's
    /// criterion (`level_for`).
    pub fn wanted_level(&self, c: Chunk) -> i32 {
        level_for(self.nearest_to(c) / self.scale)
    }

    /// The chunks the rule draws under this view's criterion, with every
    /// page resident: what the ledger sets the cover against.
    pub fn chunks(&self) -> u32 {
        self.chunks
    }

    /// Where the partition starts: every chunk of the pyramid's top within
    /// the reach that holds anything.
    pub fn roots(&self) -> &[Chunk] {
        &self.roots
    }

    /// Every column of the top within the reach, and in each the chunks its
    /// contents span.
    fn find_roots(&self) -> Vec<Chunk> {
        let n = (self.far / chunk_span_of(TOP_LEVEL)).ceil() as i64 + 1;
        let foot = Pvf3 { i: self.foot, ..Pvf3::default() };
        let mut out = Vec::new();
        for dy in -n..=n {
            for dx in -n..=n {
                let column = Chunk::of(TOP_LEVEL, &foot).beside(dx, dy, 0);
                out.extend(self.spanned(column, i64::MIN, i64::MAX).filter(|&c| self.nearest_to(c) < self.far));
            }
        }
        out
    }

    /// The chunks of `column`'s column its contents span, from the lowest to
    /// the highest, those between `from` and `to` in its level's chunks: a
    /// column's contents are one span of heights, so the chunks between the
    /// two it ends in hold something too.
    fn spanned(&self, column: Chunk, from: i64, to: i64) -> impl Iterator<Item = Chunk> + '_ {
        let (lo, hi) = Self::heights(column.page(), column.column()).unwrap_or((self.ground, self.ground));
        let span = chunk_span_of(column.level());
        let (base, k0, k1) = (column.z_range().0, (lo / span).floor() as i64, (hi / span).floor() as i64);
        let base = (base / span).round() as i64;
        (k0.max(from)..=k1.min(to)).map(move |k| column.beside(0, 0, k - base)).filter(|&c| self.occupied(c).is_some())
    }

    /// The chunks under `c` that hold anything. Each of the four columns
    /// under it reads its own heights, and where ground finer than `c`'s
    /// reaches past `c`'s top or its floor — a peak the coarser level
    /// averaged under its ceiling — the chunks it reaches into are `c`'s up
    /// to the next chunk of `c`'s column that holds anything, since nothing
    /// else would ever split into them and the peak would stand cut. So every
    /// chunk of a finer level is under exactly one chunk of the level over
    /// it: the one it lies in, or the nearest under or over it in its column.
    /// Objects are found by their bands at every level alike and never need
    /// it; ground, averaged coarser, does.
    pub fn children(&self, c: Chunk) -> Vec<Chunk> {
        match self.kids.get(&c) {
            Some(kids) => kids.clone(),
            None => self.children_of(c).collect(),
        }
    }

    fn children_of(&self, c: Chunk) -> impl Iterator<Item = Chunk> + '_ {
        let z = c.node().1[2];
        // The chunks of `c`'s column that hold anything lie within its
        // contents' span at `c`'s level; the nearest over and under `c` bound
        // what `c` takes of the finer level.
        let (lo, hi) = Self::heights(c.page(), c.column()).unwrap_or((self.ground, self.ground));
        let span = chunk_span_of(c.level());
        let (k0, k1) = ((lo / span).floor() as i64, (hi / span).floor() as i64);
        let held = |k: i64| self.occupied(c.beside(0, 0, k - z)).is_some();
        let over = (z + 1..=k1).find(|&k| held(k));
        let under = (k0..z).rev().find(|&k| held(k));
        let (from, to) = (under.map_or(i64::MIN, |k| 2 * k + 2), over.map_or(i64::MAX, |k| 2 * k - 1));
        c.children().into_iter().filter(move |k| k.node().1[2] == 2 * z).flat_map(move |column| self.spanned(column, from, to))
    }

    /// The chunks the rule draws under this view's criterion, counted, and
    /// the pages that takes (`pages`), each chunk's measure kept in
    /// `measured` for the next count.
    fn select(&self, measured: &mut HashMap<Chunk, Order>, kids: &mut HashMap<Chunk, Vec<Chunk>>) -> (u32, u32, BTreeMap<LevelPage, Option<Order>>) {
        let (mut chunks, mut z16) = (0, 0);
        let mut pages = BTreeMap::new();
        let mut open = self.roots.clone();
        while let Some(c) = open.pop() {
            let m = *measured.entry(c).or_insert_with(|| self.measure(c));
            if self.split_at(c, m.near, m.seen) {
                open.extend(kids.entry(c).or_insert_with(|| self.children_of(c).collect()).iter().copied());
                continue;
            }
            chunks += 1;
            z16 += u32::from(c.level() == 0 && m.seen);
            let first = pages.entry(c.page()).or_insert(Some(m));
            if first.is_some_and(|o| m.cmp(&o).is_lt()) {
                *first = Some(m);
            }
        }
        let drawn: Vec<LevelPage> = pages.keys().copied().collect();
        for p in drawn {
            let mut up = p;
            while up.level < TOP_LEVEL {
                up = up.parent();
                if pages.contains_key(&up) {
                    break;
                }
                pages.insert(up, None);
            }
        }
        (chunks, z16, pages)
    }

    /// Every page the view wants, and where the ground drawn at its level
    /// falls in the view's order (`order_of`): the first of its chunks. None
    /// for a page held only over ground drawn finer, which is what that
    /// ground falls back to and not ground of its own.
    pub fn pages(&self) -> impl Iterator<Item = (LevelPage, Option<Order>)> + '_ {
        self.pages.iter().map(|(&p, &order)| (p, order))
    }

    /// Whether the view wants `p` held.
    pub fn wants(&self, p: LevelPage) -> bool {
        self.pages.contains_key(&p)
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
        let m = self.measure(c);
        self.split_at(c, m.near, m.seen)
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
