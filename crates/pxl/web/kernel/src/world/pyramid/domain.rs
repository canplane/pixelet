// pxl-kernel/world/pyramid/domain.rs

//! What is made to be drawn from an observation: the frontier's goal
//! (world/chunk/frontier.rs), the view's ground cut into pieces, each a
//! chunk of the pyramid. One rule for every layer on the pyramid and every
//! level of it. It is a materialization — made for the frame from the
//! observation and what is resident, and nothing paging rests on: what is
//! fetched is the observation's demand (world/pyramid.rs), which no piece
//! enters and which this never reads.
//!
//! **A coarse cover always stands.** Every chunk of the source's top — the
//! coarsest level it supplies whole, world/paging/floor.rs — within the
//! reach that holds anything is a piece, all around the eye and not only
//! where it looks (`roots`), so every region of the view enters the frontier
//! coarse and only then refines: ground turned to for the first time shows
//! the cover's answer and never a hole. A chunk is a piece only where some
//! page at or over it is resident, which for the top is the floor, leased
//! all around whatever is observed. Where it is not — a cold start, a jump —
//! nothing is known of the ground at all, and that is a state of its own
//! (`unanswered`), not a gap in the frontier: the goal has no piece there,
//! and the frontier is without gaps over everything the floor answers.
//!
//! **A piece refines where its error would leak.** A chunk splits into the
//! chunks under it that hold anything only where it contributes to some
//! cell of the observation — some tile's footprint touches it
//! (world/observer.rs) — and its error, projected, crosses a cell's edge on
//! either axis of the screen (`Lens::error` against `cell_angle`). Its error
//! is what its page already says: its cell's side across, and up as far as
//! its ground can rise across one of its cells and its objects whole
//! (`height_error`), taken at its nearest point to the eye. Inside a cell is
//! don't-care, so nothing refines to keep what stays inside one: ground at a
//! grazing angle projects its depth small and stays coarse along the screen's
//! depth, and refines across it, and at a silhouette, as far as it crosses;
//! a far pole inside a cell is inside. Hidden ground contributes to no cell
//! and is not tested at all. Visibility decides how deep, never whether: a
//! piece exists by the cover, and an observation only ever replaces a piece
//! with the chunks under it, whole, so it can refine what stands and never
//! leave ground without one. The map center plays no part: the eye is the
//! one center of everything a view decides.
//!
//! **What bounds the goal is the rule's reach, not the screen.** A chunk's
//! error on either axis is at most its cell and its own height over its
//! distance, and its height is `CHUNK_CELLS` cells, so a chunk of a level
//! splits only within `split_reach` of the eye, whatever it holds
//! and whatever the screen: a split is `CHILDREN − 1` pieces more at most,
//! and the splits of a level are the touched chunks within its reach. That
//! bound is asserted of every cut. A footprint reaches the horizon, so the
//! screen alone bounds nothing a footprint touches; what the arena cannot
//! hold the quality policy draws coarser (world/quality.rs).
//!
//! **The steps of the quality are cut from one sight, as they are asked.**
//! The footprints do not depend on the quality, and a coarser quality only
//! widens the cell an error is held against, so it splits a subset of the
//! chunks a finer one does: the cut at each step is made from the same
//! footprints, only when the policy asks for it (`cut_at`), and never counts
//! less at a finer step (world/chunk/frontier.rs `need`).

use std::collections::{HashMap, HashSet};

use pxl_spec::bake::format::Terrain;
use pxl_spec::space::hierarchy::Scale;
use pxl_spec::space::lod::{fault_radius, Tau};
use pxl_spec::space::observation::Observation;
use pxl_spec::space::pvf3::core::IVEC_Z_W;
use pxl_spec::space::pvf3::ivec;
use pxl_spec::space::pvf3::pvf3::Pvf3;
use pxl_spec::space::pyramid::{chunk_span_of, LevelPage, CHUNK_CELLS, GROUND, TOP_LEVEL};

use crate::global::Global;
use crate::kernel::kernel;
use crate::world::access::Access;
use crate::world::observation::{heights_revision, observation, Order};
use crate::world::observer::{cell_angle, EdgeRays, Footprint, Lens, Sight};
use crate::world::quality::Effective;

use super::chunk::Chunk;
use super::Layer as _;

