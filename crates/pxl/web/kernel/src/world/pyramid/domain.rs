// pxl-kernel/world/pyramid/domain.rs

//! The frontier's goal: the view's ground cut into pyramid chunks, the same
//! for every layer. Made per frame from the observation and what is
//! resident; paging never reads it.
//!
//! Every top-level chunk within reach that holds anything is a piece (the
//! cover), all around the eye, so ground turned to shows the cover and never
//! a hole. A chunk splits only where some tile footprint touches it and its
//! projected error crosses a cell (`Lens::error` against `cell_angle`). A
//! chunk splits only within `split_reach`, which bounds the goal whatever the
//! screen. A coarser quality splits a subset of what a finer one does.

use std::collections::{HashMap, HashSet};

use pxl_spec::bake::format::Terrain;
use pxl_spec::space::lod::{cubes_meeting, fault_radius, Tau};
use pxl_spec::space::observation::{Bounds, Contents, Observation};
use pxl_spec::space::pvf3::REACH;
use pxl_spec::space::pyramid::{chunk_span_of, LevelPage, TOP_LEVEL};
use pxl_spec::space::v3::Aabb;

use crate::global::Global;
use crate::world::access::Access;
use crate::world::observation::{heights_revision, observation, Known, Order};
use crate::world::observer::{cell_angle, EdgeRays, Footprint, Lens, Sight};
use crate::world::quality::Effective;

use super::chunk::{Chunk, CHILDREN};
use super::Layer as _;

/// Most pieces the cover can be: every top-level chunk meeting the far
/// edge's box (`Observation::reach`), clamped to the position lane. An
/// interval of `d` meets at most ⌊d / side⌋ + 2 chunks, and the reach is
/// 2 · `SPLIT` / `CHUNK_CELLS` chunks each way at every level.
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

