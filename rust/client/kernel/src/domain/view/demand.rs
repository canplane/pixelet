// pxl-kernel/domain/view/demand.rs

//! The pyramid as the client holds it, for every layer on the ground: the
//! tiles held (tile_cache.rs) and what an observation asks for of them.
//!
//! Asks are the live eye's demand, as the estimator names it, merged with
//! the fallback's tiles and a destination's, the best reason per tile.
//! Nothing drawn enters them: the camera grid is an angle. Where a tile marks
//! a child as no tile (`Base::occupancy`), its ground is the tile's and the
//! child is never asked for.

use pxl_spec::bake::tile::earth::EARTH_DIST;
use pxl_spec::bake::tile::place::PlaceTile;
use pxl_spec::bake::tile::terrain::TerrainTile;
use xpute_guest::global::Global;

use crate::domain::layer::earth::Earth;
use crate::domain::layer::object::repository;
use crate::domain::layer::{AskScope, Base};
use crate::sys::io::demand::Stream;
use crate::sys::io::fetch::{answer_cnt, body, on_answer_settled, release, take_answer, Answer};
use crate::sys::io::request::attempt_of;
use pxl_spec::bake::tile::division::DivisionTile;
use xpute_guest::mem::heap::Block;
use xpute_guest::sched::pass::Pass;
use xpute_kit::status::errno::Errno;

use fnv::FnvHashMap;

use pxl_spec::space::camera::eye::Observation;
use pxl_spec::space::layer::tile::{Tile, LEAF_ZOOM};

use crate::domain::layer::tile_cache::floor;
use crate::domain::layer::tile_cache::{Lines, TileCache};
use crate::domain::query::ColumnWalk;
use crate::domain::view::observation;
use crate::domain::view::observation::{observation, ByEye};
use crate::sys::io::demand::Order;
use crate::sys::io::demand::{Ask, Phases, Priority};
use crate::sys::io::failure::Failure;
use crate::sys::io::request::declare;
use crate::sys::io::source;
use crate::sys::sched::edge;
use pxl_spec::space::coord::zoom::{Zoom, SHARD_ZOOM};
use pxl_spec::space::hierarchy::morton::key_of;
use pxl_spec::space::layer::demand::tiles_within;
use pxl_spec::space::layer::lattice::GROUND;

/// A layer as the view asks of it: its demand, held beside its records.
pub trait ObservedLayer: Base {
    fn demand() -> &'static mut Demand<Self>;

    /// How far from the eye the layer draws anything; past it nothing of it
    /// is asked for.
    fn demand_dist() -> f64 {
        f64::INFINITY
    }
}

static TERRAIN: Global<Demand<TerrainTile>> = Global::new();
static PLACE: Global<Demand<PlaceTile>> = Global::new();
static EARTH: Global<Demand<Earth>> = Global::new();
static DIVISION: Global<Demand<DivisionTile>> = Global::new();

impl ObservedLayer for TerrainTile {
    fn demand() -> &'static mut Demand<TerrainTile> {
        TERRAIN.get(Demand::default)
    }
}

impl ObservedLayer for PlaceTile {
    fn demand() -> &'static mut Demand<PlaceTile> {
        PLACE.get(Demand::default)
    }
}

impl ObservedLayer for DivisionTile {
    fn demand() -> &'static mut Demand<DivisionTile> {
        DIVISION.get(Demand::default)
    }
}

impl ObservedLayer for Earth {
    fn demand() -> &'static mut Demand<Earth> {
        EARTH.get(Demand::default)
    }

    fn demand_dist() -> f64 {
        EARTH_DIST
    }
}

/// A layer's own, by the stream its tiles come in (`Base::STREAM`).
pub struct Layer {
    pub stream: Stream,
    /// The most tiles it asks for at once: a base layer's tile cache's
    /// slots, the objects' shards about the eye or their top.
    pub slots: u32,
    /// A want's file arrived, by its subject, or its absence.
    on_arrival: fn(u64, Option<Block>, &mut Phases) -> Result<(), Failure>,
}