/// The chunks of `top` one column's heights can span: a point's height is
/// the 16-bit lane of a position (pxl-spec space/pvf3), half of it either
/// side of zero, so the chunks of `top`'s height the lane crosses.
pub const fn column_chunks(top: u32) -> u32 {
    let half = 1i64 << (IVEC_Z_W - 1);
    let span = chunk_span_of(top) as i64;
    ((half - 1).div_euclid(span) - (-half).div_euclid(span) + 1) as u32
}

/// The most pieces the cover over a source whose top is `top` is: every
/// chunk of the top within the far edge along the ground, which is never
/// past the top's fault radius (pxl-spec space/observation.rs
/// `Observation::far`). An interval of `d` touches at most ⌊d / side⌋ + 2
/// chunks, and a disc no more than the square about it; the radius and a
/// chunk grow by the same factor a level, so the square is as many chunks
/// at every level, and a column of a finer top is more of them.
pub const fn cover_pieces(top: u32) -> u32 {
    let across = (2.0 * fault_radius(top) / chunk_span_of(top)) as u32 + 2;
    across * across * column_chunks(top)
}

/// How near the eye a chunk of `level` has to come for its error to cross a
/// cell at `tau`, whatever it holds: its error on either axis is at most its
/// cell's side and its own height over its distance, and its height is at
/// most `CHUNK_CELLS` of its cells.
pub fn split_reach(level: u32, tau: Tau) -> f64 {
    (1 + CHUNK_CELLS) as f64 * GROUND.scale(level).side() / cell_angle(tau)
}

/// The view cut at one quality: the chunks split, and the pieces that
/// leaves.
pub struct Cut {
    tau: Tau,
    splits: HashSet<Chunk>,
    pieces: Vec<Chunk>,
}

impl Cut {
    /// The quality it is cut at.
    pub fn tau(&self) -> Tau {
        self.tau
    }

    /// The pieces, sorted: an antichain.
    pub fn pieces(&self) -> &[Chunk] {
        &self.pieces
    }

    /// The chunks split on the way to them.
    pub fn splits(&self) -> impl Iterator<Item = Chunk> + '_ {
        self.splits.iter().copied()
    }
}

/// What the footprints say of a chunk, whatever the quality: which of them
/// touch it, and the angle its error spans on the screen's wider axis there,
/// and how near the eye it comes.
struct Seen {
    touching: Vec<u32>,
    error: f64,
    near: f64,
}

/// What an observation is cut into to be drawn: the cover, refined where its
/// error would leak, at each quality of the ladder it is asked at. Every
/// layer on the pyramid draws this same ground at these same levels; what
/// each can draw of it is its own (world/pyramid/cover.rs).
pub struct Domain {
    o: Observation,
    lens: Option<Lens>,
    /// What the screen's tiles touched.
    footprints: Vec<Footprint>,
    /// What is known of the columns' heights: what holds anything, and what
    /// has an answer.
    known: Box<dyn ColumnBounds>,
    /// The cover: every chunk of the top within the reach that holds
    /// anything and has an answer.
    roots: Vec<Chunk>,
    /// Columns of the top within the reach with no answer yet.
    unanswered: u32,
    /// What the footprints say of each chunk asked about so far.
    seen: HashMap<Chunk, Seen>,
    /// How near the eye each chunk cut comes and whether the view sees it —
    /// its place in the order — measured once: whoever orders or draws a
    /// piece asks again, and a measure reads every placed column.
    measured: HashMap<Chunk, Order>,
    /// The qualities it may be cut at, finest first, and the cut at each
    /// made so far.
    ladder: Vec<Tau>,
    cuts: Vec<Option<Cut>>,
    /// Which cut is in force.
    at: usize,
}

impl Domain {
    /// The pieces `o` is cut into at `ladder[at]`, as far as `known` knows
    /// the heights and `footprints` say what the screen touches.
    pub fn of(o: Observation, ladder: &[Tau], at: usize, known: Box<dyn ColumnBounds>, footprints: Vec<Footprint>) -> Domain {
        xpute_core::ensure!(at < ladder.len(), EINVAL);
        let mut d = Domain {
            o,
            lens: Lens::of(&o),
            footprints,
            known,
            roots: Vec::new(),
            unanswered: 0,
            seen: HashMap::new(),
            measured: HashMap::new(),
            ladder: ladder.to_vec(),
            cuts: (0..ladder.len()).map(|_| None).collect(),
            at: at.min(ladder.len() - 1),
        };
        d.find_roots();
        let all: Vec<u32> = (0..d.footprints.len() as u32).collect();
        for c in d.roots.clone() {
            d.look(c, &all);
        }
        d.cut_at(d.at);
        d
    }

