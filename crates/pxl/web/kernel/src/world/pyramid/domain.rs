// pxl-kernel/world/pyramid/domain.rs

//! The frontier's goal: the view's ground cut into pyramid chunks, the same
//! for every layer. Residency never reads it.
//!
//! One walk gives every quality step's cut: a chunk is a piece over an
//! interval of steps. It runs a slice a turn on the observation it began
//! with, and is published whole.
//!
//! Every chunk at the top within reach that holds anything is a piece (the
//! cover), all around the eye, so ground turned to shows the cover and never
//! a hole. A chunk splits only where some patch's footprint touches it and its
//! projected error crosses a cell (`Lens::error` against `cell_angle`). A
//! chunk splits only within `split_reach`, which bounds the goal whatever the
//! screen. A coarser quality splits a subset of what a finer one does.

use std::collections::{HashMap, HashSet};

use pxl_spec::bake::format::Terrain;
use pxl_spec::space::lod::{cubes_meeting, demand_reach, Tau};
use pxl_spec::space::observation::{Bounds, Contents, Observation, Requirement};
use pxl_spec::space::pvf3::REACH;
use pxl_spec::space::pyramid::{chunk_zoom, Tile, LEAF_Z};
use pxl_spec::space::v3::Aabb;

use crate::world::access::Access;
use crate::world::chunk::mesh_slot::POOLS;
use crate::world::observation::{heights_revision, observation, Known, Order};
use crate::world::observer::{cell_angle, EdgeRays, Footprint, Lens, Sight};
use xpute_runtime::global::Global;

use super::chunk::Chunk;
use super::Layer as _;
use pxl_spec::space::zoom::{units, Zoom};

/// Most pieces the cover can be: every chunk at the top meeting the far
/// edge's box (`Observation::reach`), clamped to the position lane. An
/// interval of `d` meets at most ⌊d / side⌋ + 2 chunks, and the reach is
/// 2 · `SPLIT` / `CHUNK_CELLS` chunks each way at every zoom.
pub const fn cover_pieces(top: Zoom, planet: f64) -> u32 {
    let (far, side) = (demand_reach(top), units(chunk_zoom(top)));
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

/// How near a chunk at `z` must come for its error to cross a cell:
/// the error is at most its diagonal over its distance less the diagonal.
fn split_reach(zoom: Zoom, tau: Tau) -> f64 {
    3f64.sqrt() * units(chunk_zoom(zoom)) * (1.0 + 1.0 / cell_angle(tau))
}

pub struct Cut {
    tau: Tau,
    splits: HashSet<Chunk>,
    pieces: Vec<Chunk>,
    /// Its sets are reused across makings, so cutting stops allocating once warm.
    made: bool,
}

impl Cut {
    pub fn tau(&self) -> Tau {
        self.tau
    }

    pub fn pieces(&self) -> &[Chunk] {
        &self.pieces
    }

    pub fn splits(&self) -> impl Iterator<Item = Chunk> + '_ {
        self.splits.iter().copied()
    }
}

/// Mesh slots per pool.
pub trait Price {
    fn piece(&self, c: Chunk, outermost: bool, near: f64, tau: Tau) -> [u32; POOLS];
    fn split(&self, c: Chunk) -> [u32; POOLS];
    /// stop counting a step needing `n`
    fn over(&self, n: &[u32; POOLS]) -> bool;
}

/// No cost, no limit.
pub struct Free;

impl Price for Free {
    fn piece(&self, _: Chunk, _: bool, _: f64, _: Tau) -> [u32; POOLS] {
        [0; POOLS]
    }

    fn split(&self, _: Chunk) -> [u32; POOLS] {
        [0; POOLS]
    }

    fn over(&self, _: &[u32; POOLS]) -> bool {
        false
    }
}

#[derive(Clone, Copy)]
struct Seen {
    order: Order,
    /// piece at `from..to`, split below both
    from: u8,
    to: u8,
}

struct Open {
    c: Chunk,
    /// steps the parent splits at
    to: u8,
    from: Option<(u32, u32)>,
    /// taken by a child read unknown mid-walk
    parent: Option<(Aabb, [f64; 3])>,
}

