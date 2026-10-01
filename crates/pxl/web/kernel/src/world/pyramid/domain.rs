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
//! is what is known of what it holds already says, along each axis
//! (pxl-spec space/observation.rs `Contents`), taken at its nearest point to
//! the eye. A chunk is a box to this, and what it holds a box inside it:
//! how a layer stores what stands there — a heightfield, bands over a column
//! — is the layer's, read behind `Bounds` (world/observation.rs `Known`),
//! and no axis is special here. Inside a cell is
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
//! error along each axis is at most its extent, `CHUNK_CELLS` cells, so on
//! the screen at most its diagonal over its distance less the diagonal, and
//! a chunk of a level
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
use pxl_spec::space::lod::{cubes_meeting, fault_radius, Tau};
use pxl_spec::space::observation::{Bounds, Contents, Observation};
use pxl_spec::space::pvf3::REACH;
use pxl_spec::space::pyramid::{chunk_span_of, LevelPage};
use pxl_spec::space::v3::Aabb;

use crate::global::Global;
use crate::world::access::Access;
use crate::world::observation::{heights_revision, observation, Known, Order};
use crate::world::observer::{cell_angle, EdgeRays, Footprint, Lens, Sight};
use crate::world::quality::Effective;

use super::chunk::{Chunk, CHILDREN};
use super::Layer as _;

/// The most pieces the cover over a source whose top is `top` is, on a
/// planet no smaller than `planet`: every chunk of the top meeting the box
/// the far edge is stored in (pxl-spec space/observation.rs
/// `Observation::reach`) — the top's fault radius each way, never past it
/// (`Observation::far`), and over it as far again as the planet lowers
/// ground that far — inside what a position can name (pxl-spec
/// space/pvf3.rs `REACH`). An interval of `d` meets at most ⌊d / side⌋ + 2
/// chunks. The radius is `SPLIT` cells of the level over (pxl-spec
/// space/lod.rs) and a chunk `CHUNK_CELLS` of its own, so the reach is
/// 2 · `SPLIT` / `CHUNK_CELLS` chunks each way at every level — the criterion
/// moves it, the level does not — until the lane ends first.
pub const fn cover_pieces(top: u32, planet: f64) -> u32 {
    let (far, side) = (fault_radius(top), chunk_span_of(top));
    let reach = [2.0 * far, 2.0 * far, 2.0 * far + far * far / (2.0 * planet)];
    let (mut n, mut k) = (1, 0);
    while k < 3 {
        let across = (reach[k] / side) as u32 + 2;
        let lane = (REACH.max[k] / side).ceil() - (REACH.min[k] / side).floor();
        n *= if lane < across as f64 { lane as u32 } else { across };
        k += 1;
    }
    n
}

/// How near the eye a chunk of `level` has to come for its error to cross a
/// cell at `tau`, whatever it holds: its error along each axis is at most its
/// extent, so on the screen at most its diagonal over its distance less the
/// diagonal (world/observer.rs `Lens::error`).
pub fn split_reach(level: u32, tau: Tau) -> f64 {
    3f64.sqrt() * chunk_span_of(level) * (1.0 + 1.0 / cell_angle(tau))
}