const fn layer<L: ObservedLayer>() -> Layer {
    Layer {
        stream: L::STREAM,
        slots: TileCache::<L>::SLOTS,
        on_arrival: on_arrived::<L>,
    }
}

/// Every layer read a tile at a time, the object layer's tile its shard
/// (domain/layer/object/repository.rs): the one place a stream names its layer.
const LAYERS: [Layer; 5] = [
    layer::<TerrainTile>(),
    layer::<PlaceTile>(),
    layer::<Earth>(),
    layer::<DivisionTile>(),
    Layer {
        stream: Stream::object,
        slots: repository::WANTS,
        on_arrival: repository::on_arrived,
    },
];

/// The layer whose tiles `stream` carries; none for an asset's.
pub fn of_stream(stream: Stream) -> Option<&'static Layer> {
    LAYERS.iter().find(|l| l.stream == stream)
}

/// Everything the asks depend on, as read; two turns agreeing on all of it
/// ask for the same tiles, so the second decides nothing.
#[derive(Clone, Copy, PartialEq)]
struct Key {
    o: Option<Observation>,
    dst: Option<Tile>,
}

pub struct Demand<L: ObservedLayer> {
    layer: core::marker::PhantomData<L>,
    observation: Option<Observation>,
    wants: Vec<(u64, Ask)>,
    last: Option<Key>,
    /// The floor's tiles as last observed.
    floor: Vec<Tile>,
    /// The asks found absent in an observation, written once all are found
    /// (`observe`); reused across observations.
    gone: Vec<Tile>,
    /// What it reads moved since it last observed.
    moved: bool,
    /// Where an observation builds the estimator's tiles and its asks, kept
    /// so observing stops allocating once warm. Empty between observations:
    /// what was asked is the tile cache's (`TileCache::ask`).
    demand: Vec<Tile>,
    asks: FnvHashMap<Tile, Ask>,
    /// The tile cache's revision as it idled it (`idle`), none while it steps.
    idle_revision: Option<u32>,
}

impl<L: ObservedLayer> Default for Demand<L> {
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
            asks: FnvHashMap::with_capacity_and_hasher(
                TileCache::<L>::SLOTS as usize,
                Default::default(),
            ),
            idle_revision: None,
        }
    }
}

