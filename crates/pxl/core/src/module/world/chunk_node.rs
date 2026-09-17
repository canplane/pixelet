// pxl_core/module/world/chunk_node.rs

//! One chunk: the tiers and decor blocks it holds, and the build that makes
//! them (the second half).
//!
//! Every decision about a chunk is the scheduler's
//! (crates/pxl/core/src/module/world/sched/chunk_table.rs): whether it is on screen, which tiers it
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

use std::collections::{BTreeMap, HashMap, VecDeque};

use pxl_shared::bake::constant::{CHUNK_CELL_SPAN, ELE_PADDED_SPAN, PAGE_CELL_SPAN};
use pxl_shared::bake::format::Tile;
use pxl_shared::spec::space::indexer::{FROM_L3_IDX, L2_IDX};
use pxl_shared::spec::space::pvf3::ivec::{self, IVec};
use xpute_core::sys::mem::{alloc, free, Block};

use crate::app::setting::store::settings_store;
use crate::module::world::object::store::{decor_col_revision, object_col_idx_of, object_col_revision, Store};
use crate::module::world::sched::chunk_table::Event;
use crate::module::world::terrain::store::tile_store;
use crate::render::layer::object::adapter::{Carve, Painter};
use crate::render::layer::pipeline::chunk_mesher::{mesh_words, BakeContext, Step};
use crate::render::layer::pipeline::volume::Volumes;
use crate::render::layer::pipeline::voxel_scan::{BandView, RunSet, ScanContext, ScanStep};
use crate::render::layer::terrain::adapter::{ChunkSample, Materials, PageView, CHANNELS};
use crate::render::lod::LodStep;
use crate::sys::global::Global;
use crate::sys::kernel::{kernel, with_world};

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
/// group's lanes (module/world/decor/instance.rs).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct DecorBlock {
    pub at: usize,
    pub bytes: u32,
}

/// Hands a block back to the mesh section's pool. Every view into it is dead
/// afterwards.
pub fn release_block(at: usize, bytes: u32) {
    // SAFETY: a block this chunk took and has not given back.
    unsafe { free((at, bytes)) };
}

/// How much built work chunks may keep *while off screen*: run sets and cached
/// tiers, both in the mesh section and sized in bytes. Leaving the frustum used
/// to drop all of it on the first release, and a rotation or a zoom in and out
/// rebuilt every chunk from the scan up; the scheduler reclaims only past this,
/// farthest first. It budgets only the optional part: on-screen chunks hold
/// their tiers because that is the layout. Half the mesh section: the other half
/// is the on-screen layouts, the volumes a build composes in, and the room a
/// large block needs in one piece — at a quarter the section fragments and
/// refuses the fine tiers' blocks.
#[allow(non_snake_case)]
pub fn RETAIN_BUDGET_BYTES() -> f64 {
    crate::mem::map().map_or(0.0, |m| (m.heap.bytes / 2) as f64)
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
    /// Set where a chunk's sources change — a column written, a terrain page
    /// come or gone, the two settings a chunk paints with — and taken when
    /// the nodes next look at their own.
    sources_changed: bool,
    /// Chunks whose layout named tiers they do not hold, rebuilt after the
    /// events that found it — a rebuild would re-enter them.
    rebuild: Vec<u32>,
}

static NODES: Global<Nodes> = Global::new();

