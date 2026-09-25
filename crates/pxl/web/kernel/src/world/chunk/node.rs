// pxl-kernel/world/chunk/node.rs

//! One chunk: the tiers and decor blocks it holds, and the build that makes
//! them (the second half).
//!
//! Every decision about a chunk is the scheduler's
//! (crates/pxl/web/kernel/src/world/chunk/table.rs): whether it is on screen, which tiers it
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

use pxl_spec::space::pyramid::{chunk_span_of, Cell, CHUNK_CELLS, GROUND, PAGE_CELLS};
use std::collections::{BTreeMap, BTreeSet, HashMap, VecDeque};

use pxl_render::look::lod::LodStep;
use pxl_render::mesh::chunk_mesher::{mesh_words, BakeContext, Heights, Step, FINE_BOX_WORDS};
use pxl_render::mesh::paint::level::write_level_runs;
use pxl_render::mesh::paint::object::adapter::{Carve, Painted, Painter};
use pxl_render::mesh::paint::object::fold::{fold_columns, Column};
use pxl_render::mesh::paint::terrain::adapter::{ChunkSample, Materials, PageView};
use pxl_render::mesh::volume::Volumes;
use pxl_render::mesh::voxel_scan::{Apron, BandView, RunSet, ScanContext, ScanStep, APRON, COL_SPAN};
use pxl_spec::bake::constant::ELE_PADDED_SPAN;
use pxl_spec::bake::pack::GroundCell;
use pxl_spec::space::indexer::{FROM_L2_IDX, FROM_L3_IDX, L2_IDX};
use pxl_spec::space::pvf3::ivec::{self, IVec};
use xpute_core::math::scalar::round;
use xpute_runtime::mem::heap::{free, Block};

use crate::global::Global;
use crate::kernel::{kernel, with_world};
use crate::layer::object::store::{decor_col_revision, object_col_revision, object_cols, Store, BAND_CELLS};
use crate::mem::fixed::{bake_at, run_set_at};
use crate::shell::setting::store::{hidden_ground, settings_store};
use crate::world::chunk::mesh_slot::BAKE_BYTES;

use crate::world::chunk::table::Event;
use crate::world::pyramid::Layer as _;
use pxl_spec::bake::format::Terrain;
use pxl_spec::space::pyramid::LevelPage;

/// A tier as the node holds it: its boxes, which lie on the device in the
/// chunk's slot (mesh_slot.rs) and nowhere in the memory.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct MeshBlock {
    pub box_cnt: u32,
    /// What its boxes' heights are held in (pxl-render mesh/chunk_mesher.rs).
    pub heights: Heights,
}

/// A chunk's decor at one tier: one heap block holding every
/// group's lanes (layer/decor/instance.rs).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct DecorBlock {
    pub at: usize,
    pub bytes: u32,
}

/// Hands a block back to the heap. Every view into it is dead
/// afterwards.
pub fn release_block(at: usize, bytes: u32) {
    // SAFETY: a block this chunk took and has not given back.
    unsafe { free((at, bytes)) };
}

