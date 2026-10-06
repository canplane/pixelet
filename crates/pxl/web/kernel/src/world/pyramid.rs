// pxl-kernel/world/pyramid.rs

//! The pyramid as the client holds it, for every layer on the ground: the
//! tiles held (residency.rs) and what an observation asks for of them.
//!
//! Asks are the observation's demand, as the estimator names it, merged with
//! the fallback's tiles, a destination's and speculation, the best reason per
//! tile. Nothing drawn enters them except the shared quality, which is
//! decided first. Where a tile settles a child's ground (`Folds::settles`),
//! the child is never asked for.

pub mod chunk;
pub mod domain;
pub mod read;

use std::collections::HashMap;

use pxl_spec::space::lod::Tau;
use pxl_spec::space::observation::{Bounds, DemandEstimator, Observation, Requirement, Storage as _, Tiled};
use pxl_spec::space::pyramid::{Folds, Lattice, Tile, LEAF_Z, TOP_Z};

use crate::io::demand::{Ask, Category, Phases, Why};
use crate::io::sched::declare;
use crate::param;
use crate::sched::profiler::record::Failure;
use crate::world::observation::{heights_revision, observation, ByFootprints, Known, Order};
use crate::world::prefetch::{NoPrefetch, Prefetch};
use crate::world::quality;
use crate::world::residency::floor::{self, Fallback, Floor};
use crate::world::residency::{Candidate, Residency, Victim, DESTINATION_TILES, ZOOM_DEMAND_TILES};
use pxl_spec::space::zoom::Zoom;

pub trait Layer: Folds + Clone + 'static {
    const CATEGORY: Category;

    /// Its tiles' cells, which say what of the grid a tile at each zoom draws.
    const LATTICE: Lattice;

    /// False for a layer that holds only the floor: it asks for no miss, no
    /// destination and nothing speculative, and its residency is sized so.
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

/// Everything the asks depend on; two turns agreeing on all of it ask for
/// the same tiles, so the second decides nothing.
type Seen = (Option<Observation>, [Option<Tile>; Floor::TILES as usize], Option<Tile>, u32, (u32, u32), Tau, u32);

/// No speculation: the view converges to exact on misses alone, and
/// speculation would only buy latency.
const PREFETCH: NoPrefetch = NoPrefetch;

/// Evicts unhinted tiles before hinted ones, then the one last in view order.
struct Farthest<'a>(Option<&'a Observation>, Lattice);

impl Victim for Farthest<'_> {
    fn victim(&self, candidates: &[Candidate]) -> Option<usize> {
        let order = |c: &Candidate| self.0.map_or(Order::LAST, |o| Order::of_tile(o, self.1, c.tile));
        (0..candidates.len()).max_by_key(|&i| (!candidates[i].hinted, order(&candidates[i])))
    }
}

pub struct Pyramid<L: Layer> {
    residency: Residency<L>,
    observation: Option<Observation>,
    demand: Vec<Tile>,
    wants: Vec<(u64, Ask)>,
    last: Option<Seen>,
    /// Reused across observations so observing stops allocating once warm.
    asks: HashMap<Tile, Ask>,
    hints: Vec<(Order, Tile)>,
}

