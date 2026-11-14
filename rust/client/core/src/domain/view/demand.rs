// pxl-core/domain/view/demand.rs

//! What each layer on the ground asks of its tile pyramid. The tile cache
//! holds the tiles (tile_cache.rs); this module decides which tiles an
//! observation asks for, and in what order.
//!
//! A tile is asked for by the eye's demand (`ByEye`), by the floor
//! (layer/floor.rs), by a destination's line, or as an ancestor of a
//! missing tile. A tile asked for several reasons keeps the strongest
//! (`Priority`). The pixels drawn do not enter the asks, since the camera grid
//! is an angle. Where a tile marks a child as no tile (`Tile::occupancy`), the
//! child's ground is the tile's own and the child is never asked for.

use pxl_global::Global;
use pxl_spec::bake::tile::place::PlaceTile;
use pxl_spec::bake::tile::terrain::TerrainTile;

use crate::domain::layer::earth::EarthTile;
use crate::domain::layer::object::repository;
use crate::domain::layer::{AskScope, Base};
use crate::sys::io::body::Body;
use crate::sys::io::demand::{Source, Stream};
use crate::sys::io::fetch::{answer_cnt, take_answer, Response};
use crate::sys::io::request::{attempt_of, settle};
use pxl_spec::bake::tile::division::DivisionTile;
use xpute_kit::status::errno::Errno;

use fnv::FnvHashMap;

use pxl_spec::space::camera::eye::Observation;
use pxl_spec::space::coord::zoom::{Zoom, LEAF_ZOOM, SHARD_ZOOM};
use pxl_spec::space::layer::quadrant::Quadrant;

use crate::domain::layer::floor;
use crate::domain::layer::tile_cache::{Path, TileCache};
use crate::domain::query::ColumnWalk;
use crate::domain::view::observation;
use crate::domain::view::observation::{observation, ByEye};
use crate::sys::io::demand::{Order, PhaseList};
use crate::sys::io::failure::{Exit, Failure};
use crate::sys::io::request::declare;
use crate::sys::io::source;
use crate::sys::sched;
use pxl_spec::space::hierarchy::morton::key_of;
use pxl_spec::space::layer::demand::tiles_within;
use pxl_spec::space::layer::lattice::{demand_dist, GROUND};

/// A base layer whose demand the view holds, beside its tile cache.
pub trait DemandedBase: Base {
    fn demand() -> &'static mut Demand<Self>;
}

static TERRAIN: Global<Demand<TerrainTile>> = Global::new();
static PLACE: Global<Demand<PlaceTile>> = Global::new();
static EARTH: Global<Demand<EarthTile>> = Global::new();
static DIVISION: Global<Demand<DivisionTile>> = Global::new();

impl DemandedBase for TerrainTile {
    fn demand() -> &'static mut Demand<TerrainTile> {
        TERRAIN.get(Demand::default)
    }
}

impl DemandedBase for PlaceTile {
    fn demand() -> &'static mut Demand<PlaceTile> {
        PLACE.get(Demand::default)
    }
}

impl DemandedBase for DivisionTile {
    fn demand() -> &'static mut Demand<DivisionTile> {
        DIVISION.get(Demand::default)
    }
}

impl DemandedBase for EarthTile {
    fn demand() -> &'static mut Demand<EarthTile> {
        EARTH.get(Demand::default)
    }
}

/// What the fetch side needs of a layer, found by the stream its tiles come
/// in (`Stream::of_layer`).
pub struct Layer {
    pub stream: Stream,
    /// The most tiles the layer asks for at once: a base layer's tile cache
    /// slots, or for the objects the shards around the eye or their top.
    pub slot_cnt: u32,
    /// Called when a want's file arrives, with the want's id and the file, or
    /// with None when the source has nothing for it.
    on_arrival: fn(u64, Option<Body>, &mut PhaseList) -> Result<(), Exit>,
}

const fn layer<L: DemandedBase>() -> Layer {
    Layer {
        stream: const { Stream::of_layer(L::KIND) },
        slot_cnt: TileCache::<L>::SLOT_CNT,
        on_arrival: on_arrived::<L>,
    }
}

