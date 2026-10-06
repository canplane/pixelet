// pxl-kernel/world/pyramid.rs

//! The pyramid as the client holds it, for every layer on the ground: the
//! tiles held (residency.rs) and what an observation asks for of them.
//!
//! Asks are the observation's demand, as the estimator names it, merged with
//! the fallback's tiles and a destination's, the best reason per tile.
//! Nothing drawn enters them except the shared quality, which is decided
//! first. Where a tile marks a child as no tile (`Layer::occupancy`), its
//! ground is the tile's and the child is never asked for.

pub mod chunk;
pub mod domain;

use std::collections::HashMap;

use pxl_spec::space::camera::grid::Tau;
use pxl_spec::space::camera::eye::Observation;
use pxl_spec::space::layer::demand::{Bounds, DemandEstimator, Requirement, Storage as _, Tiled};
use pxl_spec::space::layer::lattice::Lattice;
use pxl_spec::bake::format::Occupancy;
use pxl_spec::space::layer::tile::{Tile, LEAF_ZOOM};

use crate::io::demand::{Ask, Category, Phases, Why};
use crate::io::sched::declare;
use crate::io::failure::Failure;
use crate::world::observation::{ground_known, observation, ByFootprints, Known, Order};
use crate::world::observer::Footprint;
use crate::world::quality;
use crate::world::residency::floor::{self, Fallback, Floor};
use crate::world::residency::{Candidate, Residency, Victim, ZOOM_DEMAND_TILES};
use pxl_spec::space::coord::zoom::Zoom;

pub trait Layer: Clone + 'static {
    const CATEGORY: Category;

    /// Which of its children are tiles of their own, as the bake wrote it.
    fn occupancy(&self) -> Occupancy;

    /// Its tiles' cells, which say what of the grid a tile at each zoom draws.
    const LATTICE: Lattice;

    /// False for a layer that holds only the floor: it asks for no miss and no
    /// destination, and its residency is sized so.
    const DEMANDS: bool = true;

    /// The coarsest zoom the layer's own source supplies whole, where one
    /// differs from the ground's declared top (`floor::top`). Its floor is
    /// then that zoom's whole reach about the eye: it is drawn past where
    /// the ground stops.
    const TOP: Option<Zoom> = None;

    /// How far from the eye the layer draws anything; past it nothing of it
    /// is asked for.
    fn reach() -> f64 {
        f64::INFINITY
    }

    fn pyramid() -> &'static mut Pyramid<Self>;

    fn receive(record: u32, pkt: &[u8], phases: &mut Phases) -> Result<Self, Failure>;

    /// A tile with nothing in it, which answers an absent tile at the top.
    fn empty() -> Option<Self> {
        None
    }

    fn changed(_p: Tile, _present: bool) {}

    fn tiles(r: &Requirement, each: &mut dyn FnMut(Tile)) {
        Tiled.tiles(r, each);
    }
}

/// Everything the asks depend on, as read; two turns agreeing on all of it
/// ask for the same tiles, so the second decides nothing.
#[derive(PartialEq)]
struct Seen {
    o: Option<Observation>,
    floor: [Option<Tile>; Floor::TILES as usize],
    destination: Option<Tile>,
    tau: Tau,
    /// The layer's own tiles known, and the ground's.
    own: Vec<(Tile, bool)>,
    terrain: Vec<(Tile, bool)>,
    /// The published goal's.
    footprints: Vec<Footprint>,
    lacks: Vec<Requirement>,
}

/// Evicts the tile last in view order.
struct Farthest<'a>(Option<&'a Observation>, Lattice);

impl Victim for Farthest<'_> {
    fn victim(&self, candidates: &[Candidate]) -> Option<usize> {
        let order = |c: &Candidate| self.0.map_or(Order::LAST, |o| Order::of_tile(o, self.1, c.tile));
        (0..candidates.len()).max_by_key(|&i| order(&candidates[i]))
    }
}

pub struct Pyramid<L: Layer> {
    residency: Residency<L>,
    observation: Option<Observation>,
    demand: Vec<Tile>,
    wants: Vec<(u64, Ask)>,
    last: Option<Seen>,
    /// What this turn reads, kept so a turn allocates nothing.
    now: Option<Seen>,
    /// Objects were written since it last observed.
    objects_written: bool,
    /// Reused across observations so observing stops allocating once warm.
    asks: HashMap<Tile, Ask>,
}