    /// What the footprints of `from` say of `c`, noted once.
    fn look(&mut self, c: Chunk, from: &[u32]) {
        if self.seen.contains_key(&c) {
            return;
        }
        let (z0, z1) = self.occupied(c).unwrap_or(c.z_range());
        let (x0, y0, side) = c.rect();
        let touching: Vec<u32> = from.iter().copied().filter(|&k| self.footprints[k as usize].touches((x0, y0, side), (z0, z1))).collect();
        let e = self.o.eye;
        let nearest = [e[0].clamp(x0, x0 + side), e[1].clamp(y0, y0 + side), e[2].clamp(z0, z1)];
        let near = (0..3).map(|k| (nearest[k] - e[k]).powi(2)).sum::<f64>().sqrt();
        let error = self
            .lens
            .map_or(0.0, |l| l.error(nearest, GROUND.scale(c.level()).side(), self.height_error(c)).into_iter().fold(0.0, f64::max));
        self.seen.insert(c, Seen { touching, error, near });
    }

    /// The cut at step `k` of the ladder, made now if it was not: from the
    /// cover down, a chunk some footprint touches whose error spans more
    /// than a cell is split into the chunks under it that hold anything, and
    /// what is left are the pieces. A chunk none of whose children holds
    /// anything is left a piece, so a split never takes a piece away — which
    /// is what keeps a finer quality's count no less. A piece with no answer
    /// is left out.
    pub fn cut_at(&mut self, k: usize) -> &Cut {
        if self.cuts[k].is_none() {
            let tau = self.ladder[k];
            let cell = cell_angle(tau);
            let mut splits: HashSet<Chunk> = HashSet::new();
            let mut pieces = Vec::new();
            let mut open = self.roots.clone();
            while let Some(c) = open.pop() {
                let s = &self.seen[&c];
                let leaks = c.level() > 0 && !s.touching.is_empty() && s.error > cell;
                let under: Vec<Chunk> = if leaks {
                    c.children().into_iter().filter(|&k| self.occupied(k).is_some()).collect()
                } else {
                    Vec::new()
                };
                if under.is_empty() {
                    if self.answered(c) {
                        pieces.push(c);
                    }
                    continue;
                }
                // The rule's reach, whatever the world: past it is the
                // derivation broken, and the quality policy's count with it.
                xpute_core::ensure!(s.near < split_reach(c.level(), tau), ENOTRECOVERABLE, c.level());
                let from = s.touching.clone();
                for &k in &under {
                    self.look(k, &from);
                }
                splits.insert(c);
                open.extend(under);
            }
            pieces.sort_unstable();
            for &c in pieces.iter().chain(&splits) {
                if !self.measured.contains_key(&c) {
                    let m = self.measure(c);
                    self.measured.insert(c, m);
                }
            }
            self.cuts[k] = Some(Cut { tau, splits, pieces });
        }
        self.cuts[k].as_ref().expect("made above")
    }

    /// The cut at step `k` of the ladder, where it has been made.
    pub fn cut(&self, k: usize) -> Option<&Cut> {
        self.cuts.get(k)?.as_ref()
    }

    /// Puts the cut at step `k` of the ladder in force, making it if it was
    /// not. Whether the pieces changed.
    pub fn put(&mut self, k: usize) -> bool {
        xpute_core::ensure!(k < self.ladder.len(), EINVAL);
        let k = k.min(self.ladder.len() - 1);
        self.cut_at(k);
        let moved = self.cuts[k].as_ref().map(|c| &c.pieces) != self.cuts[self.at].as_ref().map(|c| &c.pieces);
        self.at = k;
        moved
    }

    /// The cut in force.
    fn in_force(&self) -> &Cut {
        self.cuts[self.at].as_ref().expect("the cut in force is made when it is put")
    }

    /// The quality the pieces are cut at.
    pub fn tau(&self) -> Tau {
        self.in_force().tau
    }

    /// Columns of the top within the reach no page over is resident for:
    /// ground with no answer yet, which the goal has no piece of.
    pub fn unanswered(&self) -> u32 {
        self.unanswered
    }
}