/// The view cut at one quality: the chunks split, and the pieces that
/// leaves.
pub struct Cut {
    tau: Tau,
    splits: HashSet<Chunk>,
    pieces: Vec<Chunk>,
    /// Whether it is cut for the domain as it stands: its sets are kept
    /// across makings, emptied, so cutting allocates nothing once they have
    /// held the most a view cuts.
    made: bool,
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
#[derive(Clone, Copy)]
struct Seen {
    /// The footprints touching it: this many from here of the domain's
    /// `touching`, one list for every chunk, so noting one allocates nothing.
    touching: (u32, u32),
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
    /// What is known of what the world holds: which chunks hold anything,
    /// and which have an answer.
    known: Box<dyn Bounds>,
    /// The cover: every chunk of the top within the reach that holds
    /// anything and has an answer.
    roots: Vec<Chunk>,
    /// Chunks of the top within the reach with no answer yet.
    unanswered: u32,
    /// What the footprints say of each chunk asked about so far, and the
    /// footprints touching each, one after another.
    seen: HashMap<Chunk, Seen>,
    touching: Vec<u32>,
    /// The chunks a cut has still to look at.
    open: Vec<Chunk>,
    /// How near the eye each chunk cut comes and whether the view sees it —
    /// its place in the order — measured once: whoever orders or draws a
    /// piece asks again.
    measured: HashMap<Chunk, Order>,
    /// The qualities it may be cut at, finest first, and the cut at each
    /// made so far.
    ladder: Vec<Tau>,
    cuts: Vec<Cut>,
    /// Which cut is in force.
    at: usize,
}

impl Domain {
    /// The pieces `o` is cut into at `ladder[at]`, as far as `known` knows
    /// what the world holds and `footprints` say what the screen touches.
    pub fn of<'a>(o: Observation, ladder: &[Tau], at: usize, known: Box<dyn Bounds>, footprints: impl IntoIterator<Item = &'a Footprint>) -> Domain {
        let mut d = Domain {
            o,
            lens: None,
            footprints: Vec::new(),
            known,
            roots: Vec::new(),
            unanswered: 0,
            seen: HashMap::new(),
            touching: Vec::new(),
            open: Vec::new(),
            measured: HashMap::new(),
            ladder: Vec::new(),
            cuts: Vec::new(),
            at: 0,
        };
        d.remake(o, ladder, at, footprints);
        d
    }