fn nodes() -> &'static mut Nodes {
    NODES.get(|| Nodes {
        nodes: HashMap::new(),
        sources_changed: false,
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
            out.decor_instances += crate::module::world::decor::instance::decor_groups(words).map(|(_, n)| n).sum::<u32>();
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
    /// The column this chunk stands in, as the object store keys it.
    pub col_idx: IVec,
    id: u32,
    /// The tiers and the decor blocks the scheduler handed over, by step and
    /// generation: a source changed leaves the older ones held until the
    /// scheduler evicts them, when a layout of the newer is up.
    tiers: HashMap<(LodStep, u32), MeshBlock>,
    decor_cache: HashMap<(LodStep, u32), DecorBlock>,
    /// How many times the scheduler has been told this chunk's sources: a tier
    /// built from an earlier telling arriving after a newer one was told is
    /// released on arrival.
    generation: u32,
    /// What this chunk's runs hold in the mesh section, as the scheduler said.
    runs_bytes: u32,
    show_structures: bool,
    objects_revision: u32,
    decor_revision: u32,
    show_decor: bool,
}

type Queue = VecDeque<Event>;

fn page_lanes(tile: &Tile) -> PageLanes {
    let g = &tile.terrain.ground;
    PageLanes {
        ground: [
            g.land.as_slice().as_ptr() as usize,
            g.water.as_slice().as_ptr() as usize,
            g.structure.as_slice().as_ptr() as usize,
            g.transport.as_slice().as_ptr() as usize,
        ],
        ele: tile.terrain.ele.as_slice().as_ptr() as usize,
    }
}

impl ChunkNode {
    fn new(id: u32, page_idx: IVec, base_x: u32, base_y: u32) -> ChunkNode {
        let page_off = ivec::gather(base_x as i64, base_y as i64, 0);
        let col_idx = object_col_idx_of(page_idx, page_off);
        let settings = settings_store();
        ChunkNode {
            page_idx,
            base_i: ivec::add(FROM_L3_IDX(page_idx), page_off),
            col_idx,
            id,
            tiers: HashMap::new(),
            decor_cache: HashMap::new(),
            generation: 0,
            runs_bytes: 0,
            show_structures: settings.show_structures,
            objects_revision: object_col_revision(col_idx),
            decor_revision: decor_col_revision(col_idx),
            show_decor: settings.show_decor,
        }
    }

    /// Every tier's and every decor block back to the pool: the chunk unmounted.
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
        let page = tile_store().get_tile(self.page_idx).map(page_lanes);
        let (id, show) = (self.id, self.show_structures);
        with_world(|world, sched| sched.source(world, id, page, show, &mut |e| queue.push_back(e)));
        self.generation += 1;
    }

    /// A source change rebuilds what the chunk shows, not the whole visible
    /// world: the scheduler is told, keeps drawing what the chunk shows, and
    /// builds its layout ahead of every other chunk, swapping it in whole once
    /// every tier and decor of it is built. `objects_revision` is the
    /// SOLID-only revision; a solid change still rebuilds decor too, because a
    /// newly placed cube can occlude a scattered decor candidate.
    ///
    /// The terrain page is not a source that changes. A bake is the same
    /// whenever and wherever it is fetched — a page that differs is a broken
    /// bake, not a change — and a mounted chunk's page stays in its record
    /// until the chunk unmounts, so a page set again is the same lanes at the
    /// same addresses and nothing to rebuild.
    fn on_source_change(&mut self, queue: &mut Queue) {
        let show_structures = settings_store().show_structures;
        let objects_revision = object_col_revision(self.col_idx);
        if self.show_structures == show_structures && self.objects_revision == objects_revision {
            return;
        }
        self.show_structures = show_structures;
        self.objects_revision = objects_revision;
        self.tell_source(queue);
    }

    /// Decor's own revision-only rebuild. It must never touch the tiers: that
    /// would reintroduce the needless cube rebuild — visible as a flicker — this
    /// split exists to avoid.
    fn on_decor_change(&mut self, queue: &mut Queue) {
        let revision = decor_col_revision(self.col_idx);
        let show_decor = settings_store().show_decor;
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
        if generation != self.generation || self.tiers.contains_key(&(step, generation)) {
            release_block(mesh.at, mesh.bytes);
            return;
        }
        self.tiers.insert((step, generation), mesh);
    }

    /// The scheduler hands a step's decor over only where it believes this node
    /// holds none of that generation, so one already held is stale and gives
    /// way.
    fn on_decor(&mut self, step: LodStep, generation: u32, decor: DecorBlock) {
        if let Some(held) = self.decor_cache.insert((step, generation), decor) {
            release_block(held.at, held.bytes);
        }
    }

    fn on_evict(&mut self, step: LodStep, generation: u32) {
        if let Some(t) = self.tiers.remove(&(step, generation)) {
            release_block(t.at, t.bytes);
        }
        if let Some(d) = self.decor_cache.remove(&(step, generation)) {
            release_block(d.at, d.bytes);
        }
    }

    /// The layout is the scheduler's to draw; what is checked here is that this
    /// node holds every tier it names, of its generation. When it does not, the
    /// chunk starts over from its sources.
    fn on_layout(&mut self, primary: LodStep, secondary: Option<LodStep>, generation: u32, ready: bool) -> bool {
        if !ready {
            node_counts().released += 1;
            return true;
        }
        let holds = self.tiers.contains_key(&(primary, generation)) && secondary.is_none_or(|s| self.tiers.contains_key(&(s, generation)));
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
            node.tell_source(&mut queue);
        }
    }
}

