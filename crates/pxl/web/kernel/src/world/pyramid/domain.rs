// pxl-kernel/world/pyramid/domain.rs

//! What is made to be drawn from an observation: the view's ground cut into
//! pieces, each a chunk at the level the rule wants it at
//! (issue/20260925-what-a-view-costs-is-the-eye-s.md). One rule for every
//! layer on the pyramid and every level of it. It is a materialization —
//! made for the frame from the observation and the heights, and nothing
//! paging rests on: what is fetched is the observation's demand
//! (world/pyramid.rs), which no piece enters.
//!
//! A chunk of level L is split into its four children while the nearest
//! point of its box — its ground and its height, the objects on it included
//! — is nearer the eye than `split_distance(L)` (pxl-spec space/lod.rs),
//! the distance at which one of its cells covers the pixels a cell is
//! allowed. A chunk drawn is one the rule did not split under a parent it
//! did, so every cell drawn covers at most that many pixels and at least a
//! fixed share of it, whatever the tilt and wherever the eye is. The rule
//! starts at the pyramid's top and stops only at terrain's atom, so a hill
//! beside the eye is drawn as finely as the ground under it and the far
//! ground runs as far as the air lets it. The map center is only the handle
//! the camera is moved by and plays no part in it: the eye is the one center
//! of everything a view decides.
//!
//! How many pieces that is follows from the rule (`LEVEL_PIECES`), and no
//! count is fitted to: the heights a piece is measured by are the drawn
//! surface's, which averages, so they choose what is drawn and never what is
//! fetched.

use std::collections::HashMap;
use std::f64::consts::PI;

use pxl_spec::space::hierarchy::Scale;
use pxl_spec::space::lod::{self, split_distance};
use pxl_spec::space::observation::Observation;
use pxl_spec::space::pvf3::ivec::{self, IVec};
use pxl_spec::space::pvf3::pvf3::Pvf3;
use pxl_spec::space::pyramid::{chunk_span_of, LevelPage, TOP_LEVEL};

use crate::global::Global;
use crate::kernel::kernel;
use crate::world::access::Access;
use crate::world::observation::{heights_revision, observation, Order};
use crate::world::paging::LEVELS;

use super::chunk::Chunk;

/// √3, a cube's diagonal over its side.
const SQRT_3: f64 = 1.732_050_807_568_877_2;

/// The most pieces the rule makes of one level about one eye. A piece of a
/// level under the top lies under a parent that split, so whose nearest
/// point came nearer than the parent's split distance — six of the piece's
/// chunks — and every child of that parent lies within the parent's
/// diagonal of that point, 2√3 more; a piece of the top comes nearer than
/// the far edge, which is nearer still. Every chunk coming nearer a point
/// than that lies in the ball a chunk's diagonal wider, so no more of them
/// than its volume holds. It is a ball and not a disc: a chunk is a cube,
/// and ground steep enough to cross a chunk's floor or ceiling, or objects
/// stacked, make several a column, which nothing else bounds.
pub const LEVEL_PIECES: u32 = {
    let reach = (split_distance(1) + SQRT_3 * chunk_span_of(1)) / chunk_span_of(0) + SQRT_3;
    (4.0 / 3.0 * PI * reach * reach * reach) as u32 + 1
};

/// The most pieces a view is made of: every level's.
pub const VIEW_PIECES: u32 = LEVEL_PIECES * LEVELS;

/// What an observation is cut into to be drawn: every piece of the ground
/// within its reach once, each at the level the rule wants it at, before
/// anything is known of what is resident. Every layer on the pyramid draws
/// this same ground at these same levels; what each can draw of it is its
/// own (world/pyramid/cover.rs).
pub struct Domain {
    o: Observation,
    foot: IVec,
    /// The top level's chunks within the reach (`roots`), found once.
    roots: Vec<Chunk>,
    /// How near the eye each chunk the rule walked comes and whether the view
    /// sees it — how near chooses its level, and the two are its place in the
    /// order — measured once as the pieces were cut: whoever orders or draws
    /// a piece asks again, and a measure reads every placed column.
    measured: HashMap<Chunk, Order>,
    /// The chunks the rule draws: the ground cut once.
    pieces: Vec<Chunk>,
}

