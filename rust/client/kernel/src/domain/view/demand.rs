// pxl-kernel/domain/view/demand.rs

//! The pyramid as the client holds it, for every layer on the ground: the
//! tiles held (residency.rs) and what an observation asks for of them.
//!
//! Asks are the live eye's demand, as the estimator names it, merged with
//! the fallback's tiles and a destination's, the best reason per tile.
//! Nothing drawn enters them but the quality's τ the turn hands in, decided
//! first. Where a tile marks a
//! child as no tile (`Base::occupancy`), its ground is the tile's and the
//! child is never asked for.

use pxl_spec::bake::tile::earth::EARTH_REACH;
use pxl_spec::bake::tile::place::PlaceTile;
use pxl_spec::bake::tile::terrain::TerrainTile;
use xpute_guest::global::Global;

use crate::domain::layer::earth::Earth;
use crate::domain::layer::{Asks, Base};
use crate::sys::io::demand::Stream;
use crate::sys::io::fetch::{answer_settled, answered, body, next_answered, release, Answered};
use crate::sys::io::request::attempt_of;
use pxl_spec::bake::tile::division::DivisionTile;
use xpute_guest::mem::heap::Block;
use xpute_guest::sched::frame_budget::FrameBudget;
use xpute_kit::status::errno::Errno;

use fnv::FnvHashMap;

use pxl_spec::space::camera::eye::Observation;
use pxl_spec::space::camera::grid::Tau;
use pxl_spec::space::layer::tile::{Tile, LEAF_ZOOM};

use crate::domain::layer::residency::floor;
use crate::domain::layer::residency::{Lines, Residency};
use crate::domain::query::Walked;
use crate::domain::view::observation::{observation, ByEye};
use crate::sys::io::demand::Order;
use crate::sys::io::demand::{Ask, Phases, Why};
use crate::sys::io::failure::Failure;
use crate::sys::io::request::declare;
use crate::sys::io::source;
use pxl_spec::space::coord::zoom::Zoom;
use pxl_spec::space::hierarchy::morton::key_of;

/// A layer as the view asks of it: its demand, held beside its records.
pub trait Observed: Base {
    fn demand() -> &'static mut Demand<Self>;

    /// How far from the eye the layer draws anything; past it nothing of it
    /// is asked for.
    fn reach() -> f64 {
        f64::INFINITY
    }
}

static TERRAIN: Global<Demand<TerrainTile>> = Global::new();
static PLACE: Global<Demand<PlaceTile>> = Global::new();
static EARTH: Global<Demand<Earth>> = Global::new();
static DIVISION: Global<Demand<DivisionTile>> = Global::new();

impl Observed for TerrainTile {
    fn demand() -> &'static mut Demand<TerrainTile> {
        TERRAIN.get(Demand::default)
    }
}

impl Observed for PlaceTile {
    fn demand() -> &'static mut Demand<PlaceTile> {
        PLACE.get(Demand::default)
    }
}

impl Observed for DivisionTile {
    fn demand() -> &'static mut Demand<DivisionTile> {
        DIVISION.get(Demand::default)
    }
}

impl Observed for Earth {
    fn demand() -> &'static mut Demand<Earth> {
        EARTH.get(Demand::default)
    }

    fn reach() -> f64 {
        EARTH_REACH
    }
}

/// Everything the asks depend on, as read; two turns agreeing on all of it
/// ask for the same tiles, so the second decides nothing.
#[derive(Clone, Copy, PartialEq)]
struct Seen {
    o: Option<Observation>,
    destination: Option<Tile>,
    tau: Tau,
}

pub struct Demand<L: Observed> {
    layer: core::marker::PhantomData<L>,
    observation: Option<Observation>,
    wants: Vec<(u64, Ask)>,
    last: Option<Seen>,
    /// The floor's tiles as last observed.
    floor: Vec<Tile>,
    /// The asks found absent in an observation, written once all are found
    /// (`observe`); reused across observations.
    gone: Vec<Tile>,
    /// What it reads moved since it last observed.
    moved: bool,
    /// Where an observation builds the estimator's tiles and its asks, kept
    /// so observing stops allocating once warm. Empty between observations:
    /// what was asked is the residency's (`Residency::ask`).
    demand: Vec<Tile>,
    asks: FnvHashMap<Tile, Ask>,
    /// The residency's revision as it idled it (`idle`), none while it steps.
    idled: Option<u32>,
}