fn dispatch(e: Event, queue: &mut Queue) {
    let n = nodes();
    match e {
        Event::Mount { id, page_idx, base_x, base_y } => {
            let mut node = ChunkNode::new(id, page_idx, base_x, base_y);
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
        Event::Decor { id, step, generation, at, bytes } => match n.nodes.get_mut(&id) {
            Some(node) => node.on_decor(step, generation, DecorBlock { at: at as usize, bytes }),
            None => release_block(at as usize, bytes),
        },
        Event::Layout {
            id,
            primary,
            secondary,
            generation,
            ready,
        } => {
            let Some(node) = n.nodes.get_mut(&id) else { return };
            if !node.on_layout(primary, (secondary != 0).then_some(secondary), generation, ready) {
                n.rebuild.push(id);
            }
        }
        Event::Evict { id, step, generation } => {
            if let Some(node) = n.nodes.get_mut(&id) {
                node.on_evict(step, generation);
            }
        }
        Event::Visible { .. } | Event::Blend { .. } => {}
    }
}

/// The nodes' turn: a settings change, a tile arriving or leaving, or an object
/// change reaches every node, which compares its own sources against them.
/// A chunk's sources changed somewhere: the nodes look at their own next turn.
pub fn chunk_sources_changed() {
    nodes().sources_changed = true;
}

pub fn chunk_nodes_step() {
    let n = nodes();
    if !n.sources_changed {
        return;
    }
    n.sources_changed = false;
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

// ---- the build as the kernel runs it ----
//
// A chunk's build, from its sources to its meshes, a chunk at a time under
// the frame's budget.
//
// The chunk nodes say which chunks exist, what each is made of (its terrain
// page's lanes and the settings that change what they paint), and which
// tiers it still lacks; each tick it hands over the order to take them in
// and a budget. A chunk's runs come the direct way — one run a column
// from the terrain, when no object stands in its column — or by painting
// the terrain and the objects into a volume and scanning it. Its tiers are
// baked from those runs, one bake and one scan open in the whole world at a
// time. What comes
// out is events: a tier's mesh in a block of the mesh section, handed over
// whole, and what the chunk's runs hold, for the retain budget.
//
// A block the pool refuses leaves the work where it is until a later tick;
// nothing is copied to a place the core cannot see.

/// Chunks held at once: the mount radius and its hysteresis cover about
/// three thousand. The mount pass stops
/// at it rather than trapping (`Scheduler::mount`), so `Scheduler::add`'s
/// assert past it says the caller has a bug, not that the view grew.
pub const MAX_CHUNKS: usize = 4096;

/// What the chunk nodes are told.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BuildEvent {
    /// Tier `step` of chunk `id`, built from its sources as of `generation`:
    /// the mesh is `bytes` bytes at `at`, the node's to release.
    Tier {
        id: u32,
        generation: u32,
        step: u32,
        at: u32,
        bytes: u32,
        vertex_cnt: u32,
        index_cnt: u32,
    },
    /// The bytes chunk `id`'s runs now hold, zero once let go.
    Runs { id: u32, bytes: u32 },
}

/// A terrain page's lanes, as addresses into the memory.
#[derive(Clone, Copy)]
pub struct PageLanes {
    pub ground: [usize; CHANNELS],
    pub ele: usize,
}

#[derive(Clone, Copy)]
struct Runs {
    at: usize,
    bytes: u32,
    count: u32,
    /// Made the direct way, which is cheap to make again.
    direct: bool,
}

#[derive(Clone, Copy)]
struct Chunk {
    /// The chunk's corner, as an L1 coordinate.
    base: IVec,
    /// The column it stands in, as the object store keys it. Derived from the
    /// corner — a chunk is one L2 span — and kept beside it because the two
    /// are different units and nothing but this would say so.
    col_idx: IVec,
    /// Where the chunk starts in its page, in cells.
    tx0: u32,
    ty0: u32,
    page: Option<PageLanes>,
    show_structures: bool,
    generation: u32,
    /// The tiers the node lacks, in the order it wants them; 0 is none.
    want: [u32; 2],
    runs: Option<Runs>,
    /// The mesh pool's release count when it last refused this chunk a
    /// block: until a block is given back, the same request meets the
    /// same pool, and the chunk is not tried.
    refused: Option<u32>,
}

impl Chunk {
    fn wants(&self, step: u32) -> bool {
        step != 0 && self.want.contains(&step)
    }

    fn wanted(&self) -> impl Iterator<Item = u32> + '_ {
        self.want.iter().copied().filter(|&s| s != 0)
    }
}

