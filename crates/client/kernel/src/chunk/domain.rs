// pxl-kernel/chunk/domain.rs

//! What the eye's observation makes (the ground decision's "What is
//! drawn"): the chunk holding where each of the camera grid's rays stopped
//! (`view/sight.rs`), at the zoom its block is drawn at, is made, and no
//! other; in it only the blocks the rays stopped on are drawn ("Observation
//! determines materialization"). It is a function of the published cast
//! alone, made again when a new one is published, so the same observation
//! of the same world makes the same chunks in any order.
//!
//! A ray's chunk is the one whose cells are the block it stopped on, at the
//! zoom the ray's walk met it (`Stop::zoom`), which no ray asks coarser than
//! a 3D root's cells (`query/node.rs` `coarsest`): the block is drawn at its
//! own size, whatever other rays met near it. One chunk may hold another;
//! each draws only the blocks its own rays stopped on, so a stop is drawn
//! once, in one chunk.

use pxl_spec::space::camera::eye::Observation;
use pxl_spec::space::camera::grid::{Tau, CHUNK_STEPS};
use pxl_spec::space::coord::pvf3::pvf3::{self, Pvf3};
use pxl_spec::space::coord::zoom::{index_of, Zoom};
use xpute_guest::global::Global;

use crate::chunk::Chunk;
use crate::io::demand::Order;
use crate::layer::Query;
use crate::query::World;
use crate::view::sight::{self, Sight};

/// What a published cast makes.
#[derive(Default)]
pub struct Domain {
    /// The cast it is made of.
    generation: u32,
    observation: Option<Observation>,
    tau: Option<Tau>,
    /// Each block a ray stopped on: the chunk holding it and the block at
    /// the chunk's cells' zoom, sorted, once each.
    seen: Vec<(Chunk, [i64; 3])>,
    /// The chunks made, sorted.
    pieces: Vec<Chunk>,
}

impl Domain {
    fn of(&mut self, s: &Sight) {
        self.generation = s.generation();
        (self.observation, self.tau) = (s.observation().copied(), s.tau());
        // The block each ray stopped on, in the chunk whose cells it is.
        self.seen.clear();
        for stop in s.stops() {
            let at = pvf3::gather(pvf3::from_real(stop.at[0]), pvf3::from_real(stop.at[1]), pvf3::from_real(stop.at[2]));
            let c = holding(stop.zoom - CHUNK_STEPS, &at);
            self.seen.push((c, index_of(c.cell_zoom(), &at)));
        }
        self.seen.sort_unstable();
        self.seen.dedup();
        self.pieces.clear();
        self.pieces.extend(self.seen.iter().map(|&(c, _)| c));
        self.pieces.dedup();
    }

    pub fn tau(&self) -> Tau {
        self.tau.unwrap_or_else(sight::tau)
    }

    pub fn observation(&self) -> Option<&Observation> {
        self.observation.as_ref()
    }

    /// The chunks made, sorted.
    pub fn pieces(&self) -> &[Chunk] {
        &self.pieces
    }

    /// The blocks the rays stopped on in `c`, at its cells' zoom, sorted.
    pub fn seen_in(&self, c: Chunk) -> impl Iterator<Item = [i64; 3]> + '_ {
        let from = self.seen.partition_point(|&(k, _)| k < c);
        let to = self.seen.partition_point(|&(k, _)| k <= c);
        self.seen[from..to].iter().map(|&(_, b)| b)
    }

    /// The order a chunk is built in, measured from where in it anything
    /// stands: a bound, not the drawn surface, which on a coarse tile averages
    /// a peak away. A bound errs nearer.
    pub fn order_of(&self, c: Chunk) -> Order {
        let Some(o) = &self.observation else { return Order::LAST };
        let bound = World::now().bounds(c.node(), c.cell_zoom()).value().flatten().map_or(c.region(), |b| b.aabb);
        Order::of(o, &bound)
    }
}

/// The chunk at `zoom` holding `at`, by its one name round the world.
fn holding(zoom: Zoom, at: &Pvf3) -> Chunk {
    Chunk::holding(zoom, at).named()
}

static DOMAIN: Global<Domain> = Global::new();

/// What the last published cast makes; none before the first. A new one
/// changes what each chunk draws, so the chunks compare their sources.
pub fn current() -> Option<&'static Domain> {
    let s = sight::current()?;
    let d = DOMAIN.get(Domain::default);
    if d.generation != s.generation() {
        d.of(s);
        crate::chunk::source::chunk_sources_changed();
    }
    Some(&*d)
}