    /// Made again for `o`, in the sets it already holds: a view is made
    /// again every frame the eye moves, and making it allocates nothing
    /// once it has held the most a view makes. What is known is what it was
    /// made with.
    pub fn remake<'a>(&mut self, o: Observation, ladder: &[Tau], at: usize, footprints: impl IntoIterator<Item = &'a Footprint>) {
        xpute_core::ensure!(at < ladder.len(), EINVAL);
        self.o = o;
        self.lens = Lens::of(&o);
        self.footprints.clear();
        self.footprints.extend(footprints.into_iter().cloned());
        self.roots.clear();
        self.unanswered = 0;
        self.seen.clear();
        self.touching.clear();
        self.measured.clear();
        self.ladder.clear();
        self.ladder.extend_from_slice(ladder);
        for c in &mut self.cuts {
            c.made = false;
        }
        while self.cuts.len() < ladder.len() {
            self.cuts.push(Cut {
                tau: ladder[0],
                splits: HashSet::new(),
                pieces: Vec::new(),
                made: false,
            });
        }
        self.at = at.min(ladder.len() - 1);
        self.find_roots();
        for i in 0..self.roots.len() {
            self.look(self.roots[i], None);
        }
        self.cut_at(self.at);
    }

    /// What the footprints say of `c`, noted once: of every footprint, or of
    /// those touching the chunk over it (`from`, a span of `touching`).
    fn look(&mut self, c: Chunk, from: Option<(u32, u32)>) {
        if self.seen.contains_key(&c) {
            return;
        }
        let (bound, error) = match self.contents(c) {
            // A piece of the cover is drawn one cell a chunk (world/chunk/
            // frontier.rs `drawn_cube`), and measured as it is drawn: half
            // that cell across, as a level's own cell is half the cube its
            // cells are drawn averaged into, and up the one box's whole
            // height.
            Contents::Holds { bound, .. } if self.outermost(c) => {
                let half = chunk_span_of(c.level()) / 2.0;
                (bound, [half, half, bound.extent()[2]])
            }
            Contents::Holds { bound, error } => (bound, error),
            _ => (c.region(), [0.0; 3]),
        };
        let start = self.touching.len() as u32;
        let (first, n) = from.unwrap_or((0, self.footprints.len() as u32));
        for j in first..first + n {
            let k = if from.is_some() { self.touching[j as usize] } else { j };
            if self.footprints[k as usize].touches(&bound) {
                self.touching.push(k);
            }
        }
        let touching = (start, self.touching.len() as u32 - start);
        let e = self.o.eye;
        let nearest = bound.nearest(e);
        let near = (0..3).map(|k| (nearest[k] - e[k]).powi(2)).sum::<f64>().sqrt();
        let error = self.lens.map_or(0.0, |l| l.error(nearest, error).into_iter().fold(0.0, f64::max));
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
        if !self.cuts[k].made {
            let tau = self.ladder[k];
            let cell = cell_angle(tau);
            let mut splits = core::mem::take(&mut self.cuts[k].splits);
            let mut pieces = core::mem::take(&mut self.cuts[k].pieces);
            let mut open = core::mem::take(&mut self.open);
            splits.clear();
            pieces.clear();
            open.clear();
            open.extend_from_slice(&self.roots);
            while let Some(c) = open.pop() {
                let s = self.seen[&c];
                let leaks = c.level() > 0 && s.touching.1 > 0 && s.error > cell;
                let (mut under, mut n) = ([c; CHILDREN], 0);
                if leaks {
                    for child in c.children() {
                        if self.occupied(child).is_some() {
                            (under[n], n) = (child, n + 1);
                        }
                    }
                }
                if n == 0 {
                    if self.answered(c) {
                        pieces.push(c);
                    }
                    continue;
                }
                // The rule's reach, whatever the world: past it is the
                // derivation broken, and the quality policy's count with it.
                xpute_core::ensure!(s.near < split_reach(c.level(), tau), ENOTRECOVERABLE, c.level());
                for &child in &under[..n] {
                    self.look(child, Some(s.touching));
                }
                splits.insert(c);
                open.extend_from_slice(&under[..n]);
            }
            pieces.sort_unstable();
            for &c in pieces.iter().chain(&splits) {
                if !self.measured.contains_key(&c) {
                    let m = self.measure(c);
                    self.measured.insert(c, m);
                }
            }
            self.open = open;
            self.cuts[k] = Cut { tau, splits, pieces, made: true };
        }
        &self.cuts[k]
    }

    /// The cut at step `k` of the ladder, where it has been made.
    pub fn cut(&self, k: usize) -> Option<&Cut> {
        self.cuts.get(k).filter(|c| c.made)
    }

    /// Puts the cut at step `k` of the ladder in force, making it if it was
    /// not. Whether the pieces changed.
    pub fn put(&mut self, k: usize) -> bool {
        xpute_core::ensure!(k < self.ladder.len(), EINVAL);
        let k = k.min(self.ladder.len() - 1);
        self.cut_at(k);
        let moved = self.cuts[k].pieces != self.cuts[self.at].pieces;
        self.at = k;
        moved
    }

    /// The cut in force.
    fn in_force(&self) -> &Cut {
        &self.cuts[self.at]
    }

    /// The quality the pieces are cut at.
    pub fn tau(&self) -> Tau {
        self.in_force().tau
    }

    /// Chunks of the top within the reach no page over is resident for:
    /// ground with no answer yet, which the goal has no piece of.
    pub fn unanswered(&self) -> u32 {
        self.unanswered
    }
}

/// The domain made from the view's observation as it last stood, what its
/// tiles saw, and which making of its pieces this is.
#[derive(Default)]
struct Made {
    key: Option<(Observation, (u32, u32), Tau)>,
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
    let key = Some((o, heights_revision(), ladder[0]));
    if made.key != key {
        let retarget = made.key.is_none_or(|(_, _, target)| target != ladder[0]);
        made.key = key;
        let paging = Terrain::pyramid().paging();
        let resident: HashSet<LevelPage> = paging.held().map(|(p, _)| p).collect();
        let moved: Vec<(f64, f64, f64)> = resident.symmetric_difference(&made.resident).map(|p| p.rect()).collect();
        let objects = key.map_or(0, |(_, h, _)| h.1);
        made.rays = made.sight.observe(&o, &EdgeRays(Access::now(), ladder[0]), retarget || objects != made.objects, &moved);
        made.resident = resident;
        made.objects = objects;
        match made.domain.as_mut() {
            Some(d) => d.remake(o, ladder, at, made.sight.footprints()),
            None => made.domain = Some(Domain::of(o, ladder, at, Box::new(Known), made.sight.footprints())),
        }
        made.revision = made.revision.wrapping_add(1);
        made.walks = made.walks.wrapping_add(1);
        return true;
    }
    if made.domain.as_mut().is_some_and(|d| d.put(at)) {
        made.revision = made.revision.wrapping_add(1);
    }
    false
}

