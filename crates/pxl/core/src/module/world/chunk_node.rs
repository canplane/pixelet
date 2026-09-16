// pxl_core/module/world/chunk_node.rs

//! One chunk's glue half.
//!
//! Every decision about a chunk is the scheduler's
//! (crates/pxl/core/src/world/sched.rs): whether it is on screen, which tiers it
//! shows and how a blend weighs them, what to build and when, what to give back.
//! What stays here is what the decisions act on and the sources they are made
//! from: the tiers and decor blocks the scheduler handed over, held for exactly
//! as long as it names them, and the revisions that say a chunk's sources
//! changed.
//!
//! A block has one owner: this node, from the tier's arrival to the scheduler's
//! evict of the step (or the chunk's unmount, or its sources told again, when
//! the scheduler lets every tier go at once). The scheduler draws from its own
//! lists, so nothing here decides what is shown, and a block never has to
//! outlive the scheduler's naming of it.

use std::collections::{HashMap, VecDeque};

use pxl_shared::bake::format::Tile;
use pxl_shared::spec::space::indexer::FROM_L3_IDX;
use pxl_shared::spec::space::pvf3::ivec::{self, IVec};
use pxl_shared::spec::space::xy::TO_XY;

use crate::app::setting::store::use_settings_store;
use crate::module::world::change::{world_changes, TERRAIN_PAGE};
use crate::global::{now, Global};
use crate::kernel::{kernel, with_world};
use crate::module::world::lod::LodStep;
use crate::module::world::object::store::{decor_column_revision, object_column_of, object_column_revision, objects_revision};
use crate::module::world::terrain::store::{tile_pages, use_tile_store};
use crate::world::build::{Event as BuildEvent, PageLanes};
use crate::world::sched::Event;

/// A tier as the scheduler hands it over and the node holds it: the block in
/// the mesh section and the counts its lanes are read by.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct MeshBlock {
    pub at: usize,
    pub bytes: u32,
    pub vertex_cnt: u32,
    pub index_cnt: u32,
}

/// A chunk's decor at one tier: one block of the mesh section holding every
/// group's lanes (render/mesh/decor_block.rs).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct DecorBlock {
    pub at: usize,
    pub bytes: u32,
}

/// Hands a block back to the mesh section's pool. Every view into it is dead
/// afterwards.
pub fn release_block(at: usize, bytes: u32) {
    let k = kernel();
    if let Some(v) = k.volumes.as_ref() {
        let pool = v.mesh_pool();
        k.pools[pool].release(at, bytes);
    }
}

/// How much built work chunks may keep *while off screen*: run sets and cached
/// tiers, both in the mesh section and sized in bytes. Leaving the frustum used
/// to drop all of it on the first release, and a rotation or a zoom in and out
/// rebuilt every chunk from the scan up; the scheduler reclaims only past this,
/// farthest first. It budgets only the optional part: on-screen chunks hold
/// their tiers because that is the layout. Half the mesh section: the other half
/// is the on-screen layouts, the volumes a build composes in, and the room a
/// large block needs in one piece — at a quarter of the whole core reservation
/// (128 MB of a 256 MB section) the section sat two-thirds full and fragmented,
/// and refused the fine tiers' blocks 88,000 times in a minute (2026-09-14).
#[allow(non_snake_case)]
pub fn RETAIN_BUDGET_BYTES() -> f64 {
    crate::memory::map().map_or(0.0, |m| (m.mesh.bytes / 2) as f64)
}

// ---- Dev-only counters, read by the scheduler's frame sample ----

/// Chunks that appeared, disappeared, were laid out and were released since the
/// last frame sample.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct NodeCounts {
    pub added: u32,
    pub removed: u32,
    pub layouts: u32,
    pub released: u32,
}

static NODE_COUNTS: Global<NodeCounts> = Global::new();

fn node_counts() -> &'static mut NodeCounts {
    NODE_COUNTS.get(NodeCounts::default)
}