/// How near a chunk of `level` must come for its error to cross a cell:
/// the error is at most its diagonal over its distance less the diagonal.
pub fn split_reach(level: u32, tau: Tau) -> f64 {
    3f64.sqrt() * chunk_span_of(level) * (1.0 + 1.0 / cell_angle(tau))
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

#[derive(Clone, Copy)]
struct Seen {
    /// A span of the domain's shared `touching` list, so noting one allocates nothing.
    touching: (u32, u32),
    error: f64,
    near: f64,
}

pub struct Domain {
    o: Observation,
    lens: Option<Lens>,
    footprints: Vec<Footprint>,
    known: Box<dyn Bounds>,
    roots: Vec<Chunk>,
    unanswered: u32,
    seen: HashMap<Chunk, Seen>,
    touching: Vec<u32>,
    open: Vec<Chunk>,
    /// Measured once per cut chunk; whoever orders or draws a piece reads it here.
    measured: HashMap<Chunk, Order>,
    ladder: Vec<Tau>,
    cuts: Vec<Cut>,
    at: usize,
}

impl Domain {
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

    /// Remade in place every frame the eye moves, allocating nothing once warm.
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

    fn look(&mut self, c: Chunk, from: Option<(u32, u32)>) {
        if self.seen.contains_key(&c) {
            return;
        }
        let (bound, error) = match self.contents(c) {
            // A cover piece is drawn one cell per chunk (`drawn_cube`) and measured
            // as drawn: half that cell across, and the box's full height.
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

    /// Cuts from the cover down: a touched chunk whose error spans more than a
    /// cell splits into its non-empty children. A chunk with no non-empty
    /// children stays a piece, so a split never removes a piece and a finer
    /// quality never counts less. Pieces with no answer are left out.
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
                // Past the rule's reach the derivation, and the quality policy's count, are broken.
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

    pub fn cut(&self, k: usize) -> Option<&Cut> {
        self.cuts.get(k).filter(|c| c.made)
    }

    /// Returns whether the pieces changed.
    pub fn put(&mut self, k: usize) -> bool {
        xpute_core::ensure!(k < self.ladder.len(), EINVAL);
        let k = k.min(self.ladder.len() - 1);
        self.cut_at(k);
        let moved = self.cuts[k].pieces != self.cuts[self.at].pieces;
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
}

#[derive(Default)]
struct Made {
    key: Option<(Observation, (u32, u32), Tau)>,
    domain: Option<Domain>,
    sight: Sight,
    resident: HashSet<LevelPage>,
    objects: u32,
    rays: u32,
    revision: u32,
    walks: u32,
}

static MADE: Global<Made> = Global::new();

/// Remakes the domain when the observation or residency moved, cut at the
/// effective quality; otherwise only puts that cut in force. Returns whether
/// it was remade (all cut counts are stale). Only the quality policy calls it.
pub fn step(ladder: &[Tau], q: Effective) -> bool {
    let Some(o) = observation() else { return false };
    let at = ladder.iter().position(|&t| t == q.tau()).unwrap_or(0);
    let made = MADE.get(Made::default);
    let key = Some((o, heights_revision(), ladder[0]));
    if made.key != key {
        let retarget = made.key.is_none_or(|(_, _, target)| target != ladder[0]);
        made.key = key;
        let paging = Terrain::pyramid().paging();
        let resident: HashSet<LevelPage> = (0..=TOP_LEVEL).flat_map(|level| paging.level(level).map(|(p, _)| p)).collect();
        let moved: Vec<(f64, f64, f64)> = resident.symmetric_difference(&made.resident).map(|p| p.rect()).collect();
        let objects = heights_revision().1;
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

pub fn sight() -> &'static Sight {
    &MADE.get(Made::default).sight
}

pub fn current() -> Option<&'static Domain> {
    MADE.get(Made::default).domain.as_ref()
}

pub fn cut_at(k: usize) -> Option<(&'static Domain, &'static Cut)> {
    let d: &'static mut Domain = MADE.get(Made::default).domain.as_mut()?;
    d.cut_at(k);
    let d: &'static Domain = d;
    Some((d, d.cut(k)?))
}

pub fn rays() -> u32 {
    MADE.get(Made::default).rays
}

pub fn walks() -> u32 {
    MADE.get(Made::default).walks
}

pub fn revision() -> u32 {
    MADE.get(Made::default).revision
}

impl Domain {
    fn contents(&self, c: Chunk) -> Contents {
        self.known.contents(c.level(), c.region())
    }

    /// Where in `c` anything stands. A bound, not the drawn surface: a coarse
    /// page's surface averages a peak away, which would leave the chunk empty
    /// until the finer page came. A bound errs nearer.
    pub fn occupied(&self, c: Chunk) -> Option<Aabb> {
        match self.contents(c) {
            Contents::Holds { bound, .. } => Some(bound),
            _ => None,
        }
    }

    fn answered(&self, c: Chunk) -> bool {
        self.contents(c) != Contents::Unknown
    }

    fn measure(&self, c: Chunk) -> Order {
        if let Some(&m) = self.measured.get(&c) {
            return m;
        }
        Order::of(&self.o, &self.occupied(c).unwrap_or(c.region()))
    }

    pub fn nearest_to(&self, c: Chunk) -> f64 {
        self.measure(c).near
    }

    pub fn order_of(&self, c: Chunk) -> Order {
        self.measure(c)
    }

    pub fn outermost(&self, c: Chunk) -> bool {
        c.level() >= self.o.top
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

    pub fn root_of(&self, c: Chunk) -> Option<Chunk> {
        let top = core::iter::once(c).chain(c.ancestors()).find(|&a| self.outermost(a))?;
        self.roots.binary_search(&top).is_ok().then_some(top)
    }

    pub fn ladder(&self) -> &[Tau] {
        &self.ladder
    }

    /// Counted against `cover_pieces`, which the observation alone bounds.
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
                Contents::Holds { .. } => {
                    if self.nearest_to(c) < self.o.far {
                        roots.push(c);
                    }
                }
            }
        }
        xpute_core::ensure!(roots.len() as u32 <= cover_pieces(top, self.o.planet), ENOTRECOVERABLE, roots.len());
        roots.sort_unstable();
        for &c in &roots {
            let m = self.measure(c);
            self.measured.insert(c, m);
        }
        self.roots = roots;
    }

    pub fn splits(&self, c: Chunk) -> bool {
        self.in_force().splits.contains(&c)
    }

    /// The chunk over `c`'s ground at the level the view draws it, never above the top.
    pub fn wanted_over(&self, c: Chunk) -> Chunk {
        let chain: Vec<Chunk> = std::iter::once(c).chain(c.ancestors()).filter(|k| k.level() <= self.o.top).collect();
        chain.into_iter().rev().find(|&k| !self.splits(k)).unwrap_or(c)
    }
}

#[cfg(test)]
#[path = "domain.test.rs"]
mod test;