impl<L: Layer> Default for Pyramid<L> {
    fn default() -> Self {
        Pyramid {
            residency: Residency::default(),
            observation: None,
            demand: Vec::new(),
            wants: Vec::new(),
            last: None,
            asks: HashMap::with_capacity(Residency::<L>::ASKS as usize),
            hints: Vec::new(),
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
        self.last.iter().flat_map(|seen| seen.1.iter().flatten().copied())
    }

    pub fn demand(&self) -> &[Tile] {
        &self.demand
    }

    /// Whether every tile the view needs has its answer; speculation aside.
    pub fn answered(&self) -> bool {
        self.asks.iter().all(|(&p, a)| a.why == Why::Hint || self.residency.has_answer(p))
    }

    pub fn adopt(&mut self, p: Tile, pkt: &[u8], phases: &mut Phases) -> Result<(), Failure> {
        self.admit(p, |record| L::receive(record, pkt, phases))
    }

    /// No object for `p`: the tile over it says its ground; at the source's top, `Layer::empty`.
    pub fn absent(&mut self, p: Tile) {
        let top = L::TOP.or(floor::top()).unwrap_or(TOP_Z);
        match L::empty() {
            Some(empty) if p.zoom <= top => self.residency.set(p, empty),
            _ => self.residency.answer_above(p),
        }
    }

    pub fn admit(&mut self, p: Tile, receive: impl FnOnce(u32) -> Result<L, Failure>) -> Result<(), Failure> {
        self.residency.admit(p, &Farthest(self.observation.as_ref(), L::LATTICE), receive)
    }

    /// Builds the asks for `o`: the estimator's demand, the floor, the
    /// destination's line, and speculation up to `allowance`. Speculation
    /// only takes frames left over after everything hard and demanded, so
    /// nothing is fetched just to find no frame.
    #[allow(clippy::too_many_arguments)]
    pub fn observe(
        &mut self,
        o: Option<&Observation>,
        // the floor's eye, if not `o`'s
        floor_at: Option<&Observation>,
        destination: impl Iterator<Item = Tile>,
        bounds: &impl Bounds,
        estimator: &impl DemandEstimator,
        prefetch: &impl Prefetch,
        tau: Tau,
        allowance: u32,
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
                for q in std::iter::successors(Some(at), |q| (q.zoom > o.top).then(|| q.parent())).skip(1) {
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
            let mut read = [0u32; LEAF_Z as usize + 1];
            for p in asks.keys() {
                read[p.zoom as usize] += 1;
            }
            xpute_core::ensure!(read.iter().all(|&n| n <= ZOOM_DEMAND_TILES), ENOTRECOVERABLE, *read.iter().max().unwrap_or(&0));
            let mut floor = 0u32;
            let f = floor_at.unwrap_or(o);
            Floor::floor(f, |p| {
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
        xpute_core::ensure!(named <= if L::DEMANDS { DESTINATION_TILES } else { 0 }, ENOTRECOVERABLE, named);

        let mut hints = core::mem::take(&mut self.hints);
        hints.clear();
        if let (Some(o), true) = (o, L::DEMANDS) {
            let needed = asks.keys().filter(|&&p| self.residency.takes_frame(p)).count() as u32;
            let mut room = Residency::<L>::FRAMES.saturating_sub(needed).min(allowance);
            prefetch.speculate(o, tau, &mut |p| {
                if !asks.contains_key(&p) {
                    hints.push((order(o, p), p));
                }
            });
            hints.sort_unstable();
            hints.dedup_by_key(|h| h.1);
            // Settled or empty tiles take no frame but still count against `ASKS`.
            let mut tiles = Residency::<L>::ASKS.saturating_sub(asks.len() as u32);
            for &(order, p) in &hints {
                let new = u32::from(self.residency.takes_frame(p));
                if new > room || tiles == 0 {
                    break;
                }
                room -= new;
                tiles -= 1;
                ask(&mut asks, p, Ask { why: Why::Hint, order });
            }
        }

        self.wants.clear();
        self.wants.extend(
            asks.iter()
                .filter(|(p, _)| !self.residency.resident(**p) && !self.residency.settled(**p))
                .map(|(p, &a)| (p.subject(), a)),
        );
        self.residency.ask(&asks);
        (self.asks, self.hints) = (asks, hints);

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
        let Some(top) = L::TOP.or(floor::top()) else { return };
        // demand follows the published goal
        let published = domain::current();
        let own = |o: Observation| match L::TOP {
            Some(top) => Observation { top, far: L::reach(), ..o },
            None => o,
        };
        let live = observation().map(own);
        let seen_from = live.map(|live| published.map_or(live, |d| own(*d.observation())));
        // the floor follows the live eye, so a jump asks for it at once
        let mut floor_now = [None; Floor::TILES as usize];
        if let Some(l) = &live {
            let mut n = 0;
            Floor::floor(l, |p| {
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
        let o = seen_from.map(|o| Observation { far: o.far.min(L::reach()), ..o });
        let seen = (o, floor_now, destination, self.residency.revision(), heights_revision(), tau, domain::revision());
        if self.last != Some(seen) {
            let line = destination.into_iter().flat_map(|p| std::iter::successors(Some(p), move |q| (q.zoom > top).then(|| q.parent())));
            self.observe(
                o.as_ref(),
                live.as_ref(),
                line,
                &Known,
                &ByFootprints {
                    tau,
                    lattice: L::LATTICE,
                    footprints: published.map_or(&[], |d| d.footprints()),
                    lacks: published.map_or(&[], |d| d.lacks()),
                },
                &PREFETCH,
                tau,
                param::HINT_ALLOWANCE,
            );
            // Observing can let tiles go, moving the revision: record the state after it.
            self.last = Some((o, floor_now, destination, self.residency.revision(), heights_revision(), tau, domain::revision()));
        }
        declare(L::CATEGORY, &self.wants);
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