/// The domain made from the view's observation as it last stood, what its
/// tiles saw, and which making of its pieces this is.
#[derive(Default)]
struct Made {
    key: Option<(Observation, (u32, u32))>,
    domain: Option<Domain>,
    sight: Sight,
    /// The terrain pages resident when the tiles were last cast, and the
    /// objects' revision then: what a residency change is told by.
    resident: HashSet<LevelPage>,
    objects: u32,
    rays: u32,
    revision: u32,
    walks: u32,
}

static MADE: Global<Made> = Global::new();

/// Makes the view's domain again wherever what it is a function of has
/// moved — the observation, and what is resident, which the heights and
/// what the tiles see are read from — cut at the effective quality;
/// otherwise only puts that cut in force. Whether it was made again: every
/// count of its cuts is stale. The quality policy's to call
/// (world/quality.rs), which alone holds an `Effective`.
pub fn step(ladder: &[Tau], q: Effective) -> bool {
    let Some(o) = observation() else { return false };
    let at = ladder.iter().position(|&t| t == q.tau()).unwrap_or(0);
    let made = MADE.get(Made::default);
    let key = Some((o, heights_revision()));
    if made.key != key {
        made.key = key;
        let paging = Terrain::pyramid().paging();
        let resident: HashSet<LevelPage> = (0..=TOP_LEVEL).flat_map(|level| paging.level(level).map(|(p, _)| p)).collect();
        let moved: Vec<(f64, f64, f64)> = resident.symmetric_difference(&made.resident).map(|p| p.rect()).collect();
        let objects = heights_revision().1;
        made.rays = made.sight.observe(&o, &EdgeRays(Access::now()), objects != made.objects, &moved);
        made.resident = resident;
        made.objects = objects;
        made.domain = Some(Domain::of(o, ladder, at, Box::new(Held), made.sight.footprints().cloned().collect()));
        made.revision = made.revision.wrapping_add(1);
        made.walks = made.walks.wrapping_add(1);
        return true;
    }
    if made.domain.as_mut().is_some_and(|d| d.put(at)) {
        made.revision = made.revision.wrapping_add(1);
    }
    false
}

/// The view's domain as last made.
pub fn current() -> Option<&'static Domain> {
    MADE.get(Made::default).domain.as_ref()
}

/// The view's domain as last made, and its cut at step `k` of the ladder,
/// made now if it was not: for the quality policy's count.
pub fn cut_at(k: usize) -> Option<(&'static Domain, &'static Cut)> {
    let d: &'static mut Domain = MADE.get(Made::default).domain.as_mut()?;
    d.cut_at(k);
    let d: &'static Domain = d;
    Some((d, d.cut(k)?))
}

/// Rays the view's tiles cast the last time its domain was made.
pub fn rays() -> u32 {
    MADE.get(Made::default).rays
}

/// Which making of the view's domain this is: whoever counts its cuts counts
/// again when the number moves.
pub fn walks() -> u32 {
    MADE.get(Made::default).walks
}

/// Which making of the view's pieces this is: whoever draws them re-reads
/// them when the number moves.
pub fn revision() -> u32 {
    MADE.get(Made::default).revision
}

/// What a column holds, as heights in units: its ground's bound, and the
/// span from its lowest object to its highest, each none where nothing is
/// known or nothing stands.
pub type Spans = (Option<(f64, f64)>, Option<(f64, f64)>);

/// What the domain knows of a column's heights (the K of the record): a
/// bound, never tighter than the truth, and at a coarser level never
/// tighter than at a finer one under it — the index lane's law, a parent's
/// extent holding its children's (pxl-spec bake/fold.rs `fold_index`). That
/// nesting is what makes every chunk that holds anything lie under one that
/// does, a level up, so the domain's tree is the chunks' own. The ground's
/// bound is known exactly where a page at or over the column is resident,
/// which is what a piece's answer is.
pub trait ColumnBounds {
    /// Column `column` of `page`'s level: its ground's bound and its
    /// objects' span.
    fn spans(&self, page: LevelPage, column: (Scale, [i64; 2])) -> Spans;
    /// Whether anything stands in `column` between heights `z`: the objects
    /// are held by the bands they are in, not by their span, so a chunk
    /// between two things stacked apart holds nothing.
    fn holds(&self, column: (Scale, [i64; 2]), z: (f64, f64)) -> bool;
    /// The steepest column `column` of `page`'s level's ground can be, as a
    /// rise over a run (world/access.rs `Access::slope`): never less than the
    /// truth's.
    fn slope(&self, page: LevelPage, column: (Scale, [i64; 2])) -> Option<f64>;
}

