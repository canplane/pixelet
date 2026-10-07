// pxl-kernel/world/pyramid.rs

//! The pyramid as the client holds it, for every layer on the ground: the
//! tiles held (residency.rs) and what an observation asks for of them.
//!
//! Asks are the observation's demand, as the estimator names it, merged with
//! the fallback's tiles and a destination's, the best reason per tile.
//! Nothing drawn enters them except the shared quality, which is decided
//! first. Where a tile marks a child as no tile (`Layer::occupancy`), its
//! ground is the tile's and the child is never asked for.

use pxl_spec::bake::earth_tile::EARTH_REACH;
use pxl_spec::bake::format::Terrain;
use pxl_spec::bake::poi_tile::PoiTile;
use xpute_guest::global::Global;

use crate::control::camera::controller::units_per_px_at;
use crate::io::demand::Category;
use crate::io::fetch::{body, next_tile, release, tile_settled, TileArrival};
use crate::io::sched::attempt_of;
use crate::layer::earth::{self, Earth};
use crate::layer::poi::MAJOR_UNITS_PER_PX;
use crate::layer::Layer;
use crate::sched::edge::world_edge;
use xpute_core::status::errno::Errno;
use xpute_guest::mem::heap::Block;

use std::collections::HashMap;

use pxl_spec::space::camera::eye::Observation;
use pxl_spec::space::camera::grid::Tau;
use pxl_spec::space::layer::lattice::Lattice;
use pxl_spec::space::layer::tile::{Tile, LEAF_ZOOM};

use crate::io::demand::Order;
use crate::io::demand::{Ask, Phases, Why};
use crate::io::failure::Failure;
use crate::io::sched::declare;
use crate::layer::residency::floor::{self, Fallback, Floor};
use crate::layer::residency::{Candidate, Residency, Victim, ZOOM_DEMAND_TILES};
use crate::world::chunk::quality;
use crate::world::view::observation::{observation, ByFootprints};
use crate::world::view::observer::Footprint;
use pxl_spec::space::coord::zoom::Zoom;

/// A layer as the view asks of it: its demand, held beside its records.
pub trait Observed: Layer {
    fn pyramid() -> &'static mut Pyramid<Self>;

    /// How far from the eye the layer draws anything; past it nothing of it
    /// is asked for.
    fn reach() -> f64 {
        f64::INFINITY
    }
}

static TERRAIN: Global<Pyramid<Terrain>> = Global::new();
static POI: Global<Pyramid<PoiTile>> = Global::new();
static EARTH: Global<Pyramid<Earth>> = Global::new();

impl Observed for Terrain {
    fn pyramid() -> &'static mut Pyramid<Terrain> {
        TERRAIN.get(Pyramid::default)
    }
}

impl Observed for PoiTile {
    fn pyramid() -> &'static mut Pyramid<PoiTile> {
        POI.get(Pyramid::default)
    }

    fn reach() -> f64 {
        MAJOR_UNITS_PER_PX / units_per_px_at(1.0)
    }
}

impl Observed for Earth {
    fn pyramid() -> &'static mut Pyramid<Earth> {
        EARTH.get(Pyramid::default)
    }

    fn reach() -> f64 {
        EARTH_REACH
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
    /// The published goal's.
    footprints: Vec<Footprint>,
    lacks: Vec<Tile>,
}

/// Evicts the tile last in view order.
struct Farthest<'a>(Option<&'a Observation>, Lattice);

impl Victim for Farthest<'_> {
    fn victim(&self, candidates: &[Candidate]) -> Option<usize> {
        let order = |c: &Candidate| self.0.map_or(Order::LAST, |o| crate::world::view::observation::order_of_tile(o, self.1, c.tile));
        (0..candidates.len()).max_by_key(|&i| order(&candidates[i]))
    }
}

pub struct Pyramid<L: Observed> {
    layer: core::marker::PhantomData<L>,
    observation: Option<Observation>,
    demand: Vec<Tile>,
    wants: Vec<(u64, Ask)>,
    last: Option<Seen>,
    /// What this turn reads, kept so a turn allocates nothing.
    now: Option<Seen>,
    /// What it reads moved since it last observed.
    moved: bool,
    /// Reused across observations so observing stops allocating once warm.
    asks: HashMap<Tile, Ask>,
}

impl<L: Observed> Default for Pyramid<L> {
    fn default() -> Self {
        Pyramid {
            layer: core::marker::PhantomData,
            observation: None,
            demand: Vec::new(),
            wants: Vec::new(),
            last: None,
            now: None,
            moved: false,
            asks: HashMap::with_capacity(Residency::<L>::FRAMES as usize),
        }
    }
}

impl<L: Observed> Pyramid<L> {
    pub fn set(&mut self, p: Tile, tile: L) {
        L::residency().set(p, tile);
    }

    pub fn answer_above(&mut self, p: Tile) {
        L::residency().answer_above(p);
    }

    pub fn take_refusals(&mut self) -> u32 {
        L::residency().take_refusals()
    }

    pub fn observation(&self) -> Option<&Observation> {
        self.observation.as_ref()
    }

