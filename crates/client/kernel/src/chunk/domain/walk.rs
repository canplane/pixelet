// pxl-kernel/chunk/domain/walk.rs

//! One walk of the cut and its result: every quality step's pieces from one
//! descent over the ground, priced as it goes.

use fnv::FnvHashMap;
use pxl_spec::space::coord::world::nearest_box;

use pxl_spec::space::camera::eye::Observation;
use pxl_spec::space::camera::grid::Tau;
use pxl_spec::space::coord::aabb::Aabb;
use pxl_spec::space::layer::demand::cubes_meeting;
use pxl_spec::space::layer::lattice::GROUND;
use pxl_spec::space::layer::tile::Tile;

use crate::chunk::mesh_slot::POOLS;
use crate::io::demand::Order;
use crate::layer::{Answer, Bound, Query};
use crate::query::World;
use crate::view::sight::{cell_angle, Footprint, Lens};

use crate::chunk::Chunk;
use pxl_spec::space::coord::zoom::{units, Zoom, SHARD_ZOOM};

use super::Price;

/// How near a chunk must come for its error to cross a cell: the error is
/// at most its diagonal over its distance less the diagonal.
fn split_reach(c: Chunk, tau: Tau) -> f64 {
    3f64.sqrt() * units(c.zoom()) * (1.0 + 1.0 / cell_angle(tau))
}

pub struct Cut {
    tau: Tau,
    pieces: Vec<Chunk>,
    /// Its list is reused across makings, so cutting stops allocating once warm.
    made: bool,
}

impl Cut {
    pub fn tau(&self) -> Tau {
        self.tau
    }

    pub fn pieces(&self) -> &[Chunk] {
        &self.pieces
    }
}

#[derive(Clone, Copy)]
struct Seen {
    order: Order,
    /// piece at `from..to`, split below both
    from: u8,
    to: u8,
}

struct Open<C> {
    c: Chunk,
    /// The query's cursor at `c`, carried down from its parent.
    cursor: C,
    /// steps the parent splits at
    to: u8,
    from: Option<(u32, u32)>,
    /// taken by a child read unknown mid-walk
    parent: Option<(Aabb, [f64; 3])>,
}

pub struct Domain<Q: Query = World<'static>> {
    o: Observation,
    lens: Option<Lens>,
    footprints: Vec<Footprint>,
    lacks: Vec<Tile>,
    /// What the walk descends: the world, as the query answers it.
    query: Q,
    /// What the walk starts from, kept to be reused.
    start: Vec<Chunk>,
    unanswered: u32,
    seen: FnvHashMap<Chunk, Seen>,
    touching: Vec<u32>,
    open: Vec<Open<Q::Cursor>>,
    ladder: Vec<Tau>,
    /// finest step still counted
    tracked: usize,
    need: Vec<[u32; POOLS]>,
    counted: Vec<u32>,
    cuts: Vec<Cut>,
    at: usize,
}

impl<Q: Query> Domain<Q> {
    pub(crate) fn empty(o: Observation, query: Q) -> Domain<Q> {
        Domain {
            o,
            lens: None,
            footprints: Vec::new(),
            lacks: Vec::new(),
            query,
            start: Vec::new(),
            unanswered: 0,
            seen: FnvHashMap::default(),
            touching: Vec::new(),
            open: Vec::new(),
            ladder: Vec::new(),
            tracked: 0,
            need: Vec::new(),
            counted: Vec::new(),
            cuts: Vec::new(),
            at: 0,
        }
    }