/// Every layer that is read a tile at a time; the object layer's tile is its
/// shard (domain/layer/object/repository.rs). This is the one place a stream
/// names its layer. A stream whose source is fetched has exactly one layer
/// here, and a loaded stream has none.
const LAYERS: [Layer; 5] = [
    layer::<TerrainTile>(),
    layer::<PlaceTile>(),
    layer::<EarthTile>(),
    layer::<DivisionTile>(),
    Layer {
        stream: Stream::Object,
        slot_cnt: repository::WANT_CAP,
        on_arrival: repository::on_arrived,
    },
];

const _: () = {
    let mut s = 0;
    while s < Stream::ALL.len() {
        let stream = Stream::ALL[s];
        let mut layers = 0;
        let mut k = 0;
        while k < LAYERS.len() {
            if LAYERS[k].stream as u8 == stream as u8 {
                layers += 1;
            }
            k += 1;
        }
        let fetched = !matches!(stream.row().source, Source::Load);
        assert!(
            layers == fetched as u32,
            "a fetched stream is one layer's, and a load none's"
        );
        s += 1;
    }
};

/// The layer whose tiles `stream` carries. None for a stream of assets.
pub fn of_stream(stream: Stream) -> Option<&'static Layer> {
    LAYERS.iter().find(|l| l.stream == stream)
}

/// Why a tile is asked for, strongest first.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum Priority {
    Floor,
    /// On a line asked for exactly: the camera's destination's, or the line
    /// over the division layer's pivot.
    Destination,
    /// On the line over a missing tile. A coarser answer is then held
    /// wherever the view looks, and leaving a zoom shows its parent at once
    /// (as Cesium's `preloadAncestors` does).
    Ancestor,
    Demand,
}

/// Why a tile is asked for, and its place in the view, which decides when
/// its want is taken (sys/io/request.rs `declare`).
#[derive(Clone, Copy)]
struct Ask {
    priority: Priority,
    order: Order,
}

impl Ask {
    /// The stronger reason and the earlier place of two asks, each taken on
    /// its own: why a tile is asked for says nothing of when the view needs it.
    fn best(self, other: Ask) -> Ask {
        Ask {
            priority: self.priority.min(other.priority),
            order: self.order.min(other.order),
        }
    }
}

/// Everything the asks depend on. Two frames that agree on all of it ask for
/// the same tiles, so the second decides nothing.
#[derive(Clone, Copy, PartialEq)]
struct Key {
    o: Option<Observation>,
    dst: Option<Quadrant>,
}

pub struct Demand<L: DemandedBase> {
    layer: core::marker::PhantomData<L>,
    observation: Option<Observation>,
    wants: Vec<(u64, Order)>,
    /// Each want's order before its place in the view is read. `reorder`
    /// starts from it.
    orders: Vec<Order>,
    last: Option<Key>,
    /// The estimator's tiles as the last observation named them, in the
    /// order named, before any was asked for. An observation that names the
    /// same tiles and the same floor asks for what that one asked
    /// (`names_same`).
    last_tiles: Vec<Quadrant>,
    floor: Vec<Quadrant>,
    /// The asks an observation found absent. Their absence is written only
    /// once all are found (`observe`). The vector is reused across
    /// observations.
    absent_tiles: Vec<Quadrant>,
    /// Whether what the layer reads moved since it last observed.
    changed: bool,
    /// The room an observation builds the estimator's tiles and its asks in,
    /// kept so that observing stops allocating once warm. Both are empty
    /// between observations, since the tile cache keeps what was asked
    /// (`TileCache::ask`).
    demand: Vec<Quadrant>,
    asks: FnvHashMap<Quadrant, Ask>,
    /// Each ask in the view, sorted by the ask's id, paired with the tile its
    /// place in the view is judged by. An ask made for a demanded tile, to
    /// give that tile's ground, is judged by the demanded tile; an ancestor
    /// or a floor tile is judged by itself. Only the asks left unanswered,
    /// the wants, are read, since only their order matters (`policy::rank`),
    /// and the pairs are kept to order the wants again (`reorder`).
    ask_tiles: Vec<(Quadrant, Quadrant)>,
    /// The tile cache's revision when `idle` last ran. None while the layer
    /// steps.
    idle_revision: Option<u32>,
}