struct ScanJob {
    id: u32,
    volume: usize,
}

struct BakeJob {
    id: u32,
    step: u32,
    generation: u32,
    mesh: Option<Block>,
}

/// Where a chunk got to in one advance.
#[derive(PartialEq, Eq)]
enum Progress {
    /// Work was done and more remains.
    Busy,
    /// Nothing more to do for it now.
    Done,
    /// Waiting on something outside it: the slot another chunk holds, a
    /// block the pool refused, a page not yet arrived.
    Blocked,
}

/// Everything a build reads and writes that is not its own.
pub struct World<'a> {
    pub volumes: &'a mut Volumes,
    pub materials: &'a Materials,
    pub store: &'a Store,
    pub painter: &'a mut Painter,
}

/// The build counters the ledger reads.
#[derive(Clone, Copy, Default)]
pub struct BuildStat {
    pub direct: u32,
    pub scans: u32,
    pub bakes: u32,
    pub starved: u32,
}

pub struct Builder {
    chunks: BTreeMap<u32, Chunk>,
    scan: ScanContext,
    direct: ScanContext,
    bake: BakeContext,
    scan_job: Option<ScanJob>,
    bake_job: Option<BakeJob>,
    sample: ChunkSample,
    /// The drawn build's two edits: the hidden entry by its column, band
    /// and packed position, and the carve preview by its column.
    hide: Option<(IVec, i32, u32)>,
    carve: Option<(IVec, Carve)>,
    /// The scan job's bands, kept between the scan's steps.
    bands: Vec<BandView<'static>>,
    pub stat: BuildStat,
}

impl Default for Builder {
    fn default() -> Self {
        Builder::new()
    }
}

impl Builder {
    pub fn new() -> Builder {
        Builder {
            chunks: BTreeMap::new(),
            scan: ScanContext::new(),
            direct: ScanContext::new(),
            bake: BakeContext::new(),
            scan_job: None,
            bake_job: None,
            sample: ChunkSample::default(),
            hide: None,
            carve: None,
            bands: Vec::new(),
            stat: BuildStat::default(),
        }
    }

    /// What chunk `id` is made of, for a decor build: its corner, the column
    /// it stands in as the object store keys it, where it starts in its page,
    /// and its page's lanes.
    pub fn source_of(&self, id: u32) -> Option<(IVec, IVec, u32, u32, PageLanes, bool)> {
        let c = self.chunks.get(&id)?;
        Some((c.base, c.col_idx, c.tx0, c.ty0, c.page?, c.show_structures))
    }

    /// The chunk's composite of the real set — no edits — as a sealed volume
    /// for a raycast, or none: no page, or a paint refused. With the count
    /// its sources have been told, which a cached composite is valid for.
    pub fn compose_real(&mut self, world: &mut World, id: u32) -> Option<(usize, u32)> {
        let c = self.chunks.get(&id)?;
        let (page, generation) = (c.page?, c.generation);
        let volume = self.compose(world, id, page, false)?;
        Some((volume, generation))
    }

    pub fn generation_of(&self, id: u32) -> Option<u32> {
        self.chunks.get(&id).map(|c| c.generation)
    }

    /// The hidden entry's band and packed position, when it is in the column `key`.
    pub fn hide_in(&self, key: IVec) -> Option<(i32, u32)> {
        self.hide.filter(|h| h.0 == key).map(|h| (h.1, h.2))
    }