pub struct Domain {
    o: Observation,
    lens: Option<Lens>,
    footprints: Vec<Footprint>,
    lacks: Vec<Requirement>,
    known: Box<dyn Bounds>,
    roots: Vec<Chunk>,
    unanswered: u32,
    seen: HashMap<Chunk, Seen>,
    touching: Vec<u32>,
    open: Vec<Open>,
    ladder: Vec<Tau>,
    /// finest step still counted
    tracked: usize,
    need: Vec<[u32; POOLS]>,
    counted: Vec<u32>,
    cuts: Vec<Cut>,
    at: usize,
    done: bool,
}

impl Domain {
    pub fn priced<'a>(o: Observation, ladder: &[Tau], at: usize, known: Box<dyn Bounds>, footprints: impl IntoIterator<Item = &'a Footprint>, price: &dyn Price) -> Domain {
        let mut d = Domain::empty(o, known);
        d.begin(o, ladder, footprints, core::iter::empty());
        d.run(price, &|| false, &mut 0);
        d.put(at);
        d
    }

    fn empty(o: Observation, known: Box<dyn Bounds>) -> Domain {
        Domain {
            o,
            lens: None,
            footprints: Vec::new(),
            lacks: Vec::new(),
            known,
            roots: Vec::new(),
            unanswered: 0,
            seen: HashMap::new(),
            touching: Vec::new(),
            open: Vec::new(),
            ladder: Vec::new(),
            tracked: 0,
            need: Vec::new(),
            counted: Vec::new(),
            cuts: Vec::new(),
            at: 0,
            done: false,
        }
    }

    fn begin<'a, 'b>(&mut self, o: Observation, ladder: &[Tau], footprints: impl IntoIterator<Item = &'a Footprint>, lacks: impl IntoIterator<Item = &'b Requirement>) {
        xpute_core::ensure!(!ladder.is_empty() && ladder.len() < u8::MAX as usize, EINVAL);
        self.o = o;
        self.lens = Lens::of(&o);
        self.footprints.clear();
        self.footprints.extend(footprints.into_iter().cloned());
        self.lacks.clear();
        self.lacks.extend(lacks.into_iter().copied());
        self.roots.clear();
        self.unanswered = 0;
        self.seen.clear();
        self.touching.clear();
        self.open.clear();
        self.ladder.clear();
        self.ladder.extend_from_slice(ladder);
        self.tracked = 0;
        self.need.clear();
        self.need.resize(ladder.len(), [0; POOLS]);
        self.counted.clear();
        self.counted.resize(ladder.len(), 0);
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
        self.done = false;
        self.find_roots();
        let top = ladder.len() as u8;
        for i in 0..self.roots.len() {
            self.open.push(Open {
                c: self.roots[i],
                to: top,
                from: None,
                parent: None,
            });
        }
    }

    /// Whether done; counts what it takes off the stack into `n`.
    fn run(&mut self, price: &dyn Price, stop: &dyn Fn() -> bool, n: &mut u32) -> bool {
        // at least a slice a turn
        const SLICE: u32 = 32;
        let mut k = 0;
        while let Some(open) = self.open.pop() {
            if (open.to as usize) > self.tracked {
                self.visit(open, price);
            }
            k += 1;
            *n += 1;
            if k % SLICE == 0 && stop() {
                return false;
            }
        }
        self.done = true;
        true
    }

    fn visit(&mut self, open: Open, price: &dyn Price) {
        let Open { c, to, from, parent } = open;
        if self.seen.contains_key(&c) {
            return;
        }
        let (bound, error) = match (self.contents(c), parent) {
            // A cover piece is drawn one cell per chunk (`drawn_scale`) and measured
            // as drawn: half that cell across, and the box's full height.
            (Contents::Holds { bound, .. }, _) if self.outermost(c) => {
                let half = units(chunk_zoom(c.tile_zoom())) / 2.0;
                (bound, [half, half, bound.extent()[2]])
            }
            (Contents::Holds { bound, error }, _) => (bound, error),
            // evicted between slices: the parent's bound holds
            (_, Some((bound, error))) => (bound.meet(c.region()).unwrap_or(c.region()), error),
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
        let nearest = bound.nearest(self.o.eye);
        let error_angle = self.lens.map_or(0.0, |l| l.error(nearest, error).into_iter().fold(0.0, f64::max));
        let order = Order::of(&self.o, &bound);
        let mut from_step = if c.tile_zoom() < LEAF_Z && touching.1 > 0 {
            self.ladder.iter().take_while(|&&tau| error_angle > cell_angle(tau)).count() as u8
        } else {
            0
        };
        let splits_to = from_step.min(to);
        if splits_to as usize > self.tracked {
            let mut any = false;
            for child in c.children() {
                if self.contents(child) != Contents::Empty {
                    any = true;
                    self.open.push(Open {
                        c: child,
                        to: splits_to,
                        from: Some(touching),
                        parent: Some((bound, error)),
                    });
                }
            }
            if any {
                // Past the rule's reach the derivation, and the quality policy's count, are broken.
                xpute_core::ensure!(order.near < split_reach(c.tile_zoom(), self.ladder[splits_to as usize - 1]), ENOTRECOVERABLE, c.tile_zoom());
            } else {
                from_step = 0;
            }
        }
        let splits_to = from_step.min(to);
        self.seen.insert(c, Seen { order, from: from_step, to });
        let outermost = self.outermost(c);
        for k in self.tracked..self.ladder.len() {
            let at = k as u8;
            let add = if from_step <= at && at < to {
                self.counted[k] += 1;
                price.piece(c, outermost, order.near, self.ladder[k])
            } else if at < splits_to {
                price.split(c)
            } else {
                continue;
            };
            for (n, a) in self.need[k].iter_mut().zip(add) {
                *n += a;
            }
        }
        while self.tracked + 1 < self.ladder.len() && price.over(&self.need[self.tracked]) {
            self.tracked += 1;
        }
    }

    /// Empty for a step no longer counted.
    pub fn cut_at(&mut self, k: usize) -> &Cut {
        if !self.cuts[k].made {
            let at = k as u8;
            let mut splits = core::mem::take(&mut self.cuts[k].splits);
            let mut pieces = core::mem::take(&mut self.cuts[k].pieces);
            splits.clear();
            pieces.clear();
            if k >= self.tracked {
                for (&c, s) in &self.seen {
                    if s.from <= at && at < s.to {
                        pieces.push(c);
                    } else if at < s.from.min(s.to) {
                        splits.insert(c);
                    }
                }
            }
            pieces.sort_unstable();
            self.cuts[k] = Cut {
                tau: self.ladder[k],
                splits,
                pieces,
                made: true,
            };
        }
        &self.cuts[k]
    }

    /// Returns whether the pieces changed.
    pub fn put(&mut self, k: usize) -> bool {
        xpute_core::ensure!(k < self.ladder.len(), EINVAL);
        let k = k.min(self.ladder.len() - 1).max(self.tracked);
        self.cut_at(k);
        let moved = self.cuts[k].pieces != self.cuts[self.at].pieces || !self.cuts[self.at].made;
        self.at = k;
        moved
    }

    fn in_force(&self) -> &Cut {
        &self.cuts[self.at]
    }

    pub fn tau(&self) -> Tau {
        self.in_force().tau
    }

    pub fn unanswered(&self) -> u32 {
        self.unanswered
    }

    /// None: past the arena.
    pub fn need(&self, k: usize) -> Option<[u32; POOLS]> {
        (k >= self.tracked).then(|| self.need[k])
    }

    /// None: past the arena.
    pub fn counted(&self, k: usize) -> Option<u32> {
        (k >= self.tracked).then(|| self.counted[k])
    }

    pub fn footprints(&self) -> &[Footprint] {
        &self.footprints
    }

    pub fn lacks(&self) -> &[Requirement] {
        &self.lacks
    }
}