pub fn take_node_counts() -> NodeCounts {
    core::mem::take(node_counts())
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ChunkRetention {
    pub nodes: u32,
    /// Nodes holding a run set, and what those sets cost in the pool.
    pub run_sets: u32,
    pub run_set_mb: u32,
    /// Built tiers held across every node's cache.
    pub tiers: u32,
    /// Decor: cached per-step blocks.
    pub decor_steps: u32,
    /// The instances those blocks hold, which is what they cost to draw.
    pub decor_instances: u32,
}

// ---- The live chunks ----

struct Nodes {
    nodes: HashMap<u32, ChunkNode>,
    /// The world-wide sources as last seen: a change in any is when the nodes
    /// look at their own.
    seen: Option<(u32, u32, u32)>,
    /// Chunks whose layout named tiers they do not hold, rebuilt after the
    /// events that found it — a rebuild would re-enter them.
    rebuild: Vec<u32>,
}

static NODES: Global<Nodes> = Global::new();

fn nodes() -> &'static mut Nodes {
    NODES.get(|| Nodes {
        nodes: HashMap::new(),
        seen: None,
        rebuild: Vec::new(),
    })
}

/// Every live node by the scheduler's id.
pub fn chunk_nodes() -> &'static HashMap<u32, ChunkNode> {
    &nodes().nodes
}

pub fn chunk_retention() -> ChunkRetention {
    let mut out = ChunkRetention::default();
    let mut run_bytes = 0u64;
    for node in nodes().nodes.values() {
        out.nodes += 1;
        if node.runs_bytes > 0 {
            out.run_sets += 1;
            run_bytes += node.runs_bytes as u64;
        }
        out.tiers += node.tiers.len() as u32;
        out.decor_steps += node.decor_cache.len() as u32;
        for d in node.decor_cache.values() {
            // SAFETY: the block the scheduler wrote at `at`, `bytes` long, held
            // for as long as the node holds this decor.
            let words: &[u32] = unsafe { core::slice::from_raw_parts(d.at as *const u32, d.bytes as usize / 4) };
            out.decor_instances += crate::world::decor::decor_groups(words).map(|(_, n)| n).sum::<u32>();
        }
    }
    out.run_set_mb = (run_bytes as f64 / 1_048_576.0).round() as u32;
    out
}

pub struct ChunkNode {
    /// The page this chunk belongs to, for the page's own mount diff.
    pub page_idx: IVec,
    /// This chunk's own absolute lattice origin.
    pub base_i: IVec,
    /// The object store's key for this chunk's column.
    pub object_key: IVec,
    id: u32,
    /// The tiers and the decor blocks the scheduler handed over, by step.
    tiers: HashMap<LodStep, MeshBlock>,
    decor_cache: HashMap<LodStep, DecorBlock>,
    /// How many times the scheduler has been told this chunk's sources: a tier
    /// built from an earlier telling is stale and released on arrival.
    generation: u32,
    /// What this chunk's runs hold in the mesh section, as the scheduler said.
    runs_bytes: u32,
    /// The page's tile as last told, by its revision in the world change log:
    /// 0 while the page has none. Not by address: the tile store keeps its
    /// tiles in a vector, and a push that grows it or a clear that moves its
    /// last tile into the hole moves tiles no one replaced.
    tile: u32,
    show_structures: bool,
    objects_revision: u32,
    decor_revision: u32,
    show_decor: bool,
}

type Queue = VecDeque<Event>;

fn page_lanes(tile: &Tile) -> PageLanes {
    let g = &tile.terrain.ground;
    PageLanes {
        ground: [g.land.as_slice().as_ptr() as usize, g.water.as_slice().as_ptr() as usize, g.structure.as_slice().as_ptr() as usize, g.transport.as_slice().as_ptr() as usize],
        ele: tile.terrain.ele.as_slice().as_ptr() as usize,
    }
}

/// The revision the tile store last gave `page_idx`'s tile: moved by every set
/// and clear of the page, 0 once it has left. A topic's revisions only climb,
/// so a tile set again after a clear never reads as the one before.
fn tile_revision(page_idx: IVec) -> u32 {
    world_changes().revision_of(TERRAIN_PAGE, TO_XY(page_idx) as i64)
}