    /// The chunks holding the scan slot and the bake slot.
    pub fn holders(&self) -> [Option<u32>; 2] {
        [self.scan_job.as_ref().map(|j| j.id), self.bake_job.as_ref().map(|j| j.id)]
    }

    pub fn bake_context(&self) -> &BakeContext {
        &self.bake
    }

    // ---- commands ----

    pub fn add(&mut self, id: u32, base: IVec, tx0: u32, ty0: u32) {
        assert!(tx0 + CHUNK_CELL_SPAN <= PAGE_CELL_SPAN && ty0 + CHUNK_CELL_SPAN <= PAGE_CELL_SPAN, "a chunk past its page");
        self.chunks.insert(
            id,
            Chunk {
                base,
                col_idx: L2_IDX(base),
                tx0,
                ty0,
                page: None,
                show_structures: false,
                generation: 0,
                want: [0; 2],
                runs: None,
                refused: None,
            },
        );
    }

    pub fn remove(&mut self, world: &mut World, id: u32, emit: &mut impl FnMut(BuildEvent)) {
        self.drop_work(world, id, emit);
        self.chunks.remove(&id);
    }

    /// The chunk's sources changed: what it was built from is gone, and a
    /// tier built before now is stale.
    pub fn source(&mut self, world: &mut World, id: u32, page: Option<PageLanes>, show_structures: bool, emit: &mut impl FnMut(BuildEvent)) {
        self.drop_work(world, id, emit);
        if let Some(c) = self.chunks.get_mut(&id) {
            c.page = page;
            c.show_structures = show_structures;
            c.generation = c.generation.wrapping_add(1);
        }
    }

    /// Gives back the chunk's runs and its work in flight, keeping its
    /// sources: the retain budget's reclaim.
    pub fn reclaim(&mut self, world: &mut World, id: u32, emit: &mut impl FnMut(BuildEvent)) {
        self.drop_work(world, id, emit);
    }

    /// The tiers the chunk still lacks. A bake of a tier no longer wanted
    /// gives the slot back now.
    pub fn want(&mut self, world: &mut World, id: u32, want: [u32; 2]) {
        let Some(c) = self.chunks.get_mut(&id) else { return };
        c.want = want;
        let stale = matches!(&self.bake_job, Some(j) if j.id == id && !c.wants(j.step));
        if stale {
            self.abort_bake();
        }
        if want == [0, 0] {
            self.abort_scan_of(world, id);
        }
    }

    pub fn set_edits(&mut self, hide: Option<(IVec, i32, u32)>, carve: Option<(IVec, Carve)>) {
        self.hide = hide;
        self.carve = carve;
    }

    fn drop_work(&mut self, world: &mut World, id: u32, emit: &mut impl FnMut(BuildEvent)) {
        self.abort_scan_of(world, id);
        if matches!(&self.bake_job, Some(j) if j.id == id) {
            self.abort_bake();
        }
        if let Some(c) = self.chunks.get_mut(&id) {
            if let Some(r) = c.runs.take() {
                // SAFETY: a block this build took and has not given back.
                unsafe { free((r.at, r.bytes)) };
                emit(BuildEvent::Runs { id, bytes: 0 });
            }
        }
    }

    fn abort_scan_of(&mut self, world: &mut World, id: u32) {
        if let Some(job) = self.scan_job.take_if(|j| j.id == id) {
            self.scan.abort();
            world.volumes.release(job.volume);
        }
    }

    /// The pool refused chunk `id` a block: the slot it held is given up —
    /// held, it is every other chunk's wait, and the chunk cannot use it
    /// until the pool changes — and the chunk waits for a release.
    fn refuse(&mut self, world: &mut World, id: u32) {
        self.stat.starved += 1;
        self.abort_scan_of(world, id);
        if matches!(&self.bake_job, Some(j) if j.id == id) {
            self.abort_bake();
        }
        let releases = crate::sys::host::heap_releases();
        if let Some(c) = self.chunks.get_mut(&id) {
            c.refused = Some(releases);
        }
    }