impl<L: ObservedLayer> Demand<L> {
    /// The floor's tiles as last observed.
    pub fn floor(&self) -> impl Iterator<Item = Tile> + Clone + '_ {
        self.floor.iter().copied()
    }

    /// No object for `p`: the tile over it says its ground; at the layer's top (`Base::top`), `Base::empty`.
    pub fn absent(&mut self, p: Tile) {
        match L::empty() {
            Some(empty) if L::top().is_some_and(|top| p.zoom <= top) => {
                L::tile_cache().set(p, empty);
            }
            _ => L::tile_cache().answer_above(p),
        }
    }

    pub fn admit(
        &mut self,
        p: Tile,
        receive: impl FnOnce(u32) -> Result<L, Failure>,
    ) -> Result<(), Failure> {
        // The tile last in view order goes, of two as late the greater.
        let o = self.observation.as_ref();
        let farthest = |tiles: &mut dyn Iterator<Item = Tile>| {
            let mut walked = ColumnWalk::default();
            tiles.max_by_key(|&t| {
                (
                    o.map_or(Order::LAST, |o| {
                        observation::order_of_tile(o, L::LATTICE, t, &mut walked)
                    }),
                    t,
                )
            })
        };
        L::tile_cache().admit(p, farthest, receive)
    }

    /// Builds the asks for `o`: the estimator's demand, the floor and the
    /// destination's line.
    pub fn observe(
        &mut self,
        o: Option<&Observation>,
        dst: impl Iterator<Item = Tile>,
        estimator: &ByEye,
        top: Zoom,
    ) {
        let mut demand = core::mem::take(&mut self.demand);
        let mut asks = core::mem::take(&mut self.asks);
        let ask = |asks: &mut FnvHashMap<Tile, Ask>, p: Tile, a: Ask| {
            asks.entry(p).and_modify(|b| *b = b.best(a)).or_insert(a);
        };
        // One walk down the pyramid in key order, so each tile's line, its
        // own and the ground's it is ordered by, steps from its neighbor's.
        let (mut lines, mut ground) = (Lines::default(), ColumnWalk::default());
        let mut order =
            |o: &Observation, p: Tile| observation::order_of_tile(o, L::LATTICE, p, &mut ground);
        if let Some(o) = o {
            if matches!(L::ASK_SCOPE, AskScope::View) {
                estimator.requirements(o, &mut |p| demand.push(p));
            }
            demand.sort_unstable_by_key(|p| key_of::<2>(p.zoom, p.at));
            demand.dedup();
            for &p in &demand {
                let at = L::tile_cache().seek(&mut lines, p).src.unwrap_or(p);
                ask(
                    &mut asks,
                    at,
                    Ask {
                        priority: Priority::Demand,
                        order: order(o, p),
                    },
                );
                // A line already asked is asked to the top.
                for q in
                    std::iter::successors(Some(at), |q| (q.zoom > top).then(|| q.parent())).skip(1)
                {
                    if asks
                        .get(&q)
                        .is_some_and(|a| a.priority <= Priority::Ancestor)
                    {
                        break;
                    }
                    ask(
                        &mut asks,
                        q,
                        Ask {
                            priority: Priority::Ancestor,
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
            xpute_kit::ensure!(
                read.iter().all(|&n| n <= TileCache::<L>::ZOOM_DEMAND_TILES),
                ENOTRECOVERABLE,
                *read.iter().max().unwrap_or(&0)
            );
            if !matches!(L::ASK_SCOPE, AskScope::Line) {
                self.floor.clear();
                floor::floor(o, top, |p| {
                    self.floor.push(p);
                    ask(
                        &mut asks,
                        p,
                        Ask {
                            priority: Priority::Floor,
                            order: order(o, p),
                        },
                    );
                });
                xpute_kit::ensure!(
                    self.floor.len() <= floor::tiles(L::LATTICE) as usize,
                    ENOTRECOVERABLE,
                    self.floor.len()
                );
            }
        }
        // The whole line, not only the finest tile: which one says the ground is
        // known only once those above are, and asking one at a time would cost a
        // round trip per zoom.
        let mut named = 0u32;
        for p in dst {
            named += 1;
            ask(
                &mut asks,
                p,
                Ask {
                    priority: Priority::Destination,
                    order: Order::FIRST,
                },
            );
        }
        xpute_kit::ensure!(
            named
                <= if matches!(L::ASK_SCOPE, AskScope::Floor) {
                    0
                } else {
                    TileCache::<L>::DST_TILES + L::PINNED_CNT
                },
            ENOTRECOVERABLE,
            named
        );

        L::tile_cache().ask(&asks);
        // The asks in key order, in the demand's room (nothing reads it
        // between observations): each line read steps from its neighbor's
        // (`seek`), not from the top.
        demand.clear();
        demand.extend(asks.keys().copied());
        demand.sort_unstable_by_key(|p| p.key());
        let unanswered = |p: Tile, lines: &mut Lines| {
            let r = L::tile_cache();
            !r.resident(p) && (!r.seek(lines, p).settled || r.is_again_held(p))
        };
        // A tile no served shard is over or under is answered as its absence
        // is, and never asked for: every ask judged against the tile cache as
        // the asks left it, before any absence is written, so which are
        // absent depends on no order they are taken in.
        if let Some(layer) = L::STREAM.bake() {
            let (mut gone, mut lines) = (core::mem::take(&mut self.gone), Lines::default());
            gone.clear();
            gone.extend(
                demand
                    .iter()
                    .copied()
                    .filter(|&p| unanswered(p, &mut lines) && !source::serves(layer, p)),
            );
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
    /// every cast reads as it arrives. Observes again whenever anything in
    /// `Key` changed.
    ///
    /// `destination` is asked for with its line even with no observation yet:
    /// at the first fix the camera waits for that ground; and `pinned`, the
    /// tiles a reader that takes no coarser answer declares (`Base::PINNED_CNT`),
    /// as the destination is. A layer with a top of its own is observed out
    /// to its own reach.
    pub fn step(&mut self, dst: Option<Tile>, pinned: &[Tile]) {
        let Some(top) = L::top() else { return };
        let o = observation().map(|o| match L::TOP {
            Some(_) => Observation {
                demand_dist: L::demand_dist(),
                ..o
            },
            None => o,
        });
        if o.is_none() && dst.is_none() && pinned.is_empty() {
            return;
        }
        self.idle_revision = None;
        let now = Key { o, dst };
        if core::mem::take(&mut self.moved) | (self.last != Some(now)) {
            let line = dst
                .into_iter()
                .flat_map(|p| {
                    std::iter::successors(Some(p), move |q| (q.zoom > top).then(|| q.parent()))
                })
                .chain(pinned.iter().copied());
            self.observe(
                o.as_ref(),
                line,
                &ByEye {
                    lattice: L::LATTICE,
                    top,
                },
                top,
            );
            self.last = Some(now);
        }
        declare(L::STREAM, &self.wants);
    }

    /// What it reads moved: its own tiles', the ground's, or the objects'.
    pub fn on_moved(&mut self) {
        self.moved = true;
    }

    /// A layer the view does not show keeps what it holds and asks for
    /// nothing: its tile cache let go of its asks again only where what it
    /// holds moved since it last was (`TileCache::revision`), as asking
    /// nothing of what is unchanged changes nothing.
    pub fn idle(&mut self) {
        if self
            .idle_revision
            .is_some_and(|r| r == L::tile_cache().revision())
        {
            return;
        }
        self.wants.clear();
        L::tile_cache().ask(&self.asks);
        declare(L::STREAM, &[]);
        self.last = None;
        self.idle_revision = Some(L::tile_cache().revision());
    }
}

/// The object layer's tiles, its shards, from the live eye: every shard
/// within the demand the view makes of the ground at the shard's zoom
/// (`observation::demand_radius`), as many as `repository::WANTS` at most,
/// seen and nearer first. The repository reads their top first, which a
/// view from afar draws from, then those of them it lists and has not read.
pub fn object_step() {
    let o = observation();
    let r = o.zip(source::top()).and_then(|(o, top)| {
        observation::demand_radius(&o, GROUND, top, SHARD_ZOOM).map(|r| (o, r))
    });
    repository::want(r.into_iter().flat_map(|(o, r)| {
        tiles_within(SHARD_ZOOM, [o.eye[0], o.eye[1]], r).map(move |shard| {
            (
                shard,
                Ask {
                    priority: Priority::Demand,
                    order: Order::of(&o, &shard.region()),
                },
            )
        })
    }));
}

/// The tiles arrived, the pyramid's and the objects' shards (`LAYERS`),
/// admitted against the view as a pass of the
/// tick's budget, so the visible phase's hold on the content share binds it,
/// and at least one; each fetch settles here. What the pass leaves stays
/// queued, and a turn is asked for it.
pub fn admit_arrived(pass: &Pass) {
    const OF: [Stream; LAYERS.len()] = {
        let mut of = [Stream::asset; LAYERS.len()];
        let mut k = 0;
        while k < of.len() {
            of[k] = LAYERS[k].stream;
            k += 1;
        }
        of
    };
    let stat = pass.run(
        0..answer_cnt(&OF),
        |_| {
            let Some(Answer {
                key,
                stream,
                subject,
                block,
                mut phases,
            }) = take_answer(&OF)
            else {
                return false;
            };
            // A key the request scheduler no longer holds is one a reset forgot.
            if attempt_of(key).is_none() {
                release(block);
                return false;
            }
            let result = match of_stream(stream) {
                Some(l) => (l.on_arrival)(subject, block, &mut phases),
                None => {
                    release(block);
                    Err(Errno::ENOTRECOVERABLE.into())
                }
            };
            on_answer_settled(key, result, phases);
            true
        },
        None,
    );
    if stat.stopped_early {
        edge::request_world_tick();
    }
}

/// None is no object for the tile `subject` names.
fn on_arrived<L: ObservedLayer>(
    subject: u64,
    block: Option<Block>,
    phases: &mut Phases,
) -> Result<(), Failure> {
    let p = Tile::of_subject(subject);
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