impl<L: Observed> Default for Demand<L> {
    fn default() -> Self {
        Demand {
            layer: core::marker::PhantomData,
            observation: None,
            demand: Vec::new(),
            wants: Vec::new(),
            last: None,
            floor: Vec::new(),
            gone: Vec::new(),
            moved: false,
            asks: FnvHashMap::with_capacity_and_hasher(Residency::<L>::FRAMES as usize, Default::default()),
            idled: None,
        }
    }
}

impl<L: Observed> Demand<L> {
    /// The floor's tiles as last observed.
    pub fn floor(&self) -> impl Iterator<Item = Tile> + Clone + '_ {
        self.floor.iter().copied()
    }

    /// No object for `p`: the tile over it says its ground; at the layer's top (`Base::top`), `Base::empty`.
    pub fn absent(&mut self, p: Tile) {
        match L::empty() {
            Some(empty) if L::top().is_some_and(|top| p.zoom <= top) => L::residency().set(p, empty),
            _ => L::residency().answer_above(p),
        }
    }

    pub fn admit(&mut self, p: Tile, receive: impl FnOnce(u32) -> Result<L, Failure>) -> Result<(), Failure> {
        // The tile last in view order goes, of two as late the greater.
        let o = self.observation.as_ref();
        let farthest = |tiles: &mut dyn Iterator<Item = Tile>| {
            let mut walked = Walked::default();
            tiles.max_by_key(|&t| (o.map_or(Order::LAST, |o| crate::domain::view::observation::order_of_tile(o, L::LATTICE, t, &mut walked)), t))
        };
        L::residency().admit(p, farthest, receive)
    }

    /// Builds the asks for `o`: the estimator's demand, the floor and the
    /// destination's line.
    #[allow(clippy::too_many_arguments)]
    pub fn observe(&mut self, o: Option<&Observation>, destination: impl Iterator<Item = Tile>, estimator: &ByEye, top: Zoom) {
        let mut demand = core::mem::take(&mut self.demand);
        let mut asks = core::mem::take(&mut self.asks);
        let ask = |asks: &mut FnvHashMap<Tile, Ask>, p: Tile, a: Ask| {
            asks.entry(p).and_modify(|b| *b = b.best(a)).or_insert(a);
        };
        // One walk down the pyramid in key order, so each tile's line, its
        // own and the ground's it is ordered by, steps from its neighbour's.
        let (mut lines, mut ground) = (Lines::default(), Walked::default());
        let mut order = |o: &Observation, p: Tile| crate::domain::view::observation::order_of_tile(o, L::LATTICE, p, &mut ground);
        if let Some(o) = o {
            if matches!(L::ASKS, Asks::View) {
                estimator.requirements(o, &mut |p| demand.push(p));
            }
            demand.sort_unstable_by_key(|p| key_of::<2>(p.zoom, p.at));
            demand.dedup();
            for &p in &demand {
                let at = L::residency().seek(&mut lines, p).source.unwrap_or(p);
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
            xpute_kit::ensure!(read.iter().all(|&n| n <= Residency::<L>::ZOOM_DEMAND_TILES), ENOTRECOVERABLE, *read.iter().max().unwrap_or(&0));
            if !matches!(L::ASKS, Asks::Line) {
                self.floor.clear();
                floor::floor(o, top, |p| {
                    self.floor.push(p);
                    ask(&mut asks, p, Ask { why: Why::Floor, order: order(o, p) });
                });
                xpute_kit::ensure!(self.floor.len() <= floor::tiles(L::LATTICE) as usize, ENOTRECOVERABLE, self.floor.len());
            }
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
        xpute_kit::ensure!(
            named <= if matches!(L::ASKS, Asks::Floor) { 0 } else { Residency::<L>::DESTINATION_TILES + L::PINNED },
            ENOTRECOVERABLE,
            named
        );

        L::residency().ask(&asks);
        // The asks in key order, in the demand's room (nothing reads it
        // between observations): each line read steps from its neighbour's
        // (`seek`), not from the top.
        demand.clear();
        demand.extend(asks.keys().copied());
        demand.sort_unstable_by_key(|p| p.key());
        let unanswered = |p: Tile, lines: &mut Lines| {
            let r = L::residency();
            !r.resident(p) && (!r.seek(lines, p).settled || r.again_held(p))
        };
        // A tile no served shard is over or under is answered as its absence
        // is, and never asked for: every ask judged against the residency as
        // the asks left it, before any absence is written, so which are
        // absent depends on no order they are taken in.
        if let Some(layer) = L::STREAM.bake() {
            let (mut gone, mut lines) = (core::mem::take(&mut self.gone), Lines::default());
            gone.clear();
            gone.extend(demand.iter().copied().filter(|&p| unanswered(p, &mut lines) && !source::serves(layer, p)));
            for &p in &gone {
                self.absent(p);
            }
            self.gone = gone;
        }
        // By subject, as a stream's wants are said (sys/io/request.rs `declare`):
        // key order, a subject being its key's high half, after every absence
        // is written.
        let mut lines = Lines::default();
        self.wants.clear();
        for &p in &demand {
            if unanswered(p, &mut lines) {
                self.wants.push((p.subject(), asks[&p]));
            }
        }
        asks.clear();
        self.asks = asks;
        demand.clear();
        self.demand = demand;

        self.observation = o.copied();
    }

    /// The layer's turn, from the live eye: the eye that is coming, which
    /// every cast reads as it arrives. Quality is decided first so asks match
    /// what is drawn. Observes again whenever anything in `Seen` changed.
    ///
    /// `destination` is asked for with its line even with no observation yet:
    /// at the first fix the camera waits for that ground; and `pinned`, the
    /// tiles a reader that takes no coarser answer declares (`Base::PINNED`),
    /// as the destination is. A layer with a top of its own is observed out
    /// to its own reach.
    pub fn step(&mut self, destination: Option<Tile>, pinned: &[Tile], tau: Tau) {
        let Some(top) = L::top() else { return };
        let o = observation().map(|o| match L::TOP {
            Some(_) => Observation { reach: L::reach(), ..o },
            None => o,
        });
        if o.is_none() && destination.is_none() && pinned.is_empty() {
            return;
        }
        self.idled = None;
        let tau = if o.is_some() { tau } else { Tau::FINEST };
        let now = Seen { o, destination, tau };
        if core::mem::take(&mut self.moved) | (self.last != Some(now)) {
            let line = destination
                .into_iter()
                .flat_map(|p| std::iter::successors(Some(p), move |q| (q.zoom > top).then(|| q.parent())))
                .chain(pinned.iter().copied());
            self.observe(o.as_ref(), line, &ByEye { tau, lattice: L::LATTICE, top }, top);
            self.last = Some(now);
        }
        declare(L::STREAM, &self.wants);
    }

    /// What it reads moved: its own tiles', the ground's, or the objects'.
    pub fn moved(&mut self) {
        self.moved = true;
    }

    /// A layer the view does not show keeps what it holds and asks for
    /// nothing: its residency let go of its asks again only where what it
    /// holds moved since it last was (`Residency::revision`), as asking
    /// nothing of what is unchanged changes nothing.
    pub fn idle(&mut self) {
        if self.idled.is_some_and(|r| r == L::residency().revision()) {
            return;
        }
        self.wants.clear();
        L::residency().ask(&self.asks);
        declare(L::STREAM, &[]);
        self.last = None;
        self.idled = Some(L::residency().revision());
    }
}

/// The pyramid tiles arrived, admitted against the view as a pass of the
/// tick's budget, so the visible phase's hold on the content share binds it,
/// and at least one; each fetch settles here. What the pass leaves stays
/// queued, and a turn is asked for it.
pub fn admit_arrived(budget: &FrameBudget) {
    const OF: [Stream; 4] = [Stream::terrain, Stream::place, Stream::earth, Stream::division];
    let pass = budget.run(
        0..answered(&OF),
        |_| {
            let Some(Answered {
                key,
                stream,
                subject,
                block,
                mut phases,
            }) = next_answered(&OF)
            else {
                return false;
            };
            let tile = Tile::of_subject(subject);
            // A key the request scheduler no longer holds is one a reset forgot.
            if attempt_of(key).is_none() {
                release(block);
                return false;
            }
            let result = match (stream, block) {
                (Stream::terrain, block) => arrived::<TerrainTile>(tile, block, &mut phases),
                (Stream::place, block) => arrived::<PlaceTile>(tile, block, &mut phases),
                (Stream::division, block) => arrived::<DivisionTile>(tile, block, &mut phases),
                (Stream::earth, block) => arrived::<Earth>(tile, block, &mut phases),
                (_, block) => {
                    release(block);
                    Err(Errno::ENOTRECOVERABLE.into())
                }
            };
            answer_settled(key, result, phases);
            true
        },
        None,
    );
    if pass.stopped_early {
        crate::sys::sched::edge::request_world_tick();
    }
}

/// None is no object for `p`.
fn arrived<L: Observed>(p: Tile, block: Option<Block>, phases: &mut Phases) -> Result<(), Failure> {
    let Some(b) = block else {
        L::demand().absent(p);
        return Ok(());
    };
    let mut framed = None;
    let result = L::demand().admit(p, |record| {
        let tile = L::receive(p, record, body(b), phases)?;
        framed = Some(record);
        Ok(tile)
    });
    L::keep(framed, b);
    result
}