    fn abort_bake(&mut self) {
        if let Some(job) = self.bake_job.take() {
            self.bake.abort();
            if let Some(mesh) = job.mesh {
                // SAFETY: a block this build took and has not given back.
                unsafe { free(mesh) };
            }
        }
    }

    // ---- the tick ----

    /// Advances the chunks in `order` under `budget_ms` of `now`, the
    /// holders of the scan and the bake slot first: a slot held is every
    /// other chunk's wait. At least one step is taken, whatever the budget.
    pub fn tick(&mut self, world: &mut World, order: &[u32], budget_ms: f64, now: &impl Fn() -> f64, emit: &mut impl FnMut(BuildEvent)) {
        self.tick_capped(world, order, budget_ms, u32::MAX, now, emit);
    }

    /// `tick`, stopping once `max_chunks` chunks have made progress: the
    /// scheduler's cap on how many chunks move in a frame. A chunk that
    /// waits on a slot or a block does not count. A block refused ends the
    /// tick: the pool does not change until the scheduler reclaims after,
    /// so every later request would meet the same pool, and a refusal is
    /// not free — the direct runs are written whole before their block is
    /// asked for. Whether any chunk did work.
    pub fn tick_capped(&mut self, world: &mut World, order: &[u32], budget_ms: f64, max_chunks: u32, now: &impl Fn() -> f64, emit: &mut impl FnMut(BuildEvent)) -> bool {
        let t0 = now();
        let starved = self.stat.starved;
        let mut stepped = false;
        let mut worked = false;
        let mut moved = 0;
        let holders = self.holders();
        let [scan_holder, bake_holder] = holders;
        let held = |id: u32| Some(id) == scan_holder || Some(id) == bake_holder;
        let holders_once = if scan_holder == bake_holder { &holders[..1] } else { &holders[..] };
        // The holders are at most two ids, so the ones `order` repeats are
        // found by comparing against them, not by a list of everything seen.
        for id in holders_once.iter().flatten().copied().chain(order.iter().copied().filter(|&id| !held(id))) {
            if moved >= max_chunks {
                return worked;
            }
            let mut progressed = false;
            loop {
                if stepped && now() - t0 >= budget_ms {
                    return worked;
                }
                let progress = self.advance(world, id, emit);
                if self.stat.starved != starved {
                    return worked;
                }
                if progress != Progress::Blocked {
                    stepped = true;
                }
                if progress != Progress::Busy {
                    break;
                }
                worked = true;
                progressed = true;
            }
            if progressed {
                moved += 1;
            }
        }
        worked
    }

    fn advance(&mut self, world: &mut World, id: u32, emit: &mut impl FnMut(BuildEvent)) -> Progress {
        let Some(c) = self.chunks.get(&id) else { return Progress::Done };
        if c.wanted().next().is_none() {
            return Progress::Done;
        }
        if c.refused.is_some_and(|r| r == crate::sys::host::heap_releases()) {
            return Progress::Blocked;
        }
        if c.runs.is_none() {
            return self.advance_runs(world, id, emit);
        }
        self.advance_bake(world, id, emit)
    }