/// What the kernel knows: the terrain's index lanes, from each page or the
/// nearest resident page over it (world/access.rs `Access::extent`), and the
/// objects standing in the column, which are all resident.
pub struct Held;

impl Held {
    /// What is known of `column` of `page`'s level, read once a revision.
    fn read(page: LevelPage, column: (Scale, [i64; 2])) -> HeldColumn {
        let revision = heights_revision();
        let cache = HEIGHTS.get(Default::default);
        if cache.0 != Some(revision) {
            *cache = (Some(revision), HashMap::new());
        }
        *cache.1.entry((page.level, column)).or_insert_with(|| {
            let access = Access::now();
            HeldColumn {
                spans: (access.extent(page, column).value(), kernel().objects.reach_over(column)),
                slope: access.slope(page, column).value(),
            }
        })
    }
}

impl ColumnBounds for Held {
    fn spans(&self, page: LevelPage, column: (Scale, [i64; 2])) -> Spans {
        Held::read(page, column).spans
    }

    fn holds(&self, column: (Scale, [i64; 2]), z: (f64, f64)) -> bool {
        kernel().objects.holds(column, z)
    }

    fn slope(&self, page: LevelPage, column: (Scale, [i64; 2])) -> Option<f64> {
        Held::read(page, column).slope
    }
}

/// A column's heights as `Held` read them.
#[derive(Clone, Copy)]
struct HeldColumn {
    spans: Spans,
    slope: Option<f64>,
}

/// Every column's heights as `Held` read them, and the revision they were
/// read at.
type HeightsCache = (Option<(u32, u32)>, HashMap<(u32, (Scale, [i64; 2])), HeldColumn>);
static HEIGHTS: Global<HeightsCache> = Global::new();

/// A column's ground and objects as one span.
fn merged(spans: Spans) -> Option<(f64, f64)> {
    match spans {
        (Some(g), Some(o)) => Some((g.0.min(o.0), g.1.max(o.1))),
        (g, o) => g.or(o),
    }
}

impl Domain {
    /// The heights anything in a column of `page`'s ground can span, as the
    /// kernel knows them (`Held`). This is where a node of the view, which
    /// is 3D, is pointed at what is stored, which is 2D: the terrain's
    /// heightfield and the objects' columns, each read by the column
    /// (`Chunk::column`). `heights_revision` says when any answer this gives
    /// may have changed.
    ///
    /// A bound and not the surface drawn, because what it decides is which
    /// chunks hold anything and how near they come, and the surface a page
    /// over a piece draws averages what is under it: a peak it averaged under
    /// its ceiling would be no chunk until the page under it came. A bound is
    /// no nearer than the truth, so the order it gives errs nearer.
    ///
    /// Read once for each column until the revision moves: a view is made
    /// again every frame the eye moves, and every chunk it cuts asks, eight
    /// children at a time, while the answer only moves with the heights.
    /// Asked afresh, it was most of a zoom's turn (measured 2026-09-26).
    pub fn heights(page: LevelPage, column: (Scale, [i64; 2])) -> Option<(f64, f64)> {
        merged(Held.spans(page, column))
    }

    /// What of `c` can hold anything: the heights its ground's bound and the
    /// objects in it span, cut to its own, or none where both pass it by or
    /// nothing is known. A chunk that holds nothing is no node of the view —
    /// never split, counted or drawn — and there is no slot for it anywhere:
    /// its emptiness is this answer and not a thing kept. The objects are
    /// asked by the bands they are in (`Store::holds`), not by their span, so
    /// a chunk between two things stacked apart holds nothing.
    pub fn occupied(&self, c: Chunk) -> Option<(f64, f64)> {
        let (z0, z1) = c.z_range();
        let cut = |(lo, hi): (f64, f64)| (lo < z1 && hi >= z0).then(|| (lo.max(z0), hi.min(z1)));
        let (ground, objects) = self.known.spans(c.page(), c.column());
        let ground = ground.and_then(cut);
        let objects = objects.and_then(cut).filter(|_| self.known.holds(c.column(), (z0, z1)));
        match (ground, objects) {
            (Some(g), Some(o)) => Some((g.0.min(o.0), g.1.max(o.1))),
            (g, o) => g.or(o),
        }
    }