/// How much built work chunks may keep in the heap *while off screen*: their
/// decor, the one part of a chunk still there — its runs are a set of
/// `.fixed.runs` and its tiers are on the device. Dropping all of it on the
/// first release makes a rotation or a zoom in and out scatter every chunk's
/// decor again, so the scheduler reclaims only past this, farthest first. Half
/// the heap: the other half is the on-screen decor, the volumes a build
/// composes in, and the room a large block needs in one piece.
#[allow(non_snake_case)]
pub fn RETAIN_BUDGET_BYTES() -> f64 {
    crate::mem::section::map().map_or(0.0, |m| (m.heap.bytes / 2) as f64)
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
    /// Nodes holding a run set, and what those sets cost in the heap.
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

/// What the chunk nodes hold, with the run sets `.fixed.runs` has lent as the
/// builder counts them (`Builder::run_sets`).
pub fn chunk_retention(run_sets: (u32, u32)) -> ChunkRetention {
    let mut out = ChunkRetention::default();
    let (held, run_bytes) = run_sets;
    out.run_sets = held;
    for node in nodes().nodes.values() {
        out.nodes += 1;
        out.tiers += node.tiers.len() as u32;
        out.decor_steps += node.decor_cache.len() as u32;
        for d in node.decor_cache.values() {
            // SAFETY: the block the scheduler wrote at `at`, `bytes` long, held
            // for as long as the node holds this decor.
            let words: &[u32] = unsafe { core::slice::from_raw_parts(d.at as *const u32, d.bytes as usize / 4) };
            out.decor_instances += crate::layer::decor::instance::decor_groups(words).map(|(_, n)| n).sum::<u32>();
        }
    }
    out.run_set_mb = round(run_bytes as f64 / 1_048_576.0) as u32;
    out
}

pub struct ChunkNode {
    /// The page this chunk is drawn from.
    pub page: LevelPage,
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
    show_structures: bool,
    /// The texture setting the tiers were baked under. A leaf's box carries
    /// its layer and the shader samples or does not, but a tier over it
    /// carries one colour and no layer, so which colour that is was decided
    /// when it was baked (mesh/chunk_mesher.rs).
    textured: bool,
    objects_revision: u32,
    decor_revision: u32,
    show_decor: bool,
}

type Queue = VecDeque<Event>;

fn page_lanes(tile: &Terrain) -> PageLanes {
    PageLanes {
        cells: tile.ground.cells.as_ptr() as usize,
        ele: tile.ele.as_ptr() as usize,
    }
}

impl ChunkNode {
    fn new(id: u32, page: LevelPage, base_x: u32, base_y: u32) -> ChunkNode {
        let page_off = ivec::gather((base_x as i64) << page.level, (base_y as i64) << page.level, 0);
        let base_i = ivec::add(FROM_L3_IDX(page.first_leaf()), page_off);
        let col_idx = L2_IDX(base_i);
        let settings = settings_store();
        ChunkNode {
            page,
            base_i,
            col_idx,
            id,
            tiers: HashMap::new(),
            decor_cache: HashMap::new(),
            generation: 0,
            show_structures: settings.show_structures,
            textured: settings.texture_pack,
            objects_revision: objects_revision_of(page.level, base_i),
            decor_revision: decor_col_revision(col_idx),
            show_decor: settings.show_decor,
        }
    }

    /// Every decor block back to the heap, and the tiers forgotten: the chunk
    /// unmounted.
    fn release_all(&mut self) {
        self.tiers.clear();
        for (_, d) in self.decor_cache.drain() {
            release_block(d.at, d.bytes);
        }
    }

    /// Tells the scheduler what this chunk is made of now.
    fn tell_source(&mut self) {
        let page = Terrain::pyramid().get(self.page).map(page_lanes);
        let (id, show) = (self.id, self.show_structures);
        with_world(|world, sched| sched.source(world, id, page, show));
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
    fn on_source_change(&mut self) {
        let show_structures = settings_store().show_structures;
        let textured = settings_store().texture_pack;
        let objects_revision = objects_revision_of(self.page.level, self.base_i);
        if self.show_structures == show_structures && self.textured == textured && self.objects_revision == objects_revision {
            return;
        }
        self.show_structures = show_structures;
        self.textured = textured;
        self.objects_revision = objects_revision;
        self.tell_source();
    }

    /// Decor's own revision-only rebuild. It must never touch the tiers: that
    /// would reintroduce the needless cube rebuild — visible as a flicker — this
    /// split exists to avoid.
    fn on_decor_change(&mut self, queue: &mut Queue) {
        // Decor is scattered from the leaves alone: a kind's extent puts it
        // inside the reach level 0 is drawn to, so a level chunk has none.
        if self.page.level > 0 {
            return;
        }
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
        self.tiers.remove(&(step, generation));
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
            dispatch(e);
        }
        let ids = core::mem::take(&mut nodes().rebuild);
        if ids.is_empty() {
            break;
        }
        for id in ids {
            let Some(node) = nodes().nodes.get_mut(&id) else { continue };
            node.tell_source();
        }
    }
}

fn dispatch(e: Event) {
    let n = nodes();
    match e {
        Event::Mount { id, page, base_x, base_y } => {
            let mut node = ChunkNode::new(id, page, base_x, base_y);
            node.tell_source();
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
            box_cnt,
            heights,
            ..
        }) => {
            if let Some(node) = n.nodes.get_mut(&id) {
                node.on_tier(generation, step, MeshBlock { box_cnt, heights });
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
            node.on_source_change();
            node.on_decor_change(&mut queue);
        }
    }
    run_chunk_events(queue);
}

/// Whether page `page_idx` has a terrain tile chunks can be mounted from.
pub fn chunk_page(page: LevelPage, present: bool) {
    kernel().sched.page(page, present);
}

/// What a chunk's objects are made of, as a revision: its column's for a
/// leaf chunk, and every column under a level chunk and its apron summed,
/// since what it draws is all of theirs folded.
fn objects_revision_of(level: u32, base: IVec) -> u32 {
    if level == 0 {
        return object_col_revision(L2_IDX(base));
    }
    object_cols()
        .into_iter()
        .filter(|&c| folds_into(level, base, c))
        .fold(0u32, |acc, c| acc.wrapping_add(object_col_revision(c)))
}

/// Whether the objects of column `col` fold into the level-`level` chunk at
/// `base`: its own cells, and the one-cell apron around them a fold reads.
fn folds_into(level: u32, base: IVec, col: IVec) -> bool {
    // In units: a cell of the level, the chunk, and a column, which is a
    // leaf chunk.
    let (s, span, w) = (GROUND.scale(level).side(), chunk_span_of(level), chunk_span_of(0));
    let (o, x0, y0) = (FROM_L2_IDX(col), ivec::X(base) as f64, ivec::Y(base) as f64);
    let (ox, oy) = (ivec::X(o) as f64, ivec::Y(o) as f64);
    ox + w > x0 - s && ox < x0 + span + s && oy + w > y0 - s && oy < y0 + span + s
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
// time. What comes out is events: a tier's mesh, handed over whole, and what
// the chunk's runs hold.
//
// A chunk's runs are one of `.fixed.runs`'s two sets, held from the moment
// they are made until every tier of the chunk is baked from them: the tiers
// are what lasts, on the device, so a set is not kept for a tier asked later
// but the chunk's tiers are all baked while it is there. Two, so that one is
// filled while the other is baked from. A chunk that finds both held waits.

/// Run sets `.fixed.runs` holds.
pub const RUN_SETS: usize = 2;
/// Bytes a run set holds: a leaf chunk's columns at eleven runs each, where
/// the terrain is one and what is placed on it rarely a handful more. A set
/// past it is refused rather than cut, since a run left out is a hole.
pub const RUN_SET_BYTES: u32 = 1 << 20;

/// Chunks held at once: the mount radius and its hysteresis cover about
/// three thousand. The mount pass stops
/// at it rather than trapping (`Scheduler::mount`), so `Scheduler::add`'s
/// assert past it says the caller has a bug, not that the view grew.
pub const MAX_CHUNKS: usize = 4096;

/// What the chunk nodes are told.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum BuildEvent {
    /// Tier `step` of chunk `id`, built from its sources as of `generation`:
    /// the mesh is `bytes` bytes at `at` in `.fixed.bake`, until it is sent
    /// to its slot on the device, and its boxes' heights are held in
    /// `heights` (pxl-render mesh/chunk_mesher.rs).
    Tier {
        id: u32,
        generation: u32,
        step: u32,
        at: u32,
        bytes: u32,
        box_cnt: u32,
        heights: Heights,
    },
}

/// A leaf column, as a decor build reads it (Builder::leaf_source).
#[derive(Clone, Copy)]
pub struct LeafSource {
    pub page: PageLanes,
    pub tx0: u32,
    pub ty0: u32,
    pub show_structures: bool,
}

/// A terrain page's lanes, as addresses into the memory.
#[derive(Clone, Copy)]
pub struct PageLanes {
    /// The page's ground cells, packed (pxl-spec bake/pack.rs).
    pub cells: usize,
    pub ele: usize,
}

/// A chunk's runs: which of `.fixed.runs`'s sets, and what is in it.
#[derive(Clone, Copy)]
struct Runs {
    set: usize,
    bytes: u32,
    count: u32,
}

#[derive(Clone, Copy)]
struct Chunk {
    /// The level of the page it is drawn from: its cells are 2^level units.
    level: u32,
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
    /// The heap's release count when it last refused this chunk a
    /// block: until a block is given back, the same request meets the
    /// same heap, and the chunk is not tried.
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

#[derive(Clone, Copy)]
struct ScanJob {
    id: u32,
    volume: u32,
    /// The objects' paint into the volume, until it is finished and the scan
    /// begins.
    paint: Option<PaintJob>,
}

/// A chunk's objects painted a step at a time: the entry the next step
/// starts at; the column's revisions it began on, since a store written
/// between steps has moved the entries under it and the paint begins again;
/// and the drawn build's edits as they were then, as a source change for
/// new ones drops the job anyway.
#[derive(Clone, Copy)]
struct PaintJob {
    next: usize,
    revision: (u32, u32),
    hide: Option<(i32, u32)>,
    carve: Option<Carve>,
}

/// The voxels a step of a paint sets before it stops: a band's cells, which
/// is also the most one entry sets, so a step sets at most twice this
/// whatever the column holds.
const PAINT_VOXELS: usize = (BAND_CELLS * BAND_CELLS) as usize;

fn column_revision(col_idx: IVec) -> (u32, u32) {
    (object_col_revision(col_idx), decor_col_revision(col_idx))
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
    /// block the heap refused, a page not yet arrived.
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
    /// The chunk holding each run set, whether it is being filled or baked
    /// from.
    run_holders: [Option<u32>; RUN_SETS],
    /// The scan job's bands, kept between the scan's steps.
    bands: Vec<BandView<'static>>,
    /// The scan job's paint's claimed voxels, kept between its steps.
    claimed: BTreeSet<u32>,
    /// A level chunk's objects folded, kept for its capacity.
    folded: HashMap<[i64; 3], u32>,
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
            run_holders: [None; RUN_SETS],
            bands: Vec::new(),
            claimed: BTreeSet::new(),
            folded: HashMap::new(),
            stat: BuildStat::default(),
        }
    }

    /// The leaf column at `base`, a chunk corner, as a decor build reads it:
    /// its page's lanes and where it starts in them, and the structure
    /// setting. Resident or not, whichever level draws the ground there.
    pub fn leaf_source(&self, base: IVec) -> Option<LeafSource> {
        let cell = Cell::of(0, &pxl_spec::space::pvf3::Pvf3 { i: base, ..Default::default() });
        let (leaf, first) = (cell.page(), cell.index());
        let page = Terrain::pyramid().get(leaf).map(page_lanes)?;
        Some(LeafSource {
            page,
            tx0: first % PAGE_CELLS,
            ty0: first / PAGE_CELLS,
            show_structures: settings_store().show_structures,
        })
    }

    /// What chunk `id` is made of, for a decor build: its corner, the column
    /// it stands in as the object store keys it, where it starts in its page,
    /// and its page's lanes.
    pub fn source_of(&self, id: u32) -> Option<(IVec, IVec, u32, u32, PageLanes, bool)> {
        let c = self.chunks.get(&id).filter(|c| c.level == 0)?;
        Some((c.base, c.col_idx, c.tx0, c.ty0, c.page?, c.show_structures))
    }

    /// What a leaf composite at `base` is valid for: its column's objects and
    /// the structure setting, the two things its paint reads that change.
    pub fn leaf_valid(&self, base: IVec) -> u32 {
        object_col_revision(L2_IDX(base)).wrapping_mul(2) | settings_store().show_structures as u32
    }

    /// The leaf composite at `base`, a chunk corner — the real set, no edits —
    /// as a sealed volume for a raycast, or none: no leaf there, or a paint
    /// refused. Made from the leaf page wherever it is resident, whether or
    /// not a chunk is drawn from it: the ground is hit at the finest level
    /// whichever level it is drawn at. With what it is valid for, which a
    /// cached composite is compared by.
    pub fn compose_leaf(&mut self, world: &mut World, base: IVec) -> Option<(u32, u32)> {
        let cell = Cell::of(0, &pxl_spec::space::pvf3::Pvf3 { i: base, ..Default::default() });
        let (leaf, first) = (cell.page(), cell.index());
        let page = Terrain::pyramid().get(leaf).map(page_lanes)?;
        let (tx0, ty0) = (first % PAGE_CELLS, first / PAGE_CELLS);
        let show = settings_store().show_structures;
        let col_idx = L2_IDX(base);
        let valid = self.leaf_valid(base);
        let volume = self.paint_terrain_at(world, tx0, ty0, show, page)?;
        if world.painter.paint(world.store.entries(col_idx), world.volumes, volume, None, None).refused {
            world.volumes.release(volume);
            return None;
        }
        world.volumes.seal(volume);
        Some((volume, valid))
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

    pub fn add(&mut self, id: u32, level: u32, base: IVec, tx0: u32, ty0: u32) {
        assert!(tx0 + CHUNK_CELLS <= PAGE_CELLS && ty0 + CHUNK_CELLS <= PAGE_CELLS, "a chunk past its page");
        self.chunks.insert(
            id,
            Chunk {
                level,
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

    pub fn remove(&mut self, world: &mut World, id: u32) {
        self.drop_work(world, id);
        self.chunks.remove(&id);
    }

    /// The chunk's sources changed: what it was built from is gone, and a
    /// tier built before now is stale.
    pub fn source(&mut self, world: &mut World, id: u32, page: Option<PageLanes>, show_structures: bool) {
        self.drop_work(world, id);
        if let Some(c) = self.chunks.get_mut(&id) {
            c.page = page;
            c.show_structures = show_structures;
            c.generation = c.generation.wrapping_add(1);
        }
    }

    /// The tiers the chunk still lacks. A bake of a tier not wanted
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
            self.let_runs_go(id);
        }
    }

    pub fn set_edits(&mut self, hide: Option<(IVec, i32, u32)>, carve: Option<(IVec, Carve)>) {
        self.hide = hide;
        self.carve = carve;
    }

    fn drop_work(&mut self, world: &mut World, id: u32) {
        self.abort_scan_of(world, id);
        if matches!(&self.bake_job, Some(j) if j.id == id) {
            self.abort_bake();
        }
        self.let_runs_go(id);
    }

    fn abort_scan_of(&mut self, world: &mut World, id: u32) {
        if let Some(job) = self.scan_job.take_if(|j| j.id == id) {
            self.scan.abort();
            world.volumes.release(job.volume);
            self.let_runs_go(id);
        }
    }

    /// A run set for chunk `id`: the one it holds, or a free one it now
    /// does. None while both are held by others.
    fn run_set_for(&mut self, id: u32) -> Option<usize> {
        let set = self.run_holders.iter().position(|&h| h == Some(id)).or_else(|| self.run_holders.iter().position(|h| h.is_none()))?;
        self.run_holders[set] = Some(id);
        Some(set)
    }

    /// Gives back the run set chunk `id` holds, if any.
    fn let_runs_go(&mut self, id: u32) {
        for h in self.run_holders.iter_mut().filter(|h| **h == Some(id)) {
            *h = None;
        }
        if let Some(c) = self.chunks.get_mut(&id) {
            c.runs = None;
        }
    }

    /// The run sets lent now, and the bytes their runs fill.
    pub fn run_sets(&self) -> (u32, u32) {
        let held = self.run_holders.iter().flatten();
        let bytes = held.clone().filter_map(|id| self.chunks.get(id)?.runs.map(|r| r.bytes)).sum();
        (held.count() as u32, bytes)
    }

    /// The heap refused chunk `id` a block: the slots it held are given up —
    /// held, each is every other chunk's wait, and the chunk cannot use them
    /// until the heap changes — and the chunk waits for a release.
    fn refuse(&mut self, world: &mut World, id: u32) {
        self.stat.starved += 1;
        self.abort_scan_of(world, id);
        if matches!(&self.bake_job, Some(j) if j.id == id) {
            self.abort_bake();
        }
        self.let_runs_go(id);
        let releases = crate::host::heap_stat().releases;
        if let Some(c) = self.chunks.get_mut(&id) {
            c.refused = Some(releases);
        }
    }

    fn abort_bake(&mut self) {
        if self.bake_job.take().is_some() {
            self.bake.abort();
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
    /// tick: the heap does not change until the scheduler reclaims after,
    /// so every later request would meet the same heap, and a refusal is
    /// not free — the direct runs are written whole before their block is
    /// asked for. Whether any chunk did work.
    pub fn tick_capped(&mut self, world: &mut World, order: &[u32], budget_ms: f64, max_chunks: u32, now: &impl Fn() -> f64, emit: &mut impl FnMut(BuildEvent)) -> bool {
        let t0 = now();
        let starved = self.stat.starved;
        let mut stepped = false;
        let mut worked = false;
        let mut moved = 0;
        // The chunks holding the scan, the bake or a run set go first, each
        // once: held, a slot is every other chunk's wait.
        let mut first = [None; 2 + RUN_SETS];
        for (n, id) in self.holders().into_iter().chain(self.run_holders).enumerate() {
            if !first[..n].contains(&id) {
                first[n] = id;
            }
        }
        let is_first = |id: u32| first.contains(&Some(id));
        for id in first.iter().flatten().copied().chain(order.iter().copied().filter(|&id| !is_first(id))) {
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
        if c.refused.is_some_and(|r| r == crate::host::heap_stat().releases) {
            return Progress::Blocked;
        }
        if c.runs.is_none() {
            return self.advance_runs(world, id);
        }
        self.advance_bake(world, id, emit)
    }

    fn advance_runs(&mut self, world: &mut World, id: u32) -> Progress {
        let c = &self.chunks[&id];
        let Some(page) = c.page else { return Progress::Blocked };
        if c.level > 0 {
            return self.level_runs(world, id, page);
        }
        let holding = matches!(&self.scan_job, Some(j) if j.id == id);
        // A column with nothing placed is one run of terrain, floor to top,
        // whichever way it is made: the direct runs and the scan bake the
        // same mesh at every tier, and the scan's
        // exactness — a cube carving the terrain, a sub-cell object — is
        // only ever about what is placed, so the direct path serves every
        // tier: a zoom-in would otherwise scan every chunk in view, most of
        // them empty of objects.
        if !holding && !self.has_objects(world, c.col_idx) {
            return self.direct_runs(world, id, page);
        }
        if !holding {
            let col_idx = c.col_idx;
            if self.scan_job.is_some() || self.run_set_for(id).is_none() {
                return Progress::Blocked;
            }
            let Some(volume) = self.paint_terrain(world, id, page) else {
                self.refuse(world, id);
                return Progress::Blocked;
            };
            self.claimed.clear();
            let paint = PaintJob {
                next: 0,
                revision: column_revision(col_idx),
                hide: self.hide.filter(|h| h.0 == col_idx).map(|h| (h.1, h.2)),
                carve: self.carve.filter(|c| c.0 == col_idx).map(|c| c.1),
            };
            self.scan_job = Some(ScanJob { id, volume, paint: Some(paint) });
            return Progress::Busy;
        }
        let ScanJob { volume, paint, .. } = *self.scan_job.as_ref().unwrap();
        if let Some(paint) = paint {
            return self.advance_paint(world, id, volume, paint);
        }
        // SAFETY: the scan job's volume, held until the scan finishes.
        unsafe { world.volumes.band_views(volume, &mut self.bands) };
        // The apron's columns are the sample's, which paint_terrain read
        // before the volume was painted: the bands hold the chunk's own
        // cells and nothing beyond them.
        let apron = Apron {
            ele: &self.sample.ele,
            material: &self.sample.material,
            layer_idx: &self.sample.layer_idx,
            floor: self.sample.floor,
        };
        match self.scan.step(&self.bands, &apron) {
            ScanStep::Working => Progress::Busy,
            ScanStep::Done => unreachable!("a scan is finished when its set is provided"),
            ScanStep::NeedSet(bytes) => {
                let Some((set, at)) = self.set_of(id, bytes) else {
                    self.refuse(world, id);
                    return Progress::Blocked;
                };
                // SAFETY: the run set the chunk holds, past the bytes asked for.
                unsafe { self.scan.provide_set(world.volumes.mem().add(at), bytes) };
                let count = self.scan.finish();
                let job = self.scan_job.take().unwrap();
                world.volumes.release(job.volume);
                self.stat.scans += 1;
                self.keep_runs(id, Runs { set, bytes, count });
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
        let cells = (PAGE_CELLS * PAGE_CELLS) as usize;
        let samples = (ELE_PADDED_SPAN * ELE_PADDED_SPAN) as usize;
        PageView {
            cells: core::slice::from_raw_parts(mem.add(page.cells) as *const GroundCell, cells),
            ele: core::slice::from_raw_parts(mem.add(page.ele) as *const i16, samples),
        }
    }

    /// A new volume with the chunk's terrain painted in, open; none when a
    /// paint was refused.
    fn paint_terrain(&mut self, world: &mut World, id: u32, page: PageLanes) -> Option<u32> {
        let c = &self.chunks[&id];
        let (tx0, ty0, show) = (c.tx0, c.ty0, c.show_structures);
        self.paint_terrain_at(world, tx0, ty0, show, page)
    }

    fn paint_terrain_at(&mut self, world: &mut World, tx0: u32, ty0: u32, show: bool, page: PageLanes) -> Option<u32> {
        // SAFETY: a resident page's lanes, read within the command.
        let view = unsafe { Self::page_view(world, page) };
        self.sample.read(&view, world.materials, tx0, ty0, hidden_ground(show));
        let volume = world.volumes.open();
        if !self.sample.paint(world.volumes, volume) {
            world.volumes.release(volume);
            return None;
        }
        Some(volume)
    }

    /// A step of the scan job's paint of the chunk's objects. Once every
    /// entry and the carve are painted, the volume is sealed and the scan
    /// begins on it.
    fn advance_paint(&mut self, world: &mut World, id: u32, volume: u32, mut paint: PaintJob) -> Progress {
        let col_idx = self.chunks[&id].col_idx;
        if column_revision(col_idx) != paint.revision {
            self.abort_scan_of(world, id);
            return Progress::Busy;
        }
        let mut any = false;
        match world
            .painter
            .paint_entries(world.store.entries(col_idx), world.volumes, volume, paint.hide, paint.next, PAINT_VOXELS, &mut self.claimed, &mut any)
        {
            Painted::Refused => {
                self.refuse(world, id);
                return Progress::Blocked;
            }
            Painted::More(next) => {
                paint.next = next;
                self.scan_job.as_mut().unwrap().paint = Some(paint);
                return Progress::Busy;
            }
            Painted::Done => {}
        }
        if !world.painter.paint_carve(world.volumes, volume, paint.carve, &mut self.claimed, &mut any) {
            self.refuse(world, id);
            return Progress::Blocked;
        }
        world.volumes.seal(volume);
        // SAFETY: the volume was just sealed and is held by the scan job
        // until the scan finishes.
        unsafe { world.volumes.band_views(volume, &mut self.bands) };
        assert!(self.scan.begin(&self.bands), "a scan open with no job naming it");
        self.scan_job.as_mut().unwrap().paint = None;
        Progress::Busy
    }

    fn direct_runs(&mut self, world: &mut World, id: u32, page: PageLanes) -> Progress {
        if self.run_set_for(id).is_none() {
            return Progress::Blocked;
        }
        let c = &self.chunks[&id];
        let (tx0, ty0, show) = (c.tx0, c.ty0, c.show_structures);
        // SAFETY: the chunk's page, as its source named it, resident while
        // the chunk is.
        let view = unsafe { Self::page_view(world, page) };
        self.sample.read(&view, world.materials, tx0, ty0, hidden_ground(show));
        assert!(self.direct.begin_direct(), "the direct writer left open");
        let floor = self.sample.floor;
        // Every column of the set, the chunk's own and the apron's: the
        // sample holds both, and the writer wants them in one order.
        for i in 0..(COL_SPAN * COL_SPAN) as usize {
            self.direct.direct_column(i as u32);
            let top = self.sample.ele[i] as i32 - 1;
            if top >= floor {
                self.direct.direct_run(floor, top, self.sample.material[i], self.sample.layer_idx[i]);
            }
        }
        let bytes = self.direct.close_direct();
        let Some((set, at)) = self.set_of(id, bytes) else {
            self.direct.abort();
            self.refuse(world, id);
            return Progress::Blocked;
        };
        // SAFETY: as for a scan's set.
        unsafe { self.direct.provide_set(world.volumes.mem().add(at), bytes) };
        let count = self.direct.finish();
        self.stat.direct += 1;
        self.keep_runs(id, Runs { set, bytes, count });
        Progress::Busy
    }

    /// A level chunk's runs, written the direct way: its level page's ground
    /// and the object layer folded to its level (pxl-render mesh/paint/level.rs),
    /// in the level's cells. Cheap to make again, like a leaf's direct runs.
    fn level_runs(&mut self, world: &mut World, id: u32, page: PageLanes) -> Progress {
        if self.run_set_for(id).is_none() {
            return Progress::Blocked;
        }
        let c = &self.chunks[&id];
        let (tx0, ty0, show, level, base) = (c.tx0, c.ty0, c.show_structures, c.level, c.base);
        // SAFETY: the chunk's page, as its source named it, resident while
        // the chunk is.
        let view = unsafe { Self::page_view(world, page) };
        self.sample.read(&view, world.materials, tx0, ty0, hidden_ground(show));
        let target = GROUND.scale(level);
        let (x0, y0) = (ivec::X(base), ivec::Y(base));
        let mut columns = Vec::new();
        for i in 0..world.store.chunk_count() {
            let key = world.store.chunk_key(i);
            if !folds_into(level, base, key) {
                continue;
            }
            let o = FROM_L2_IDX(key);
            let (ox, oy) = (ivec::X(o), ivec::Y(o));
            if let Some(entries) = world.store.entries(key) {
                columns.push(Column { x0: ox, y0: oy, entries });
            }
        }
        let hide = self.hide.map(|(col, band, pos)| {
            let o = FROM_L2_IDX(col);
            (ivec::X(o), ivec::Y(o), band, pos)
        });
        fold_columns(world.painter, &columns, target, hide, &mut self.folded);
        assert!(self.direct.begin_direct(), "the direct writer left open");
        write_level_runs(
            &mut self.direct,
            &self.sample,
            world.painter,
            &self.folded,
            (target.index(x0), target.index(y0)),
            GROUND.scale(0).count_in(target),
        );
        let bytes = self.direct.close_direct();
        let Some((set, at)) = self.set_of(id, bytes) else {
            self.direct.abort();
            self.refuse(world, id);
            return Progress::Blocked;
        };
        // SAFETY: as for a scan's set.
        unsafe { self.direct.provide_set(world.volumes.mem().add(at), bytes) };
        let count = self.direct.finish();
        self.stat.direct += 1;
        self.keep_runs(id, Runs { set, bytes, count });
        Progress::Busy
    }

    /// The run set chunk `id` holds and where it lies, when `bytes` fit it.
    /// A set past it is refused, and the set given back.
    fn set_of(&mut self, id: u32, bytes: u32) -> Option<(usize, usize)> {
        let set = self.run_set_for(id)?;
        match run_set_at(set).filter(|_| bytes <= RUN_SET_BYTES) {
            Some(at) => Some((set, at as usize)),
            None => {
                self.let_runs_go(id);
                None
            }
        }
    }

    fn keep_runs(&mut self, id: u32, runs: Runs) {
        self.chunks.get_mut(&id).unwrap().runs = Some(runs);
    }

    fn run_set(world: &World, runs: &Runs) -> RunSet<'static> {
        let bytes = pxl_render::mesh::voxel_scan::run_set_bytes(runs.count, COL_SPAN) as usize;
        let at = run_set_at(runs.set).expect("runs made before the memory was laid out") as usize;
        // SAFETY: the chunk's own set, held until its runs are let go.
        unsafe { RunSet::carve(core::slice::from_raw_parts(world.volumes.mem().add(at), bytes), runs.count, COL_SPAN, APRON) }
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
            assert!(self.bake.begin(&set, step, 1 << c.level, settings_store().texture_pack), "a bake open with no job naming it");
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
                // The one bake open writes into `.fixed.bake`, which is past
                // any mesh a chunk has been measured to make (mesh_slot.rs).
                let Some(at) = bake_at().filter(|_| words * 4 <= BAKE_BYTES) else {
                    self.abort_bake();
                    self.refuse(world, id);
                    return Progress::Blocked;
                };
                // SAFETY: `.fixed.bake`, the open bake's alone until it is
                // finished and handed over, or aborted.
                unsafe { self.bake.provide_mesh(world.volumes.mem().add(at as usize) as *mut u32, words) };
                self.bake_job.as_mut().unwrap().mesh = Some((at as usize, words * 4));
                Progress::Busy
            }
            Step::Done => {
                let job = self.bake_job.as_ref().unwrap();
                // A mesh of nothing never asked for a block, and is handed
                // over as none: zero bytes at zero.
                debug_assert!(job.mesh.is_some() || mesh_words(0, FINE_BOX_WORDS) == 0);
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
                if c.wanted().next().is_none() {
                    self.let_runs_go(id);
                }
                emit(BuildEvent::Tier {
                    id,
                    generation: job.generation,
                    step: job.step,
                    at: at as u32,
                    bytes,
                    box_cnt: mesh.box_cnt,
                    heights: mesh.heights,
                });
                Progress::Busy
            }
        }
    }
}