    fn advance_runs(&mut self, world: &mut World, id: u32, emit: &mut impl FnMut(BuildEvent)) -> Progress {
        let c = &self.chunks[&id];
        let Some(page) = c.page else { return Progress::Blocked };
        let holding = matches!(&self.scan_job, Some(j) if j.id == id);
        // A column with nothing placed is one run of terrain, floor to top,
        // whichever way it is made: the direct runs and the scan bake the
        // same mesh at every tier, and the scan's
        // exactness — a cube carving the terrain, a sub-cell object — is
        // only ever about what is placed, so the direct path serves every
        // tier: a zoom-in would otherwise scan every chunk in view, most of
        // them empty of objects.
        if !holding && !self.has_objects(world, c.col_idx) {
            return self.direct_runs(world, id, page, emit);
        }
        if !holding {
            if self.scan_job.is_some() {
                return Progress::Blocked;
            }
            match self.compose(world, id, page, true) {
                Some(volume) => {
                    // SAFETY: the volume was just composed and is held by the
                    // scan job until the scan finishes.
                    unsafe { world.volumes.band_views(volume, &mut self.bands) };
                    assert!(self.scan.begin(&self.bands), "a scan open with no job naming it");
                    self.scan_job = Some(ScanJob { id, volume });
                }
                None => {
                    self.refuse(world, id);
                    return Progress::Blocked;
                }
            }
        }
        let volume = self.scan_job.as_ref().unwrap().volume;
        // SAFETY: the scan job's volume, held until the scan finishes.
        unsafe { world.volumes.band_views(volume, &mut self.bands) };
        match self.scan.step(&self.bands) {
            ScanStep::Working => Progress::Busy,
            ScanStep::Done => unreachable!("a scan is finished when its set is provided"),
            ScanStep::NeedSet(bytes) => {
                let Some((at, bytes)) = alloc(bytes) else {
                    self.refuse(world, id);
                    return Progress::Blocked;
                };
                // SAFETY: a block of the bytes asked for, the chunk's until released.
                unsafe { self.scan.provide_set(world.volumes.mem().add(at), bytes) };
                let count = self.scan.finish();
                let job = self.scan_job.take().unwrap();
                world.volumes.release(job.volume);
                self.stat.scans += 1;
                self.keep_runs(id, Runs { at, bytes, count, direct: false }, emit);
                Progress::Busy
            }
        }
    }

    fn has_objects(&self, world: &World, col_idx: IVec) -> bool {
        world.store.chunk_lanes(col_idx).is_some_and(|l| l.count > 0) || matches!(self.carve, Some((k, _)) if k == col_idx)
    }