#[derive(Default)]
struct Made {
    /// whole
    published: Option<Domain>,
    /// sliced
    job: Option<Domain>,
    spare: Option<Domain>,
    key: Option<(Observation, (u32, u32), Tau, u32)>,
    sight: Sight,
    resident: HashSet<Tile>,
    revision: u32,
    /// rays cast and nodes visited since `take_work`
    work: [u32; 2],
}

static MADE: Global<Made> = Global::new();

/// Starts the next goal's job, casting the screen's rays for it, when the
/// observation, residency, target or `extra` moved; a job is never restarted.
pub fn start(ladder: &[Tau], extra: u32) {
    let made = MADE.get(Made::default);
    if made.job.is_some() {
        return;
    }
    let Some(o) = observation() else { return };
    let key = (o, heights_revision(), ladder[0], extra);
    if made.key == Some(key) && made.published.is_some() {
        return;
    }
    // a new target or moved objects recast every patch; moved ones the patches over them
    let everything = made.key.is_none_or(|(_, (_, objects), tau, _)| tau != key.2 || objects != key.1 .1);
    made.key = Some(key);
    let resident: HashSet<Tile> = Terrain::pyramid().residency().held().map(|(p, _)| p).collect();
    let near = |p: &Tile| {
        let r = pxl_spec::space::pvf3::nearest_box(&p.region(), o.eye[0]);
        (r.min[0], r.min[1], r.max[0] - r.min[0])
    };
    let moved: Vec<(f64, f64, f64)> = resident.symmetric_difference(&made.resident).map(near).collect();
    made.work[0] += made.sight.observe(&o, &EdgeRays(Access::now(), ladder[0]), everything, &moved);
    made.resident = resident;
    let mut job = made.spare.take().unwrap_or_else(|| Domain::empty(o, Box::new(Known)));
    job.begin(o, ladder, made.sight.footprints(), made.sight.lacks());
    made.job = Some(job);
}