    /// How far up or down what `c` draws can be from what it stands for, in
    /// units, and never more than its own height: its ground as far as it
    /// can rise across one of its cells, and the objects in it whole. A
    /// surface drawn in coarser cells stays the surface, off by what it
    /// rises across a cell; an object thinner than a cell may be folded
    /// away, so it can be off by all of itself, which a far pole is and
    /// what a sheer face is taken to be.
    fn height_error(&self, c: Chunk) -> f64 {
        let (z0, z1) = c.z_range();
        let ground = self.known.slope(c.page(), c.column()).map_or(0.0, |s| s * GROUND.scale(c.level()).side());
        let objects = self
            .known
            .spans(c.page(), c.column())
            .1
            .filter(|_| self.known.holds(c.column(), (z0, z1)))
            .map_or(0.0, |(lo, hi)| hi.min(z1) - lo.max(z0));
        ground.max(objects).clamp(0.0, z1 - z0)
    }

    /// Whether `c`'s ground has an answer: some page at or over it is
    /// resident, which is where its bound is known.
    fn answered(&self, c: Chunk) -> bool {
        self.known.spans(c.page(), c.column()).0.is_some()
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

    /// Whether `c` is of the source's top, which no level over it draws.
    pub fn outermost(&self, c: Chunk) -> bool {
        c.level() >= self.o.top
    }

    /// The pieces of the cut in force, sorted: the ground cut once.
    pub fn pieces(&self) -> &[Chunk] {
        &self.in_force().pieces
    }

    /// The observation the pieces were cut from.
    pub fn observation(&self) -> &Observation {
        &self.o
    }

    /// The cover: every chunk of the source's top within the reach that
    /// holds anything and has an answer.
    pub fn roots(&self) -> &[Chunk] {
        &self.roots
    }

    /// The qualities it may be cut at, finest first.
    pub fn ladder(&self) -> &[Tau] {
        &self.ladder
    }

    /// Every column of the top within the reach along the ground, and in each
    /// with an answer the chunks its contents span that hold anything and
    /// come within the reach; the ones with none, counted.
    fn find_roots(&mut self) {
        let top = self.o.top;
        let n = (self.o.far / chunk_span_of(top)).ceil() as i64 + 1;
        let foot = Pvf3 {
            i: ivec::gather(self.o.eye[0].floor() as i64, self.o.eye[1].floor() as i64, 0),
            ..Pvf3::default()
        };
        let mut roots = Vec::new();
        for dy in -n..=n {
            for dx in -n..=n {
                let column = Chunk::of(top, &foot).beside(dx, dy, 0);
                if Order::of(&self.o, column.rect(), None).near >= self.o.far {
                    continue;
                }
                let spans = self.known.spans(column.page(), column.column());
                let (Some(_), Some(heights)) = (spans.0, merged(spans)) else {
                    self.unanswered += 1;
                    continue;
                };
                roots.extend(self.spanned(column, heights).filter(|&c| self.nearest_to(c) < self.o.far));
            }
        }
        xpute_core::ensure!(roots.len() as u32 <= cover_pieces(top), ENOTRECOVERABLE, roots.len());
        for &c in &roots {
            let m = self.measure(c);
            self.measured.insert(c, m);
        }
        self.roots = roots;
    }

    /// The chunks of `column`'s column its contents reach, from the lowest,
    /// `lo`, to the highest, `hi`, that hold anything.
    fn spanned(&self, column: Chunk, (lo, hi): (f64, f64)) -> impl Iterator<Item = Chunk> + '_ {
        let span = chunk_span_of(column.level());
        let (base, k0, k1) = (column.z_range().0, (lo / span).floor() as i64, (hi / span).floor() as i64);
        let base = (base / span).round() as i64;
        (k0..=k1).map(move |k| column.beside(0, 0, k - base)).filter(|&c| self.occupied(c).is_some())
    }

    /// Whether the cut in force splits `c`.
    pub fn splits(&self, c: Chunk) -> bool {
        self.in_force().splits.contains(&c)
    }

    /// The chunk over `c`'s ground at the level the view draws that ground
    /// at: `c` itself where the view draws it at `c`'s level or finer. No
    /// coarser than the source's top, which the view never draws over.
    pub fn wanted_over(&self, c: Chunk) -> Chunk {
        let chain: Vec<Chunk> = std::iter::once(c).chain(c.ancestors()).filter(|k| k.level() <= self.o.top).collect();
        chain.into_iter().rev().find(|&k| !self.splits(k)).unwrap_or(c)
    }
}

#[cfg(test)]
#[path = "domain.test.rs"]
mod test;