impl<L: DemandedBase> Default for Demand<L> {
    fn default() -> Self {
        Demand {
            layer: core::marker::PhantomData,
            observation: None,
            demand: Vec::new(),
            wants: Vec::new(),
            orders: Vec::new(),
            last: None,
            last_tiles: Vec::new(),
            floor: Vec::new(),
            absent_tiles: Vec::new(),
            changed: false,
            asks: FnvHashMap::with_capacity_and_hasher(
                TileCache::<L>::SLOT_CNT as usize,
                Default::default(),
            ),
            ask_tiles: Vec::new(),
            idle_revision: None,
        }
    }
}

impl<L: DemandedBase> Demand<L> {
    /// The floor's tiles as last observed.
    pub fn floor(&self) -> impl Iterator<Item = Quadrant> + Clone + '_ {
        self.floor.iter().copied()
    }

    /// Records that the source has no object for `p`. At the layer's coarsest
    /// zoom (`Base::min_zoom`) or coarser, `p` takes the layer's empty tile
    /// where it has one (`Base::empty`); otherwise the tile over `p` gives its
    /// ground.
    pub fn absent(&mut self, p: Quadrant) {
        match L::empty() {
            Some(empty) if L::min_zoom().is_some_and(|min| p.zoom <= min) => {
                L::tile_cache().set(p, empty);
            }
            _ => L::tile_cache().answer_above(p),
        }
    }

    pub fn admit(
        &mut self,
        p: Quadrant,
        receive: impl FnOnce(u32) -> Result<L, Exit>,
    ) -> Result<(), Exit> {
        // When the cache must evict, the tile last in view order goes, and of
        // two equally late, the greater.
        let o = self.observation.as_ref();
        let farthest = |tiles: &mut dyn Iterator<Item = Quadrant>| {
            let mut walk = ColumnWalk::default();
            tiles.max_by_key(|&t| {
                (
                    o.map_or(Order::LAST, |o| {
                        observation::order_of_tile(o, L::LATTICE, t, &mut walk)
                    }),
                    t,
                )
            })
        };
        L::tile_cache().admit(p, farthest, receive)
    }

    /// Builds the asks for `o`: the estimator's demand, the floor, and the
    /// destination's line with the pinned tiles (`dst`).
    pub fn observe(
        &mut self,
        o: Option<&Observation>,
        dst: impl Iterator<Item = Quadrant>,
        estimator: &ByEye,
        top: Zoom,
    ) {
        let mut demand = core::mem::take(&mut self.demand);
        let mut asks = core::mem::take(&mut self.asks);
        let mut ask_tiles = core::mem::take(&mut self.ask_tiles);
        ask_tiles.clear();
        // An ask in the view starts last in order. Its place, the earliest of
        // the tiles it is asked by, is read later, and only for a want
        // (`reorder`).
        let mut ask = |asks: &mut FnvHashMap<Quadrant, Ask>,
                       p: Quadrant,
                       priority: Priority,
                       of: Quadrant| {
            let a = Ask {
                priority,
                order: Order::LAST,
            };
            asks.entry(p).and_modify(|b| *b = b.best(a)).or_insert(a);
            ask_tiles.push((p, of));
        };
        let mut path = Path::default();
        self.last_tiles.clear();
        if let Some(o) = o {
            if matches!(L::ASK_SCOPE, AskScope::View) {
                estimator.requirements(o, &mut |p| demand.push(p));
            }
            self.last_tiles.extend_from_slice(&demand);
            demand.sort_unstable_by_key(|p| key_of::<2>(p.zoom, p.at));
            demand.dedup();
            for &p in &demand {
                let at = L::tile_cache().seek(&mut path, p).src.unwrap_or(p);
                ask(&mut asks, at, Priority::Demand, p);
                // An ancestor already asked has its line asked up to the top,
                // so the walk up stops there.
                for q in
                    std::iter::successors(Some(at), |q| (q.zoom > top).then(|| q.parent())).skip(1)
                {
                    if asks
                        .get(&q)
                        .is_some_and(|a| a.priority <= Priority::Ancestor)
                    {
                        break;
                    }
                    ask(&mut asks, q, Priority::Ancestor, q);
                }
            }
            // The tile cache is sized for at most `ZOOM_DEMAND_TILE_CAP` asks at
            // each zoom, so more means that sizing is broken. The ancestors of
            // a missing tile hold ground within its reach, so they stay inside
            // each zoom's ring and the bound covers them too.
            let mut read = [0u32; LEAF_ZOOM as usize + 1];
            for p in asks.keys() {
                read[p.zoom as usize] += 1;
            }
            xpute_kit::ensure!(
                read.iter()
                    .all(|&n| n <= TileCache::<L>::ZOOM_DEMAND_TILE_CAP),
                ENOTRECOVERABLE,
                *read.iter().max().unwrap_or(&0)
            );
            if !matches!(L::ASK_SCOPE, AskScope::Line) {
                self.floor.clear();
                floor::floor(o, top, |p| {
                    self.floor.push(p);
                    ask(&mut asks, p, Priority::Floor, p);
                });
                xpute_kit::ensure!(
                    self.floor.len() <= floor::tile_cap(L::LATTICE) as usize,
                    ENOTRECOVERABLE,
                    self.floor.len()
                );
            }
        }
        // The destination asks for its whole line, not only the finest tile:
        // which tile on the line gives the ground is known only once the
        // tiles above it are.
        let mut named = 0u32;
        for p in dst {
            named += 1;
            let a = Ask {
                priority: Priority::Destination,
                order: Order::FIRST,
            };
            asks.entry(p).and_modify(|b| *b = b.best(a)).or_insert(a);
        }
        xpute_kit::ensure!(
            named
                <= if matches!(L::ASK_SCOPE, AskScope::Floor) {
                    0
                } else {
                    TileCache::<L>::ZOOM_CNT + L::PIN_CAP
                },
            ENOTRECOVERABLE,
            named
        );

        L::tile_cache().ask(&asks);
        // The asks are sorted in key order in the demand's room, which
        // nothing reads between observations. Each line read then steps on
        // from its neighbor's (`seek`) and not from the top.
        demand.clear();
        demand.extend(asks.keys().copied());
        demand.sort_unstable_by_key(|p| p.id());
        let unanswered = |p: Quadrant, path: &mut Path| {
            let r = L::tile_cache();
            !r.resident(p) && (!r.seek(path, p).settled || r.needs_refetch(p))
        };
        // A tile that no served shard is over or under is answered as absent
        // and never asked for. Every ask is judged against the tile cache as
        // the asks left it, before any absence is written, so which tiles
        // are absent does not depend on the order they are taken in.
        let (mut absent_tiles, mut path) =
            (core::mem::take(&mut self.absent_tiles), Path::default());
        absent_tiles.clear();
        absent_tiles.extend(
            demand
                .iter()
                .copied()
                .filter(|&p| unanswered(p, &mut path) && !source::serves(L::KIND, p)),
        );
        for &p in &absent_tiles {
            self.absent(p);
        }
        self.absent_tiles = absent_tiles;
        // The wants are listed by id, the order a stream's wants are declared
        // in (sys/io/request.rs `declare`). A tile's id is its key, so each
        // line read steps on from its neighbor's. They are listed after every
        // absence is written, then placed in the view (`reorder`).
        ask_tiles.sort_unstable_by_key(|&(p, of)| (p.id(), of.id()));
        let mut path = Path::default();
        self.wants.clear();
        self.orders.clear();
        for &p in &demand {
            if unanswered(p, &mut path) {
                let a = asks[&p];
                self.orders.push(a.order);
                self.wants.push((p.id(), a.order));
            }
        }
        asks.clear();
        self.asks = asks;
        self.ask_tiles = ask_tiles;
        demand.clear();
        self.demand = demand;
        self.reorder(o);
    }

    /// Whether `o` names the same estimator's and floor's tiles as the last
    /// observation, in the same order. Both depend on the eye's place and
    /// reach alone, so between two such observations the asks stand and only
    /// the wants' order moves.
    fn names_same(&self, o: &Observation, estimator: &ByEye, top: Zoom) -> bool {
        (!matches!(L::ASK_SCOPE, AskScope::View)
            || names(&self.last_tiles, |each| estimator.requirements(o, each)))
            && (matches!(L::ASK_SCOPE, AskScope::Line)
                || names(&self.floor, |each| floor::floor(o, top, each)))
    }

    /// Places the wants in the view of `o`. A want takes its own order, which
    /// puts a destination's first, or else the earliest place of the tiles
    /// it is asked by (`ask_tiles`). The places are read in one walk down the
    /// pyramid in key order, so each tile's ground steps on from its
    /// neighbor's. The result depends on what is asked and on `o` alone, so a
    /// frame that asks what the last one did only needs this.
    fn reorder(&mut self, o: Option<&Observation>) {
        let mut ground = ColumnWalk::default();
        let mut ask_tiles = self.ask_tiles.iter().peekable();
        for ((id, order), &base) in self.wants.iter_mut().zip(&self.orders) {
            *order = base;
            while ask_tiles.next_if(|(q, _)| q.id() < *id).is_some() {}
            while let Some(&(_, of)) = ask_tiles.next_if(|(q, _)| q.id() == *id) {
                if let Some(o) = o {
                    *order =
                        (*order).min(observation::order_of_tile(o, L::LATTICE, of, &mut ground));
                }
            }
        }
        self.observation = o.copied();
    }

    /// The layer's frame, from the observation of the eye this frame composed,
    /// the eye this frame casts from (frame.rs `camera_tick`). When
    /// anything in `Key` changed, the layer observes again. When nothing it
    /// reads moved and the eye names the tiles it named before (`names_same`),
    /// it only places the wants again.
    ///
    /// `dst` is asked for with its line even before there is an observation,
    /// since at the first fix the camera waits for that ground. `pinned`
    /// holds the tiles that a reader taking no coarser answer declares
    /// (`Base::PIN_CAP`), and they are asked for as the destination is. A
    /// layer with a top of its own (`Base::MIN_ZOOM`) is observed out to that
    /// top's reach.
    pub fn step(&mut self, dst: Option<Quadrant>, pinned: &[Quadrant]) {
        let Some(min_zoom) = L::min_zoom() else {
            return;
        };
        let o = observation().map(|o| match L::MIN_ZOOM {
            Some(_) => Observation {
                demand_dist: demand_dist(L::LATTICE, min_zoom),
                ..o
            },
            None => o,
        });
        if o.is_none() && dst.is_none() && pinned.is_empty() {
            return;
        }
        self.want(o, dst, pinned, min_zoom);
        declare(const { Stream::of_layer(L::KIND) }, &self.wants);
    }

    /// The body of `step` for the observation `o` and the layer's `top`. It
    /// leaves the wants in `wants`.
    fn want(
        &mut self,
        o: Option<Observation>,
        dst: Option<Quadrant>,
        pinned: &[Quadrant],
        top: Zoom,
    ) {
        self.idle_revision = None;
        let now = Key { o, dst };
        let changed = core::mem::take(&mut self.changed);
        if !changed && self.last == Some(now) {
            return;
        }
        let estimator = ByEye {
            lattice: L::LATTICE,
            top,
        };
        // When only the frustum turned, the observation names what the last
        // one did.
        let named = !changed
            && self.last.is_some_and(|k| {
                k.dst == dst
                    && k.o.zip(o).is_some_and(|(was, is)| {
                        Observation {
                            frustum: is.frustum,
                            ..was
                        } == is
                            || self.names_same(&is, &estimator, top)
                    })
            });
        if named {
            self.reorder(o.as_ref());
        } else {
            let line = dst
                .into_iter()
                .flat_map(|p| {
                    std::iter::successors(Some(p), move |q| (q.zoom > top).then(|| q.parent()))
                })
                .chain(pinned.iter().copied());
            self.observe(o.as_ref(), line, &estimator, top);
        }
        self.last = Some(now);
    }

    /// Marks that what the layer reads moved: its own tiles, the ground or
    /// the objects.
    pub fn on_changed(&mut self) {
        self.changed = true;
    }

    /// Keeps what a layer the view does not show holds, and asks for nothing.
    /// Its tile cache drops its asks again only when what it holds moved
    /// since the last time (`TileCache::revision`).
    pub fn idle(&mut self) {
        if self
            .idle_revision
            .is_some_and(|r| r == L::tile_cache().revision())
        {
            return;
        }
        self.wants.clear();
        L::tile_cache().ask(&self.asks);
        declare(const { Stream::of_layer(L::KIND) }, &[]);
        self.last = None;
        self.idle_revision = Some(L::tile_cache().revision());
    }
}