/// A slice of the started goal; publishes it whole.
pub fn advance(at: usize, price: &dyn Price, stop: &dyn Fn() -> bool) -> bool {
    let made = MADE.get(Made::default);
    let Some(job) = made.job.as_mut() else { return false };
    if !job.run(price, stop, &mut made.work[1]) {
        return false;
    }
    let Some(mut done) = made.job.take() else { return false };
    done.put(at);
    made.spare = made.published.replace(done);
    made.revision = made.revision.wrapping_add(1);
    true
}

pub fn take_work() -> [u32; 2] {
    core::mem::take(&mut MADE.get(Made::default).work)
}

pub fn put(k: usize) {
    let made = MADE.get(Made::default);
    if made.published.as_mut().is_some_and(|d| d.put(k)) {
        made.revision = made.revision.wrapping_add(1);
    }
}

pub fn walking() -> bool {
    MADE.get(Made::default).job.is_some()
}

pub fn current() -> Option<&'static Domain> {
    MADE.get(Made::default).published.as_ref()
}

pub fn revision() -> u32 {
    MADE.get(Made::default).revision
}

impl Domain {
    fn contents(&self, c: Chunk) -> Contents {
        self.known.contents(c.tile_zoom(), c.region())
    }

    /// Measured from where in `c` anything stands: a bound, not the drawn
    /// surface, which on a coarse tile averages a peak away and would leave
    /// the chunk empty until the finer tile came. A bound errs nearer.
    pub fn order_of(&self, c: Chunk) -> Order {
        if let Some(s) = self.seen.get(&c) {
            return s.order;
        }
        let bound = match self.contents(c) {
            Contents::Holds { bound, .. } => bound,
            _ => c.region(),
        };
        Order::of(&self.o, &bound)
    }

    pub fn outermost(&self, c: Chunk) -> bool {
        c.tile_zoom() <= self.o.top
    }

    pub fn pieces(&self) -> &[Chunk] {
        &self.in_force().pieces
    }

    pub fn observation(&self) -> &Observation {
        &self.o
    }

    pub fn roots(&self) -> &[Chunk] {
        &self.roots
    }

    pub fn ladder(&self) -> &[Tau] {
        &self.ladder
    }

    /// Counted against `cover_pieces`, which the observation alone bounds.
    fn find_roots(&mut self) {
        let top = self.o.top;
        let mut roots = core::mem::take(&mut self.roots);
        roots.clear();
        for at in cubes_meeting(units(chunk_zoom(top)), self.o.reach(self.o.far)) {
            let c = Chunk::at(top, at);
            if Order::of(&self.o, &c.region()).near >= self.o.far {
                continue;
            }
            match self.contents(c) {
                Contents::Unknown => self.unanswered += 1,
                Contents::Empty => {}
                Contents::Holds { bound, .. } => {
                    if Order::of(&self.o, &bound).near < self.o.far {
                        roots.push(c);
                    }
                }
            }
        }
        xpute_core::ensure!(roots.len() as u32 <= cover_pieces(top, self.o.planet), ENOTRECOVERABLE, roots.len());
        roots.sort_unstable();
        self.roots = roots;
    }

    pub fn splits(&self, c: Chunk) -> bool {
        self.in_force().splits.contains(&c)
    }

    /// The chunk over `c`'s ground at the zoom the view draws it, never past the top.
    pub fn wanted_over(&self, c: Chunk) -> Chunk {
        std::iter::once(c).chain(c.ancestors()).filter(|k| k.tile_zoom() >= self.o.top && !self.splits(*k)).last().unwrap_or(c)
    }
}

#[cfg(test)]
#[path = "domain.test.rs"]
mod test;