impl Domain {
    /// The pieces `o` is cut into.
    pub fn of(o: Observation) -> Domain {
        let mut d = Domain {
            o,
            foot: ivec::gather(o.eye[0].floor() as i64, o.eye[1].floor() as i64, 0),
            roots: Vec::new(),
            measured: HashMap::new(),
            pieces: Vec::new(),
        };
        d.roots = d.find_roots();
        let mut open = d.roots.clone();
        while let Some(c) = open.pop() {
            let m = d.measure(c);
            d.measured.insert(c, m);
            if d.split_at(c, m) {
                open.extend(d.children_of(c));
            } else {
                d.pieces.push(c);
            }
        }
        d.pieces.sort_unstable();
        d
    }
}

/// The domain made from the view's observation as it last stood, and which
/// making of it this is.
#[derive(Default)]
struct Made {
    key: Option<(Observation, (u32, u32))>,
    domain: Option<Domain>,
    revision: u32,
}

static MADE: Global<Made> = Global::new();

/// Makes the view's domain again wherever what it is a function of has moved
/// — the observation, and the heights the pieces are measured by — and
/// otherwise leaves it as it is.
pub fn step() {
    let Some(o) = observation() else { return };
    let made = MADE.get(Made::default);
    let key = Some((o, heights_revision()));
    if made.key != key {
        made.key = key;
        made.domain = Some(Domain::of(o));
        made.revision = made.revision.wrapping_add(1);
    }
}

/// The view's domain as last made.
pub fn current() -> Option<&'static Domain> {
    MADE.get(Made::default).domain.as_ref()
}

/// Which making of the view's domain this is: whoever draws it re-reads it
/// when the number moves.
pub fn revision() -> u32 {
    MADE.get(Made::default).revision
}

/// Every column's heights as `Domain::heights` read them, and the revision
/// they were read at.
type HeightsCache = (Option<(u32, u32)>, HashMap<(u32, (Scale, [i64; 2])), Spans>);

/// What a column holds, as heights in units: its ground's bound, and the
/// span from its lowest object to its highest.
type Spans = (Option<(f64, f64)>, Option<(f64, f64)>);
static HEIGHTS: Global<HeightsCache> = Global::new();