    /// The floor's tiles as last observed.
    pub fn floor(&self) -> impl Iterator<Item = Tile> + Clone + '_ {
        self.last.iter().flat_map(|seen| seen.floor.iter().flatten().copied())
    }

    pub fn demand(&self) -> &[Tile] {
        &self.demand
    }

    /// Whether every tile the view needs has its answer.
    pub fn answered(&self) -> bool {
        self.asks.keys().all(|&p| L::residency().has_answer(p))
    }

    /// No object for `p`: the tile over it says its ground; at the source's top, `Layer::empty`.
    pub fn absent(&mut self, p: Tile) {
        let top = L::TOP.or(floor::top()).unwrap_or(L::LATTICE.top());
        match L::empty() {
            Some(empty) if p.zoom <= top => L::residency().set(p, empty),
            _ => L::residency().answer_above(p),
        }
    }

    pub fn admit(&mut self, p: Tile, receive: impl FnOnce(u32) -> Result<L, Failure>) -> Result<(), Failure> {
        L::residency().admit(p, &Farthest(self.observation.as_ref(), L::LATTICE), receive)
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
        estimator: &ByFootprints,
        top: Zoom,
    ) {
        let mut demand = core::mem::take(&mut self.demand);
        demand.clear();
        let mut asks = core::mem::take(&mut self.asks);
        asks.clear();
        let ask = |asks: &mut HashMap<Tile, Ask>, p: Tile, a: Ask| {
            asks.entry(p).and_modify(|b| *b = b.best(a)).or_insert(a);
        };
        let order = |o: &Observation, p: Tile| crate::world::view::observation::order_of_tile(o, L::LATTICE, p);
        if let Some(o) = o {
            if L::DEMANDS {
                estimator.requirements(o, &mut |p| demand.push(p));
            }
            demand.sort_unstable();
            demand.dedup();
            for &p in &demand {
                let at = L::residency().source(p).unwrap_or(p);
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
                .filter(|(p, _)| !L::residency().resident(**p) && !L::residency().settled(**p))
                .map(|(p, &a)| (p.subject(), a)),
        );
        L::residency().ask(&asks);
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
        let published = crate::world::chunk::domain::current();
        let own = |o: Observation| match L::TOP {
            Some(_) => Observation {
                reach: L::reach(),
                fog: f64::INFINITY,
                ..o
            },
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
            footprints: Vec::new(),
            lacks: Vec::new(),
        });
        (now.o, now.floor, now.destination, now.tau) = (o, floor_now, destination, tau);
        now.footprints.clear();
        now.footprints.extend_from_slice(published.map_or(&[], |d| d.footprints()));
        now.lacks.clear();
        now.lacks.extend_from_slice(published.map_or(&[], |d| d.lacks()));
        if core::mem::take(&mut self.moved) | (self.last.as_ref() != Some(&now)) {
            let line = destination.into_iter().flat_map(|p| std::iter::successors(Some(p), move |q| (q.zoom > top).then(|| q.parent())));
            self.observe(
                o.as_ref(),
                live.as_ref(),
                line,
                &ByFootprints {
                    tau,
                    lattice: L::LATTICE,
                    top,
                    footprints: published.map_or(&[], |d| d.footprints()),
                    lacks: published.map_or(&[], |d| d.lacks()),
                },
                top,
            );
            self.now = self.last.replace(now);
        } else {
            self.now = Some(now);
        }
        declare(L::CATEGORY, &self.wants);
    }

    /// What it reads moved: its own tiles', the ground's, or the objects'.
    pub fn moved(&mut self) {
        self.moved = true;
    }

    /// A layer the view does not show keeps what it holds and asks for nothing.
    pub fn idle(&mut self) {
        self.wants.clear();
        self.asks.clear();
        L::residency().ask(&self.asks);
        declare(L::CATEGORY, &[]);
        self.last = None;
    }
}

/// The pyramid tiles arrived, admitted against the view as many as the
/// turn's quota allows and at least one; each fetch settles here.
pub fn admit_arrived() {
    let mut first = true;
    while first || !world_edge().spent() {
        let Some(TileArrival {
            key,
            category,
            tile,
            block,
            mut phases,
        }) = next_tile()
        else {
            return;
        };
        first = false;
        // A key the request scheduler no longer holds is one a reset forgot.
        if attempt_of(key).is_none() {
            release(block);
            continue;
        }
        let result = match (category, block) {
            (Category::terrain, block) => arrived::<Terrain>(tile, block, &mut phases),
            (Category::poi, block) => arrived::<PoiTile>(tile, block, &mut phases),
            (Category::earth, None) => arrived::<Earth>(tile, None, &mut phases),
            (Category::earth, Some(b)) => {
                let mut framed = None;
                let result = Earth::pyramid().admit(tile, |record| {
                    let earth = Earth::receive(record, body(b), &mut phases)?;
                    framed = Some(record);
                    Ok(earth)
                });
                // The texels are kept until the GPU has them.
                earth::keep(framed, b);
                result
            }
            (_, block) => {
                release(block);
                Err(Errno::ENOTRECOVERABLE.into())
            }
        };
        tile_settled(key, result, phases);
    }
}

/// None is no object for `p`.
fn arrived<L: Observed>(p: Tile, block: Option<Block>, phases: &mut Phases) -> Result<(), Failure> {
    let Some(b) = block else {
        L::pyramid().absent(p);
        return Ok(());
    };
    let result = L::pyramid().admit(p, |record| L::receive(record, body(b), phases));
    release(Some(b));
    result
}