/// What the screen's tiles saw when the domain was last made.
pub fn sight() -> &'static Sight {
    &MADE.get(Made::default).sight
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

impl Domain {
    /// What `c` holds, as far as what is known says.
    fn contents(&self, c: Chunk) -> Contents {
        self.known.contents(c.level(), c.region())
    }

    /// Where in `c` anything stands, or none where nothing does or nothing
    /// is known. A chunk that holds nothing is no node of the view — never
    /// split, counted or drawn — and there is no slot for it anywhere: its
    /// emptiness is this answer and not a thing kept.
    ///
    /// A bound and not the surface drawn, because what it decides is which
    /// chunks hold anything and how near they come, and the surface a page
    /// over a piece draws averages what is under it: a peak it averaged under
    /// its ceiling would be no chunk until the page under it came. A bound is
    /// no nearer than the truth, so the order it gives errs nearer.
    pub fn occupied(&self, c: Chunk) -> Option<Aabb> {
        match self.contents(c) {
            Contents::Holds { bound, .. } => Some(bound),
            _ => None,
        }
    }

    /// Whether anything is known of what `c` holds: some page at or over it
    /// is resident.
    fn answered(&self, c: Chunk) -> bool {
        self.contents(c) != Contents::Unknown
    }

    /// How near the eye `c` comes, and whether the view sees any of it: by
    /// where in it anything stands.
    fn measure(&self, c: Chunk) -> Order {
        if let Some(&m) = self.measured.get(&c) {
            return m;
        }
        Order::of(&self.o, &self.occupied(c).unwrap_or(c.region()))
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
    /// holds anything and has an answer, sorted.
    pub fn roots(&self) -> &[Chunk] {
        &self.roots
    }

    /// The root `c` is under, or is, where it is one: none where the view
    /// reaches no ground over it.
    pub fn root_of(&self, c: Chunk) -> Option<Chunk> {
        let top = core::iter::once(c).chain(c.ancestors()).find(|&a| self.outermost(a))?;
        self.roots.binary_search(&top).is_ok().then_some(top)
    }

    /// The qualities it may be cut at, finest first.
    pub fn ladder(&self) -> &[Tau] {
        &self.ladder
    }

    /// Every chunk of the top within the reach that holds anything; the ones
    /// with no answer, counted. The chunks are the ones the box the far edge
    /// is stored in meets (pxl-spec space/observation.rs
    /// `Observation::reach`): a number the observation alone bounds
    /// (`cover_pieces`), whatever the world holds.
    fn find_roots(&mut self) {
        let top = self.o.top;
        let mut roots = core::mem::take(&mut self.roots);
        roots.clear();
        for at in cubes_meeting(chunk_span_of(top), self.o.reach(self.o.far)) {
            let c = Chunk::at(top, at);
            if Order::of(&self.o, &c.region()).near >= self.o.far {
                continue;
            }
            match self.contents(c) {
                Contents::Unknown => self.unanswered += 1,
                Contents::Empty => {}
                Contents::Holds { bound, .. } => {
                    let m = Order::of(&self.o, &bound);
                    if m.near < self.o.far {
                        roots.push(c);
                        self.measured.insert(c, m);
                    }
                }
            }
        }
        xpute_core::ensure!(roots.len() as u32 <= cover_pieces(top, self.o.planet), ENOTRECOVERABLE, roots.len());
        roots.sort_unstable();
        self.roots = roots;
    }

    /// Whether the cut in force splits `c`.
    pub fn splits(&self, c: Chunk) -> bool {
        self.in_force().splits.contains(&c)
    }

    /// The chunk over `c`'s ground at the level the view draws that ground
    /// at: `c` itself where the view draws it at `c`'s level or finer. No
    /// coarser than the source's top, which the view never draws over.
    pub fn wanted_over(&self, c: Chunk) -> Chunk {
        std::iter::once(c).chain(c.ancestors()).filter(|k| k.level() <= self.o.top && !self.splits(*k)).last().unwrap_or(c)
    }
}

#[cfg(test)]
#[path = "domain.test.rs"]
mod test;