impl Domain {
    /// The heights anything in a column of `page`'s ground can span: the
    /// bound the terrain's index lane holds under the column
    /// (world/access.rs `Access::extent`), and the objects standing in it.
    /// This is where a node of the view, which is 3D, is pointed at what is
    /// stored, which is 2D: the terrain's heightfield and the objects'
    /// columns, each read by the column (`Chunk::column`). `heights_revision`
    /// says when any answer this gives may have changed.
    ///
    /// A bound and not the surface drawn, because what it decides is which
    /// pieces of ground a view holds and how near they come — which pages are
    /// asked for — and the surface a page over a piece draws averages what is
    /// under it: a peak it averaged under its ceiling would be no chunk and
    /// no fault until the page under it came, which only a fault brings. A
    /// bound is no nearer than the truth, so the level it chooses errs finer.
    ///
    /// Read once for each column until the revision moves: a view is made
    /// again every frame the eye moves, and every chunk it walks asks, eight
    /// children at a time, while the answer only moves with the heights.
    /// Asked afresh, it was most of a zoom's turn (measured 2026-09-26).
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
        *cache
            .1
            .entry((page.level, column))
            .or_insert_with(|| (Access::now().extent(page, column).value(), kernel().objects.reach_over(column)))
    }

    /// What of `c` can hold anything: the heights its ground's bound and the
    /// objects in it span, cut to its own, or none where both pass it by. A
    /// chunk that holds nothing is no node of the view — never split, counted,
    /// fetched for or drawn — and there is no slot for it anywhere: its
    /// emptiness is this answer and not a thing kept. The objects are asked
    /// by the bands they are in (`Store::holds`), not by their span, so a
    /// chunk between two things stacked apart holds nothing. Where nothing
    /// over its column is held yet, the ground under the eye stands in for the
    /// surface, as it does for the span a column is walked by (`spanned`).
    pub fn occupied(&self, c: Chunk) -> Option<(f64, f64)> {
        let (z0, z1) = c.z_range();
        let cut = |(lo, hi): (f64, f64)| (lo < z1 && hi >= z0).then(|| (lo.max(z0), hi.min(z1)));
        let (ground, objects) = match Self::spans(c.page(), c.column()) {
            (None, None) => (Some((self.o.ground, self.o.ground)), None),
            spans => spans,
        };
        let ground = ground.and_then(cut);
        let objects = objects.and_then(cut).filter(|_| kernel().objects.holds(c.column(), (z0, z1)));
        match (ground, objects) {
            (Some(g), Some(o)) => Some((g.0.min(o.0), g.1.max(o.1))),
            (g, o) => g.or(o),
        }
    }

    /// How near the eye `c` comes, and whether the view sees any of it: its
    /// footprint, and of its heights the ones that hold something.
    fn measure(&self, c: Chunk) -> Order {
        if let Some(&m) = self.measured.get(&c) {
            return m;
        }
        Order::of(&self.o, c.rect(), Some(self.occupied(c).unwrap_or(c.z_range())))
    }

    /// How near the eye `c` comes.
    pub fn nearest_to(&self, c: Chunk) -> f64 {
        self.measure(c).near
    }

    /// Where `c` falls in the view's order (`Order`).
    pub fn order_of(&self, c: Chunk) -> Order {
        self.measure(c)
    }

    /// The level the rule wants `c`'s nearest ground at (pxl-spec
    /// space/lod.rs `level`).
    pub fn wanted_level(&self, c: Chunk) -> i32 {
        lod::level(self.nearest_to(c))
    }

    /// The chunks the rule draws, with every page resident, in their own
    /// order: the ground cut once.
    pub fn pieces(&self) -> &[Chunk] {
        &self.pieces
    }

    /// The observation the pieces were cut from.
    pub fn observation(&self) -> &Observation {
        &self.o
    }

    /// Where the partition starts: every chunk of the pyramid's top within
    /// the reach that holds anything.
    pub fn roots(&self) -> &[Chunk] {
        &self.roots
    }

    /// Every column of the top within the reach, and in each the chunks its
    /// contents span.
    fn find_roots(&self) -> Vec<Chunk> {
        let n = (self.o.far / chunk_span_of(TOP_LEVEL)).ceil() as i64 + 1;
        let foot = Pvf3 { i: self.foot, ..Pvf3::default() };
        let mut out = Vec::new();
        for dy in -n..=n {
            for dx in -n..=n {
                let column = Chunk::of(TOP_LEVEL, &foot).beside(dx, dy, 0);
                out.extend(self.spanned(column, i64::MIN, i64::MAX).filter(|&c| self.nearest_to(c) < self.o.far));
            }
        }
        out
    }

    /// The chunks of `column`'s column its contents span, from the lowest to
    /// the highest, those between `from` and `to` in its level's chunks: a
    /// column's contents are one span of heights, so the chunks between the
    /// two it ends in hold something too.
    fn spanned(&self, column: Chunk, from: i64, to: i64) -> impl Iterator<Item = Chunk> + '_ {
        let (lo, hi) = Self::heights(column.page(), column.column()).unwrap_or((self.o.ground, self.o.ground));
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
    fn children_of(&self, c: Chunk) -> impl Iterator<Item = Chunk> + '_ {
        let z = c.node().1[2];
        // The chunks of `c`'s column that hold anything lie within its
        // contents' span at `c`'s level; the nearest over and under `c` bound
        // what `c` takes of the finer level.
        let (lo, hi) = Self::heights(c.page(), c.column()).unwrap_or((self.o.ground, self.o.ground));
        let span = chunk_span_of(c.level());
        let (k0, k1) = ((lo / span).floor() as i64, (hi / span).floor() as i64);
        let held = |k: i64| self.occupied(c.beside(0, 0, k - z)).is_some();
        let over = (z + 1..=k1).find(|&k| held(k));
        let under = (k0..z).rev().find(|&k| held(k));
        let (from, to) = (under.map_or(i64::MIN, |k| 2 * k + 2), over.map_or(i64::MAX, |k| 2 * k - 1));
        c.children().into_iter().filter(move |k| k.node().1[2] == 2 * z).flat_map(move |column| self.spanned(column, from, to))
    }

    /// The rule's split for a chunk measured as `m`.
    fn split_at(&self, c: Chunk, m: Order) -> bool {
        c.level() > 0 && m.near < split_distance(c.level() as i32) && m.seen
    }

    /// Whether the rule wants `c`'s ground finer than `c`: it comes nearer the
    /// eye than its level's split distance, there is a level under it, and
    /// the view sees it. Ground the view does not see is held at the level it
    /// is reached at and refined only once it is seen, as Cesium refines only
    /// what is in the frustum.
    pub fn splits(&self, c: Chunk) -> bool {
        self.split_at(c, self.measure(c))
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