impl<L: Layer> Default for Pyramid<L> {
    fn default() -> Self {
        Pyramid {
            residency: Residency::default(),
            observation: None,
            demand: Vec::new(),
            wants: Vec::new(),
            last: None,
            now: None,
            objects_written: false,
            asks: HashMap::with_capacity(Residency::<L>::FRAMES as usize),
        }
    }
}

impl<L: Layer> Pyramid<L> {
    pub fn residency(&self) -> &Residency<L> {
        &self.residency
    }

    pub fn set(&mut self, p: Tile, tile: L) {
        self.residency.set(p, tile);
    }

    pub fn answer_above(&mut self, p: Tile) {
        self.residency.answer_above(p);
    }

    pub fn take_refusals(&mut self) -> u32 {
        self.residency.take_refusals()
    }

    pub fn observation(&self) -> Option<&Observation> {
        self.observation.as_ref()
    }

    /// The floor's tiles as last observed.
    pub fn floor(&self) -> impl Iterator<Item = Tile> + '_ {
        self.last.iter().flat_map(|seen| seen.floor.iter().flatten().copied())
    }

    pub fn demand(&self) -> &[Tile] {
        &self.demand
    }

    /// Whether every tile the view needs has its answer.
    pub fn answered(&self) -> bool {
        self.asks.keys().all(|&p| self.residency.has_answer(p))
    }

    pub fn adopt(&mut self, p: Tile, pkt: &[u8], phases: &mut Phases) -> Result<(), Failure> {
        self.admit(p, |record| L::receive(record, pkt, phases))
    }

    /// No object for `p`: the tile over it says its ground; at the source's top, `Layer::empty`.
    pub fn absent(&mut self, p: Tile) {
        let top = L::TOP.or(floor::top()).unwrap_or(L::LATTICE.top());
        match L::empty() {
            Some(empty) if p.zoom <= top => self.residency.set(p, empty),
            _ => self.residency.answer_above(p),
        }
    }

    pub fn admit(&mut self, p: Tile, receive: impl FnOnce(u32) -> Result<L, Failure>) -> Result<(), Failure> {
        self.residency.admit(p, &Farthest(self.observation.as_ref(), L::LATTICE), receive)
    }

    /// Builds the asks for `o`: the estimator's demand, the floor and the
    /// destination's line.
    #[allow(clippy::too_many_arguments)]
    pub fn observe(
        &mut self,
        o: Option<&Observation>,
        // the floor's eye, if not `o`'s
        floor_at: Option<&Observation>,
        destination: impl Iterator<Item = Tile>,
        bounds: &impl Bounds,
        estimator: &impl DemandEstimator,
        top: Zoom,
    ) {
        let mut demand = core::mem::take(&mut self.demand);
        demand.clear();
        let mut asks = core::mem::take(&mut self.asks);
        asks.clear();
        let ask = |asks: &mut HashMap<Tile, Ask>, p: Tile, a: Ask| {
            asks.entry(p).and_modify(|b| *b = b.best(a)).or_insert(a);
        };
        let order = |o: &Observation, p: Tile| Order::of_region(o, bounds, L::LATTICE.grid_cell(p.zoom), p.region());
        if let Some(o) = o {
            if L::DEMANDS {
                estimator.requirements(o, bounds, &mut |r| L::tiles(&r, &mut |p| demand.push(p)));
            }
            demand.sort_unstable();
            demand.dedup();
            for &p in &demand {
                let at = self.residency.source(p).unwrap_or(p);
                ask(&mut asks, at, Ask { why: Why::Demand, order: order(o, p) });
                // A line already asked is asked to the top.
                for q in std::iter::successors(Some(at), |q| (q.zoom > top).then(|| q.parent())).skip(1) {
                    if asks.get(&q).is_some_and(|a| a.why <= Why::Ancestor) {
                        break;
                    }
                    ask(
                        &mut asks,
                        q,
                        Ask {
                            why: Why::Ancestor,
                            order: order(o, q),
                        },
                    );
                }
            }
            // More demanded than the structure's bound means the capacity is broken too:
            // a miss's line has ground within its reach, so it stays in each zoom's ring.
            let mut read = [0u32; LEAF_ZOOM as usize + 1];
            for p in asks.keys() {
                read[p.zoom as usize] += 1;
            }
            xpute_core::ensure!(read.iter().all(|&n| n <= ZOOM_DEMAND_TILES), ENOTRECOVERABLE, *read.iter().max().unwrap_or(&0));
            let mut floor = 0u32;
            let f = floor_at.unwrap_or(o);
            Floor::floor(f, top, |p| {
                floor += 1;
                ask(&mut asks, p, Ask { why: Why::Floor, order: order(o, p) });
            });
            xpute_core::ensure!(floor <= Floor::TILES, ENOTRECOVERABLE, floor);
        }
        // The whole line, not only the finest tile: which one says the ground is
        // known only once those above are, and asking one at a time would cost a
        // round trip per zoom.
        let mut named = 0u32;
        for p in destination {
            named += 1;
            ask(
                &mut asks,
                p,
                Ask {
                    why: Why::Destination,
                    order: Order::FIRST,
                },
            );
        }
        xpute_core::ensure!(named <= if L::DEMANDS { Residency::<L>::DESTINATION_TILES } else { 0 }, ENOTRECOVERABLE, named);


        self.wants.clear();
        self.wants.extend(
            asks.iter()
                .filter(|(p, _)| !self.residency.resident(**p) && !self.residency.settled(**p))
                .map(|(p, &a)| (p.subject(), a)),
        );
        self.residency.ask(&asks);
        self.asks = asks;

        self.observation = o.copied();
        self.demand = demand;
    }

    pub fn wants(&self) -> &[(u64, Ask)] {
        &self.wants
    }

    /// The layer's turn. Quality is decided first so asks match what is drawn.
    /// Observes again whenever anything in `Seen` changed.
    ///
    /// `destination` is asked for with its line even with no observation yet:
    /// at the first fix the camera waits for that ground. `far` cuts the
    /// observation where the layer stops being drawn.
    pub fn step(&mut self, destination: Option<Tile>) {
        // Nothing coarser than the layer's own top is drawn, whatever the source holds.
        let Some(top) = L::TOP.or(floor::top()).map(|t| t.max(L::LATTICE.top())) else { return };
        // demand follows the published goal
        let published = domain::current();
        let own = |o: Observation| match L::TOP {
            Some(_) => Observation { reach: L::reach(), fog: f64::INFINITY, ..o },
            None => o,
        };
        let live = observation().map(own);
        let seen_from = live.map(|live| published.map_or(live, |d| own(*d.observation())));
        // the floor follows the live eye, so a jump asks for it at once
        let mut floor_now = [None; Floor::TILES as usize];
        if let Some(l) = &live {
            let mut n = 0;
            Floor::floor(l, top, |p| {
                if n < floor_now.len() {
                    floor_now[n] = Some(p);
                }
                n += 1;
            });
        }
        if seen_from.is_none() && destination.is_none() {
            return;
        }
        let tau = match seen_from {
            Some(_) => quality::effective().tau(),
            None => Tau::FINEST,
        };
        let o = seen_from.map(|o| Observation { reach: o.reach.min(L::reach()), ..o });
        let mut now = self.now.take().unwrap_or_else(|| Seen {
            o: None,
            floor: [None; Floor::TILES as usize],
            destination: None,
            tau,
            own: Vec::new(),
            terrain: Vec::new(),
            footprints: Vec::new(),
            lacks: Vec::new(),
        });
        (now.o, now.floor, now.destination, now.tau) = (o, floor_now, destination, tau);
        self.residency.known(&mut now.own);
        ground_known(&mut now.terrain);
        now.footprints.clear();
        now.footprints.extend_from_slice(published.map_or(&[], |d| d.footprints()));
        now.lacks.clear();
        now.lacks.extend_from_slice(published.map_or(&[], |d| d.lacks()));
        if core::mem::take(&mut self.objects_written) | (self.last.as_ref() != Some(&now)) {
            let line = destination.into_iter().flat_map(|p| std::iter::successors(Some(p), move |q| (q.zoom > top).then(|| q.parent())));
            self.observe(
                o.as_ref(),
                live.as_ref(),
                line,
                &Known,
                &ByFootprints {
                    tau,
                    lattice: L::LATTICE,
                    top,
                    footprints: published.map_or(&[], |d| d.footprints()),
                    lacks: published.map_or(&[], |d| d.lacks()),
                },
                top,
            );
            // Observing can let tiles go: record what is known after it.
            self.residency.known(&mut now.own);
            ground_known(&mut now.terrain);
            self.now = self.last.replace(now);
        } else {
            self.now = Some(now);
        }
        declare(L::CATEGORY, &self.wants);
    }

    pub fn objects_written(&mut self) {
        self.objects_written = true;
    }

    /// A layer the view does not show keeps what it holds and asks for nothing.
    pub fn idle(&mut self) {
        self.wants.clear();
        self.asks.clear();
        self.residency.ask(&self.asks);
        declare(L::CATEGORY, &[]);
        self.last = None;
    }
}