impl ChunkNode {
    fn new(id: u32, page_idx: IVec, base_x: u32, base_y: u32) -> ChunkNode {
        let page_off = ivec::gather(base_x as i64, base_y as i64, 0);
        let object_key = object_column_of(page_idx, page_off);
        let settings = use_settings_store();
        ChunkNode {
            page_idx,
            base_i: ivec::add(FROM_L3_IDX(page_idx), page_off),
            object_key,
            id,
            tiers: HashMap::new(),
            decor_cache: HashMap::new(),
            generation: 0,
            runs_bytes: 0,
            tile: tile_revision(page_idx),
            show_structures: settings.show_structures,
            objects_revision: object_column_revision(object_key),
            decor_revision: decor_column_revision(object_key),
            show_decor: settings.show_decor,
        }
    }

    /// Every tier's and every decor block back to the pool: the scheduler let
    /// them all go (its sources told again, or the chunk unmounted).
    fn release_all(&mut self) {
        for (_, t) in self.tiers.drain() {
            release_block(t.at, t.bytes);
        }
        for (_, d) in self.decor_cache.drain() {
            release_block(d.at, d.bytes);
        }
    }

    /// Tells the scheduler what this chunk is made of now.
    fn tell_source(&mut self, queue: &mut Queue) {
        let page = use_tile_store().get_tile(self.page_idx).map(page_lanes);
        let (id, show) = (self.id, self.show_structures);
        with_world(|world, sched| sched.source(world, id, page, show, &mut |e| queue.push_back(e)));
        self.generation += 1;
    }

    fn build_now(&self, queue: &mut Queue) {
        let id = self.id;
        with_world(|world, sched| sched.build_now(world, id, &now, &mut |e| queue.push_back(e)));
    }

    /// A source revision rebuilds what the chunk shows, not the whole visible
    /// world: the scheduler is told, and builds this chunk's layout to the end at
    /// once, so a placement shows in the frame it was made. `objects_revision`
    /// is the SOLID-only revision; a solid change still clears decor's cache,
    /// because a newly placed cube can occlude a scattered decor candidate.
    fn on_source_change(&mut self, queue: &mut Queue) {
        let tile = tile_revision(self.page_idx);
        let show_structures = use_settings_store().show_structures;
        let objects_revision = object_column_revision(self.object_key);
        if self.tile == tile && self.show_structures == show_structures && self.objects_revision == objects_revision {
            return;
        }
        self.tile = tile;
        self.show_structures = show_structures;
        self.objects_revision = objects_revision;
        self.release_all();
        self.tell_source(queue);
        self.build_now(queue);
    }

    /// Decor's own revision-only rebuild. It must never touch the tiers: that
    /// would reintroduce the needless cube rebuild — visible as a flicker — this
    /// split exists to avoid.
    fn on_decor_change(&mut self, queue: &mut Queue) {
        let revision = decor_column_revision(self.object_key);
        let show_decor = use_settings_store().show_decor;
        if revision == self.decor_revision && show_decor == self.show_decor {
            return;
        }
        self.decor_revision = revision;
        self.show_decor = show_decor;
        for (_, d) in self.decor_cache.drain() {
            release_block(d.at, d.bytes);
        }
        let id = self.id;
        with_world(|world, sched| sched.decor_now(world, id, show_decor, &mut |e| queue.push_back(e)));
    }

    fn on_tier(&mut self, generation: u32, step: LodStep, mesh: MeshBlock) {
        if generation != self.generation || self.tiers.contains_key(&step) {
            release_block(mesh.at, mesh.bytes);
            return;
        }
        self.tiers.insert(step, mesh);
    }

    /// The scheduler hands a step's decor over only where it believes this node
    /// holds none, so one already held is stale and gives way.
    fn on_decor(&mut self, step: LodStep, decor: DecorBlock) {
        if let Some(held) = self.decor_cache.insert(step, decor) {
            release_block(held.at, held.bytes);
        }
    }