    /// The page's lanes as slices.
    ///
    /// # Safety
    /// `page` names lanes of a page's fixed shape inside the world's memory,
    /// resident for as long as the view is read: what the chunk node promises
    /// when it tells a chunk its source.
    pub unsafe fn page_view(world: &World, page: PageLanes) -> PageView<'static> {
        let mem = world.volumes.mem();
        let cells = (PAGE_CELL_SPAN * PAGE_CELL_SPAN) as usize;
        let samples = (ELE_PADDED_SPAN * ELE_PADDED_SPAN) as usize;
        PageView {
            ground: page.ground.map(|at| core::slice::from_raw_parts(mem.add(at), cells)),
            ele: core::slice::from_raw_parts(mem.add(page.ele) as *const i16, samples),
        }
    }

    /// Paints the terrain and then the objects into a new volume, sealed;
    /// none when a paint was refused. With `edits`, the drawn build's hide
    /// and carve; without, the real set — what a raycast tests.
    fn compose(&mut self, world: &mut World, id: u32, page: PageLanes, edits: bool) -> Option<usize> {
        let c = &self.chunks[&id];
        let (col_idx, tx0, ty0, show) = (c.col_idx, c.tx0, c.ty0, c.show_structures);
        // SAFETY: the chunk's page, as its source named it, resident while
        // the chunk is.
        let view = unsafe { Self::page_view(world, page) };
        self.sample.read(&view, world.materials, tx0, ty0, show);
        let volume = world.volumes.open();
        let mut ok = self.sample.paint(world.volumes, volume);
        if ok {
            let hide = self.hide.filter(|h| edits && h.0 == col_idx).map(|h| (h.1, h.2));
            let carve = self.carve.filter(|c| edits && c.0 == col_idx).map(|c| c.1);
            ok = !world.painter.paint(world.store, world.volumes, volume, col_idx, hide, carve).refused;
        }
        if !ok {
            world.volumes.release(volume);
            return None;
        }
        world.volumes.seal(volume);
        Some(volume)
    }

    fn direct_runs(&mut self, world: &mut World, id: u32, page: PageLanes, emit: &mut impl FnMut(BuildEvent)) -> Progress {
        let c = &self.chunks[&id];
        let (tx0, ty0, show) = (c.tx0, c.ty0, c.show_structures);
        // SAFETY: the chunk's page, as its source named it, resident while
        // the chunk is.
        let view = unsafe { Self::page_view(world, page) };
        self.sample.read(&view, world.materials, tx0, ty0, show);
        assert!(self.direct.begin_direct(), "the direct writer left open");
        let floor = self.sample.floor;
        for i in 0..(CHUNK_CELL_SPAN * CHUNK_CELL_SPAN) as usize {
            self.direct.direct_column(i as u32);
            let top = self.sample.ele[i] as i32 - 1;
            if top >= floor {
                self.direct.direct_run(floor, top, self.sample.material[i], self.sample.layer_idx[i]);
            }
        }
        let bytes = self.direct.close_direct();
        let Some((at, bytes)) = alloc(bytes) else {
            self.direct.abort();
            self.refuse(world, id);
            return Progress::Blocked;
        };
        // SAFETY: as for a scan's set.
        unsafe { self.direct.provide_set(world.volumes.mem().add(at), bytes) };
        let count = self.direct.finish();
        self.stat.direct += 1;
        self.keep_runs(id, Runs { at, bytes, count, direct: true }, emit);
        Progress::Busy
    }

    fn keep_runs(&mut self, id: u32, runs: Runs, emit: &mut impl FnMut(BuildEvent)) {
        emit(BuildEvent::Runs { id, bytes: runs.bytes });
        self.chunks.get_mut(&id).unwrap().runs = Some(runs);
    }

    fn run_set(world: &World, runs: &Runs) -> RunSet<'static> {
        let bytes = crate::render::layer::pipeline::voxel_scan::run_set_bytes(runs.count, CHUNK_CELL_SPAN) as usize;
        // SAFETY: the chunk's own block, held until its runs are let go.
        unsafe { RunSet::carve(core::slice::from_raw_parts(world.volumes.mem().add(runs.at), bytes), runs.count, CHUNK_CELL_SPAN) }
    }

    fn advance_bake(&mut self, world: &mut World, id: u32, emit: &mut impl FnMut(BuildEvent)) -> Progress {
        let holding = matches!(&self.bake_job, Some(j) if j.id == id);
        let c = &self.chunks[&id];
        let set = Self::run_set(world, c.runs.as_ref().unwrap());
        if !holding {
            if self.bake_job.is_some() {
                return Progress::Blocked;
            }
            let step = c.wanted().next().unwrap();
            assert!(self.bake.begin(&set, step), "a bake open with no job naming it");
            self.bake_job = Some(BakeJob {
                id,
                step,
                generation: c.generation,
                mesh: None,
            });
        }
        match self.bake.step(&set) {
            Step::Working => Progress::Busy,
            Step::NeedMesh(words) => {
                let Some(mesh) = alloc(words * 4) else {
                    self.refuse(world, id);
                    return Progress::Blocked;
                };
                // SAFETY: a block of the words asked for, the bake's until it
                // is finished and handed over, or aborted and released.
                unsafe { self.bake.provide_mesh(world.volumes.mem().add(mesh.0) as *mut u32, words) };
                self.bake_job.as_mut().unwrap().mesh = Some(mesh);
                Progress::Busy
            }
            Step::Done => {
                let job = self.bake_job.as_ref().unwrap();
                // A mesh of nothing never asked for a block, and is handed
                // over as none: zero bytes at zero.
                debug_assert!(job.mesh.is_some() || mesh_words(0, 0) == 0);
                let (at, bytes) = job.mesh.unwrap_or((0, 0));
                let mesh = self.bake.finish();
                let job = self.bake_job.take().unwrap();
                self.stat.bakes += 1;
                let c = self.chunks.get_mut(&id).unwrap();
                for w in c.want.iter_mut() {
                    if *w == job.step {
                        *w = 0;
                    }
                }
                // Direct runs are not worth keeping past the last tier
                // wanted: they are cheap to make again, and held they are small
                // blocks the fine tiers' large ones fail to find room among. A
                // scan's runs stay: those cost the scan.
                if c.wanted().next().is_none() {
                    if let Some(r) = c.runs.take_if(|r| r.direct) {
                        // SAFETY: a block this build took and has not given back.
                        unsafe { free((r.at, r.bytes)) };
                        emit(BuildEvent::Runs { id, bytes: 0 });
                    }
                }
                emit(BuildEvent::Tier {
                    id,
                    generation: job.generation,
                    step: job.step,
                    at: at as u32,
                    bytes,
                    vertex_cnt: mesh.vertex_cnt,
                    index_cnt: mesh.index_cnt,
                });
                Progress::Busy
            }
        }
    }
}