/// Whether `name` names the tiles of `was`, in its order.
fn names(was: &[Quadrant], name: impl FnOnce(&mut dyn FnMut(Quadrant))) -> bool {
    let (mut k, mut same) = (0, true);
    name(&mut |p| {
        same &= was.get(k) == Some(&p);
        k += 1;
    });
    same && k == was.len()
}

/// What the objects' shards are asked from (`object_step`): the
/// observation, the source's top and the repository's revision. Two frames
/// agreeing on all three ask for the same shards in the same order.
type ObjectKey = (Option<Observation>, Option<Zoom>, u32);

/// The key the objects' shards were last asked from.
static OBJECT: Global<Option<ObjectKey>> = Global::new();

/// Asks for the object layer's tiles, its shards, from the eye: every
/// shard within the radius the view asks the ground for at the shard's zoom
/// (`observation::demand_radius`), at most `repository::WANT_CAP` of them, seen
/// and nearer ones first. The repository reads their top first, which a
/// view from afar draws from. Then it reads the shards the top lists and it
/// has not read, and places each in the view only once it is listed.
pub fn object_step() {
    let (o, top) = (observation(), source::min_zoom());
    let now = Some((o, top, repository::revision()));
    let last = OBJECT.get(|| None);
    if *last == now {
        repository::want_again();
        return;
    }
    *last = now;
    let r = o.zip(top).and_then(|(o, top)| {
        observation::demand_radius(&o, GROUND, top, SHARD_ZOOM).map(|r| (o, r))
    });
    repository::want(
        r.into_iter()
            .flat_map(|(o, r)| tiles_within(SHARD_ZOOM, [o.eye[0], o.eye[1]], r)),
        |shard| o.map_or(Order::LAST, |o| observation::order_of(&o, &shard.region())),
    );
}