    fn on_evict(&mut self, step: LodStep) {
        if let Some(t) = self.tiers.remove(&step) {
            release_block(t.at, t.bytes);
        }
        if let Some(d) = self.decor_cache.remove(&step) {
            release_block(d.at, d.bytes);
        }
    }

    /// The layout is the scheduler's to draw; what is checked here is that this
    /// node holds every tier it names. When it does not, the chunk starts over
    /// from its sources.
    fn on_layout(&mut self, primary: LodStep, secondary: Option<LodStep>, ready: bool) -> bool {
        if !ready {
            node_counts().released += 1;
            return true;
        }
        let holds = self.tiers.contains_key(&primary) && secondary.is_none_or(|s| self.tiers.contains_key(&s));
        if holds {
            node_counts().layouts += 1;
        }
        holds
    }
}

/// Applies what the scheduler told, and whatever that made it tell next.
pub fn run_chunk_events(events: impl IntoIterator<Item = Event>) {
    let mut queue: Queue = events.into_iter().collect();
    loop {
        while let Some(e) = queue.pop_front() {
            dispatch(e, &mut queue);
        }
        let ids = core::mem::take(&mut nodes().rebuild);
        if ids.is_empty() {
            break;
        }
        for id in ids {
            let Some(node) = nodes().nodes.get_mut(&id) else { continue };
            node.release_all();
            node.tell_source(&mut queue);
            node.build_now(&mut queue);
        }
    }
}

fn dispatch(e: Event, queue: &mut Queue) {
    let n = nodes();
    match e {
        Event::Mount { id, page, base_x, base_y } => {
            let mut node = ChunkNode::new(id, page, base_x, base_y);
            node.tell_source(queue);
            n.nodes.insert(id, node);
            node_counts().added += 1;
        }
        Event::Unmount { id } => {
            if let Some(mut node) = n.nodes.remove(&id) {
                node.release_all();
                node_counts().removed += 1;
            }
        }
        Event::Build(BuildEvent::Tier {
            id,
            generation,
            step,
            at,
            bytes,
            vertex_cnt,
            index_cnt,
        }) => {
            let mesh = MeshBlock {
                at: at as usize,
                bytes,
                vertex_cnt,
                index_cnt,
            };
            match n.nodes.get_mut(&id) {
                Some(node) => node.on_tier(generation, step, mesh),
                None => release_block(mesh.at, mesh.bytes),
            }
        }
        Event::Build(BuildEvent::Runs { id, bytes }) => {
            if let Some(node) = n.nodes.get_mut(&id) {
                node.runs_bytes = bytes;
            }
        }
        Event::Decor { id, step, at, bytes } => match n.nodes.get_mut(&id) {
            Some(node) => node.on_decor(step, DecorBlock { at: at as usize, bytes }),
            None => release_block(at as usize, bytes),
        },
        Event::Layout { id, primary, secondary, ready } => {
            let Some(node) = n.nodes.get_mut(&id) else { return };
            if !node.on_layout(primary, (secondary != 0).then_some(secondary), ready) {
                n.rebuild.push(id);
            }
        }
        Event::Evict { id, step } => {
            if let Some(node) = n.nodes.get_mut(&id) {
                node.on_evict(step);
            }
        }
        Event::Visible { .. } | Event::Blend { .. } => {}
    }
}

/// The nodes' turn: a settings change, a tile arriving or leaving, or an object
/// change reaches every node, which compares its own sources against them.
pub fn chunk_nodes_step() {
    let n = nodes();
    let seen = (use_settings_store().revision, tile_pages().revision(), objects_revision());
    if n.seen == Some(seen) {
        return;
    }
    n.seen = Some(seen);
    let mut queue = Queue::new();
    let ids: Vec<u32> = n.nodes.keys().copied().collect();
    for id in ids {
        if let Some(node) = nodes().nodes.get_mut(&id) {
            node.on_source_change(&mut queue);
            node.on_decor_change(&mut queue);
        }
    }
    run_chunk_events(queue);
}

/// Whether page `page_idx` has a terrain tile chunks can be mounted from.
pub fn chunk_page(page_idx: IVec, present: bool) {
    kernel().sched.page(page_idx, present);
}