    /// `top`: the coarsest zoom the ground's source supplies whole.
    pub(crate) fn begin<'a, 'b>(&mut self, o: Observation, top: Zoom, ladder: &[Tau], footprints: impl IntoIterator<Item = &'a Footprint>, lacks: impl IntoIterator<Item = &'b Tile>) {
        xpute_kit::ensure!(!ladder.is_empty() && ladder.len() < u8::MAX as usize, EINVAL);
        self.o = o;
        self.lens = Lens::of(&o);
        self.footprints.clear();
        self.footprints.extend(footprints.into_iter().cloned());
        self.lacks.clear();
        self.lacks.extend(lacks.into_iter().copied());
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
                pieces: Vec::new(),
                made: false,
            });
        }
        let (coarsest, far) = (GROUND.grid_zoom(top), o.far());
        let mut start = core::mem::take(&mut self.start);
        start.clear();
        start.extend(cubes_meeting(units(SHARD_ZOOM), o.reach(far)).map(|at| Chunk::indexed(SHARD_ZOOM, at)));
        let mut k = 0;
        while k < start.len() {
            let c = start[k];
            if c.cell_zoom() < coarsest {
                start.swap_remove(k);
                start.extend(c.children().into_iter().filter(|c| Order::of(&o, &c.region()).near < far));
            } else {
                k += 1;
            }
        }
        start.retain(|&c| self.starts(c));
        start.sort_unstable();
        let top = ladder.len() as u8;
        for &c in &start {
            let cursor = self.query.cursor(c.node(), c.cell_zoom());
            self.open.push(Open {
                c,
                cursor,
                to: top,
                from: None,
                parent: None,
            });
        }
        self.start = start;
    }

    /// Whether the walk starts from `c`: it holds anything within reach. One
    /// not known is counted.
    fn starts(&mut self, c: Chunk) -> bool {
        if Order::of(&self.o, &c.region()).near >= self.o.far() {
            return false;
        }
        match self.bound(c) {
            Answer::Nothing(_) => {
                self.unanswered += 1;
                false
            }
            Answer::Exact(None) | Answer::Coarser(None, _) => false,
            Answer::Exact(Some(b)) | Answer::Coarser(Some(b), _) => Order::of(&self.o, &b.aabb).near < self.o.far(),
        }
    }

    /// Whether done; counts what it takes off the stack into `n`.
    pub(crate) fn run(&mut self, price: &dyn Price, stop: &dyn Fn() -> bool, n: &mut u32) -> bool {
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
        true
    }

    fn visit(&mut self, open: Open<Q::Cursor>, price: &dyn Price) {
        let Open { c, cursor, to, from, parent } = open;
        if self.seen.contains_key(&c) {
            return;
        }
        let (first, n) = from.unwrap_or((0, self.footprints.len() as u32));
        let candidates = (first..first + n).map(|j| if from.is_some() { self.touching[j as usize] } else { j });
        // Where the eye's rays land is there, whatever the bound holds: a
        // cave the hash opens under a dig is met by them and drawn.
        let region = c.region();
        let landed = candidates
            .clone()
            .filter_map(|k| self.footprints[k as usize].landed(&region))
            .reduce(|a, b| (a.0.min(b.0), a.1.max(b.1)));
        let landed = landed.map(|(lo, hi)| Bound {
            aabb: Aabb {
                min: [region.min[0], region.min[1], lo],
                max: [region.max[0], region.max[1], hi],
            },
            finest: GROUND.cell(),
        });
        let held = Bound::join(self.query.bound(&cursor, c.node(), c.cell_zoom()).value().flatten(), landed);
        let (bound, error) = match (held, parent) {
            // The chunk's cell, alike on every axis.
            (Some(b), _) => (b.aabb, [units(c.cell_zoom()); 3]),
            // evicted between slices: the parent's bound holds
            (_, Some((bound, error))) => (bound.meet(c.region()).unwrap_or(c.region()), error),
            _ => (c.region(), [0.0; 3]),
        };
        let start = self.touching.len() as u32;
        for j in first..first + n {
            let k = if from.is_some() { self.touching[j as usize] } else { j };
            if self.footprints[k as usize].touches(&bound) {
                self.touching.push(k);
            }
        }
        let touching = (start, self.touching.len() as u32 - start);
        let nearest = nearest_box(&bound, self.o.eye[0]).nearest(self.o.eye);
        let error_angle = self.lens.map_or(0.0, |l| l.error(nearest, error));
        let order = Order::of(&self.o, &bound);
        // Drawn as finely as anything under it is held, there is nothing to refine.
        let finer_held = held.is_some_and(|b| c.cell_zoom() < b.finest);
        let mut from_step = if finer_held && touching.1 > 0 {
            self.ladder.iter().take_while(|&&tau| error_angle > cell_angle(tau)).count() as u8
        } else {
            0
        };
        let splits_to = from_step.min(to);
        if splits_to as usize > self.tracked {
            let mut any = false;
            for child in c.children() {
                let cursor = self.query.child(&cursor, child.node(), child.cell_zoom());
                let met = (touching.0..touching.0 + touching.1).any(|j| self.footprints[self.touching[j as usize] as usize].touches(&child.region()));
                if met || !matches!(self.query.bound(&cursor, child.node(), child.cell_zoom()), Answer::Exact(None) | Answer::Coarser(None, _)) {
                    any = true;
                    self.open.push(Open {
                        c: child,
                        cursor,
                        to: splits_to,
                        from: Some(touching),
                        parent: Some((bound, error)),
                    });
                }
            }
            if any {
                // Past the rule's reach the derivation, and the quality policy's count, are broken.
                xpute_kit::ensure!(order.near < split_reach(c, self.ladder[splits_to as usize - 1]), ENOTRECOVERABLE, c.zoom());
            } else {
                from_step = 0;
            }
        }
        let splits_to = from_step.min(to);
        self.seen.insert(c, Seen { order, from: from_step, to });
        for k in self.tracked..self.ladder.len() {
            let at = k as u8;
            let add = if from_step <= at && at < to {
                self.counted[k] += 1;
                price.piece(c, order.near, self.ladder[k])
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
            let mut pieces = core::mem::take(&mut self.cuts[k].pieces);
            pieces.clear();
            if k >= self.tracked {
                for (&c, s) in &self.seen {
                    if s.from <= at && at < s.to {
                        pieces.push(c);
                    }
                }
            }
            pieces.sort_unstable();
            self.cuts[k] = Cut {
                tau: self.ladder[k],
                pieces,
                made: true,
            };
        }
        &self.cuts[k]
    }

    pub fn put(&mut self, k: usize) {
        xpute_kit::ensure!(k < self.ladder.len(), EINVAL);
        let k = k.min(self.ladder.len() - 1).max(self.tracked);
        self.cut_at(k);
        self.at = k;
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

    pub fn lacks(&self) -> &[Tile] {
        &self.lacks
    }
}

impl<Q: Query> Domain<Q> {
    fn bound(&self, c: Chunk) -> Answer<Option<Bound>> {
        self.query.bounds(c.node(), c.cell_zoom())
    }

    /// Measured from where in `c` anything stands: a bound, not the drawn
    /// surface, which on a coarse tile averages a peak away and would leave
    /// the chunk empty until the finer tile came. A bound errs nearer.
    pub fn order_of(&self, c: Chunk) -> Order {
        if let Some(s) = self.seen.get(&c) {
            return s.order;
        }
        let bound = self.bound(c).value().flatten().map_or(c.region(), |b| b.aabb);
        Order::of(&self.o, &bound)
    }

    pub fn pieces(&self) -> &[Chunk] {
        &self.in_force().pieces
    }

    pub fn observation(&self) -> &Observation {
        &self.o
    }
}