/// Admits the tiles that arrived, the pyramid's and the objects' shards
/// (`LAYERS`), within the entry's budget, and at least one where any has
/// arrived. Each fetch settles here. What the budget leaves stays queued, and
/// a frame is asked for it.
pub fn admit_arrived() {
    const OF: [Stream; LAYERS.len()] = {
        let mut of = [Stream::Asset; LAYERS.len()];
        let mut k = 0;
        while k < of.len() {
            of[k] = LAYERS[k].stream;
            k += 1;
        }
        of
    };
    let stat = sched::run(0..answer_cnt(&OF), |_| {
        let Some(Response {
            key,
            stream,
            id,
            pkt,
            mut phases,
        }) = take_answer(&OF)
        else {
            return false;
        };
        // A key the request scheduler no longer holds belongs to a request
        // stopped or reset since it was answered.
        if attempt_of(key).is_none() {
            return false;
        }
        let result = match of_stream(stream) {
            Some(l) => (l.on_arrival)(id, pkt, &mut phases),
            None => Err(Failure::from(Errno::ENOTRECOVERABLE).into()),
        };
        settle(key, Exit::from(result), Some(phases));
        true
    });
    if stat.stopped_early {
        sched::invalidate();
    }
}

/// Admits the file for the tile `id` names. A `pkt` of None means the source
/// has no object for that tile. A packet no slot took goes back with the
/// closure that held it.
fn on_arrived<L: DemandedBase>(
    id: u64,
    pkt: Option<Body>,
    phases: &mut PhaseList,
) -> Result<(), Exit> {
    let p = Quadrant::of_id(id);
    let Some(pkt) = pkt else {
        L::demand().absent(p);
        return Ok(());
    };
    L::demand().admit(p, |slot| L::receive(p, slot, pkt, phases))
}

#[cfg(test)]
#[path = "demand.test.rs"]
mod test;
