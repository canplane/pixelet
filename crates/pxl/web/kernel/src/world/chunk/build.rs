// pxl-kernel/world/chunk/build.rs

//! A chunk's build: the builder that makes its tiers from what it is made
//! of (source.rs).
//!
//! Every decision about a chunk, and everything it holds — its tiers, its
//! decor blocks, the sources they were made from — is the scheduler's
//! (crates/pxl/web/kernel/src/world/chunk/table.rs). What is here acts on
//! what it asks.

use pxl_spec::space::pyramid::{LevelPage, CHUNK_CELLS, GROUND};

use pxl_render::gpu::api::{gpu, room};
use pxl_render::gpu::renderer::device_ready;
use pxl_render::mesh::chunk_mesher::{box_words, mesh_words, BakeContext, Heights, Step, COARSE_BOX_WORDS, FINE_BOX_WORDS};
use pxl_render::mesh::paint::level::write_level_runs;
use pxl_render::mesh::paint::object::adapter::{Claims, Entries, Painted, Painter};
use pxl_render::mesh::paint::terrain::adapter::{beside_index, Around, ChunkSample, PageSource, PageView};
use pxl_render::mesh::volume::Volumes;
use pxl_render::mesh::voxel_scan::{Apron, BandView, RunSet, ScanContext, ScanStep, APRON, COL_SPAN};
use pxl_spec::bake::constant::{pack_pos, BandZ, EntryAt, BAND, CELL, OBJECT_FINEST};
use pxl_spec::space::indexer::{FROM_L2_IDX, L2, L2_IDX};
use pxl_spec::space::pvf3::ivec::{self, IVec};
use std::collections::HashMap;
use xpute_runtime::mem::base::ptr_at;
use xpute_runtime::mem::heap::{free, Block};

use crate::layer::object::store::{object_col_revision, Store};
use crate::mem::fixed::{at, Range};
use crate::shell::setting::store::{hidden_ground, settings_store};
use pxl_spec::bake::constant::BAND_CELLS;

use crate::world::chunk::mesh_slot::{capacity, COARSE_BOXES, FINE_BOXES, OCCUPANCY};
use crate::world::pyramid::chunk::Chunk as PyramidChunk;
use crate::world::pyramid::Layer as _;
use pxl_spec::bake::format::Terrain;
use pxl_spec::space::hierarchy::Scale;
use xpute_core::status::bug::OrBug;
use xpute_core::status::errno::Errno;

/// A tier: the cube a chunk is drawn in, its side as a scale of the unit.
/// The shared rule chooses it by the chunk's distance from the eye
/// (chunk/table.rs `wanted_cube`), and each layer draws its own detail at it:
/// terrain its level's cells, averaged to the cube where the cube is
/// coarser and as they are where it is finer, since there is nothing under
/// them; objects folded to it, down to their finest tier.
pub type Cube = Scale;

/// How many of a level's cells a cube spans along an axis: the mesher's
/// step, one where the cube is no coarser than the level's cell.
pub fn mesh_step(cube: Cube, level: u32) -> u32 {
    let own = GROUND.scale(level);
    own.count_in(cube.max(own))
}

/// The scale a chunk's objects are folded to for a cube: the cube, no finer
/// than an object's finest tier and no coarser than a cell, past which the
/// mesher's step averages them with the ground they stand on.
pub fn object_fold(cube: Cube) -> Scale {
    cube.clamp(OBJECT_FINEST, CELL)
}

/// Hands a block back to the heap. Every view into it is dead
/// afterward.
pub fn release_block(at: usize, bytes: u32) {
    // SAFETY: a block this chunk took and has not given back.
    unsafe { free((at, bytes)) };
}


// ---- the build as the kernel runs it ----
//
// A chunk's build, from its sources to its meshes, a chunk at a time under
// the frame's budget.
//
// The scheduler says which chunks exist, what each is made of (its terrain
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

/// The workspace: `.fixed.bake`, where the one bake open writes its mesh a
/// slot's worth at a time, each block handed to its slot before the next
/// is written (world/chunk/mesh_slot.rs). As large as any one slot, and no
/// larger, whatever the mesh. It keeps nothing across frames; what is kept
/// is the slots.
pub const BAKE_BYTES: u32 = {
    let (fine, coarse) = (FINE_BOXES * FINE_BOX_WORDS, COARSE_BOXES * COARSE_BOX_WORDS);
    4 * if fine > coarse { fine } else { coarse }
};

/// Run sets `.fixed.runs` holds.
pub const RUN_SETS: usize = 2;

/// What a build hands the scheduler.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum BuildEvent {
    /// Tier `cube` of chunk `id`, of its sources as of `generation`, is
    /// `boxes` boxes: the slots for all of it are asked for at once, before
    /// any is written. The answer is whether they are held for it; where
    /// not, the bake is thrown away, and what the tier needs is known the
    /// next time it is counted (world/chunk/table.rs `Slot::need`).
    Reserve { id: u32, generation: u32, cube: Cube, boxes: u32 },
    /// A block of that tier's boxes: `box_cnt` of them from the mesh's box
    /// `first`, at `at` in `.fixed.bake` until they are sent to their slot
    /// on the device, their heights held in `heights` (pxl-render
    /// mesh/chunk_mesher.rs). A slot's worth, but for the mesh's last.
    Part {
        id: u32,
        generation: u32,
        cube: Cube,
        at: u32,
        first: u32,
        box_cnt: u32,
        heights: Heights,
    },
    /// Tier `cube` of chunk `id`, built from its sources as of `generation`,
    /// all of its `box_cnt` boxes handed: it lands.
    Tier { id: u32, generation: u32, cube: Cube, box_cnt: u32, heights: Heights },
}

/// A chunk's runs: which of `.fixed.runs`'s sets, and what is in it — and
/// the scale its objects were folded to where it painted any
/// (`object_fold`), which only the cubes that fold to it are baked from.
/// A run set held: the column its runs are of, the chunks of that column
/// using it, and its runs once made. A column crossing a cube's floor or
/// ceiling is two chunks or more, each a cut of the one column's runs, so the
/// runs are the column's, made once, and every chunk over it bakes from them.
struct RunHold {
    column: (Scale, [i64; 2]),
    users: Vec<u32>,
    runs: Option<Runs>,
}

#[derive(Clone, Copy)]
struct Runs {
    set: usize,
    bytes: u32,
    count: u32,
    fold: Option<Scale>,
}

/// What a page's ground is read from: the page drawn in its place, the
/// page's first cell as its own cells from that page's corner, and how many
/// of them a side a source cell stands for — 1 on its own page, more on the
/// nearest page over it held while its own has not come (world/pyramid.rs
/// `drawn_from`).
#[derive(Clone, Copy)]
struct Source {
    page: &'static Terrain,
    origin: [i64; 2],
    n: u32,
}

impl Source {
    /// What `p` is drawn from now. A page is found by its key, so one gone,
    /// or whose record another page now holds, is none and never another
    /// page's bytes.
    fn of(p: LevelPage) -> Option<Source> {
        let pyramid = Terrain::pyramid();
        let src = pyramid.drawn_from(p)?;
        let page = pyramid.get(src)?;
        let corner = GROUND.cell_at(p, GROUND.cell_index(0, 0));
        Some(Source {
            page,
            origin: corner.in_page_above(src.level - p.level),
            n: GROUND.scale(p.level).count_in(GROUND.scale(src.level)),
        })
    }

    fn view(&self) -> PageSource<'static> {
        PageSource {
            page: Builder::page_view(self.page),
            origin: self.origin,
            n: self.n,
        }
    }
}

/// What a chunk's ground is read from: its own page's source, and the
/// sources of the pages beside it its apron reaches into, so the edge it
/// shares with a chunk there is read from what that chunk is drawn from —
/// both sides of it one surface, whichever of them is borrowed; and the
/// chunk's first cell in its page.
#[derive(Clone, Copy)]
struct Read {
    own: Source,
    beside: [Option<Source>; 8],
    first: [i64; 2],
}

impl Read {
    /// Whether it is the chunk's own page.
    fn own(&self) -> bool {
        self.own.n == 1
    }

    fn around(&self) -> Around<'static> {
        Around {
            own: self.own.view(),
            beside: self.beside.map(|b| b.as_ref().map(Source::view)),
        }
    }
}

#[derive(Clone, Copy)]
struct Chunk {
    /// The chunk being built: its level, corner, column and page all come
    /// from it.
    at: PyramidChunk,
    structures: bool,
    generation: u32,
    /// The tiers the node lacks, in the order it wants them.
    want: [Option<Cube>; 2],
    /// The heap's release count when it last refused this chunk a
    /// block: until a block is given back, the same request meets the
    /// same heap, and the chunk is not tried.
    refused: Option<u32>,
}

impl Chunk {
    /// The level of the page it is drawn from: its cells are 2^level units.
    fn level(&self) -> u32 {
        self.at.level()
    }

    /// The corner of the chunk's column, as an L1 coordinate at no height:
    /// what its objects are read by.
    fn base(&self) -> IVec {
        self.at.corner()
    }

    /// The column it stands in, as the object store keys it: a chunk is one
    /// L2 span.
    fn col_idx(&self) -> IVec {
        L2_IDX(self.base())
    }

    /// What the chunk is drawn from now: its own page, or the nearest
    /// resident page over it, and what the pages its apron reaches into are.
    fn read(&self) -> Option<Read> {
        let mut beside = [None; 8];
        for ([qx, qy], p) in self.at.pages_beside(APRON) {
            beside[beside_index(qx, qy)] = Source::of(p);
        }
        let (x0, y0) = self.at.in_page();
        Some(Read {
            own: Source::of(self.at.page())?,
            beside,
            first: [x0 as i64, y0 as i64],
        })
    }

    fn wants(&self, cube: Cube) -> bool {
        self.want.contains(&Some(cube))
    }

    fn wanted(&self) -> impl Iterator<Item = Cube> + '_ {
        self.want.iter().copied().flatten()
    }
}

#[derive(Clone, Copy)]
struct ScanJob {
    id: u32,
    volume: u32,
    /// The scale its objects are painted at (`object_fold`).
    fold: Scale,
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
    /// Whether it paints the builder's folded entries rather than the
    /// store's own: its fold is coarser than an object's finest tier.
    folded: bool,
    revision: u32,
    hide: Option<EntryAt>,
    carve: Option<EntryAt>,
}

/// The voxels a step of a paint sets before it stops: a band's cells, which
/// is also the most one entry sets, so a step sets at most twice this
/// whatever the column holds.
const PAINT_VOXELS: usize = (BAND_CELLS * BAND_CELLS) as usize;

struct BakeJob {
    id: u32,
    cube: Cube,
    generation: u32,
    mesh: Option<Block>,
    /// The mesh's boxes handed so far, block by block.
    handed: u32,
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
    pub store: &'a Store,
    pub painter: &'a mut Painter,
}

/// What the builder holds of each chunk mounted, by its mount (world/chunk/
/// table.rs `Scheduler::place`): room for every mount the table can give,
/// made once.
struct ByMount<T>(Vec<Option<T>>);

impl<T> ByMount<T> {
    fn new() -> ByMount<T> {
        ByMount((0..OCCUPANCY).map(|_| None).collect())
    }

    fn get(&self, id: &u32) -> Option<&T> {
        self.0.get(*id as usize)?.as_ref()
    }

    fn get_mut(&mut self, id: &u32) -> Option<&mut T> {
        self.0.get_mut(*id as usize)?.as_mut()
    }

    fn insert(&mut self, id: u32, t: T) {
        self.0[id as usize] = Some(t);
    }

    fn remove(&mut self, id: &u32) {
        self.0[*id as usize] = None;
    }
}

impl<T> core::ops::Index<&u32> for ByMount<T> {
    type Output = T;

    fn index(&self, id: &u32) -> &T {
        self.get(id).or_bug(Errno::ENOTRECOVERABLE)
    }
}

/// The build counters the profiler reads.
#[derive(Clone, Copy, Default)]
pub struct BuildStat {
    pub direct: u32,
    pub scans: u32,
    pub bakes: u32,
    pub starved: u32,
}

pub struct Builder {
    chunks: ByMount<Chunk>,
    scan: ScanContext,
    direct: ScanContext,
    bake: BakeContext,
    scan_job: Option<ScanJob>,
    bake_job: Option<BakeJob>,
    sample: ChunkSample,
    /// The drawn build's two edits: the hidden entry, and the carve
    /// preview where the store would file it.
    hide: Option<EntryAt>,
    carve: Option<EntryAt>,
    /// The chunk holding each run set, whether it is being filled or baked
    /// from.
    run_holds: [Option<RunHold>; RUN_SETS],
    /// The scan job's bands, kept between the scan's steps.
    bands: Vec<BandView<'static>>,
    /// The scan job's paint's claimed voxels, kept between its steps.
    claimed: Claims,
    /// Objects folded, a level chunk's or a z16 chunk's to its cube, as the
    /// store keeps them (layer/object/store.rs `Store::fold_at`), and those
    /// cells in order, kept for their capacity.
    folded: HashMap<[i64; 3], u32>,
    fold_cells: Vec<([i64; 3], u32)>,
    /// A z16 chunk's objects folded to its cube, as entries: what its paint
    /// reads where the cube is coarser than an object's finest tier.
    fold_band: Vec<BandZ>,
    fold_pos: Vec<u32>,
    fold_state: Vec<u32>,
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
            chunks: ByMount::new(),
            scan: ScanContext::new(),
            direct: ScanContext::new(),
            bake: BakeContext::new(),
            scan_job: None,
            bake_job: None,
            sample: ChunkSample::default(),
            hide: None,
            carve: None,
            run_holds: [const { None }; RUN_SETS],
            bands: Vec::new(),
            claimed: Claims::new(),
            folded: HashMap::new(),
            fold_cells: Vec::new(),
            fold_band: Vec::new(),
            fold_pos: Vec::new(),
            fold_state: Vec::new(),
            stat: BuildStat::default(),
        }
    }

    /// Where chunk `id` starts in its page, the page's lanes and the
    /// structure setting it is drawn under: the ground its decor is
    /// scattered over, at any level. None while its own page has not come:
    /// a chunk drawn from a page over it scatters nothing, since what it
    /// scattered there would move when its own page came.
    pub fn ground_of(&self, id: u32) -> Option<(u32, u32, &'static Terrain, bool)> {
        let c = self.chunks.get(&id)?;
        let read = c.read().filter(Read::own)?;
        let (tx0, ty0) = c.at.in_page();
        Some((tx0, ty0, read.own.page, c.structures))
    }

    pub fn generation_of(&self, id: u32) -> Option<u32> {
        self.chunks.get(&id).map(|c| c.generation)
    }

    /// The entry the drawn build leaves out.
    pub fn hide(&self) -> Option<EntryAt> {
        self.hide
    }

    /// The chunks holding the scan slot and the bake slot.
    pub fn holders(&self) -> [Option<u32>; 2] {
        [self.scan_job.as_ref().map(|j| j.id), self.bake_job.as_ref().map(|j| j.id)]
    }

    pub fn bake_context(&self) -> &BakeContext {
        &self.bake
    }

    // ---- commands ----

    pub fn add(&mut self, id: u32, at: PyramidChunk) {
        self.chunks.insert(
            id,
            Chunk {
                at,
                structures: false,
                generation: 0,
                want: [None; 2],
                refused: None,
            },
        );
    }

    pub fn remove(&mut self, world: &mut World, id: u32) {
        self.abort_scan_of(world, id);
        if matches!(&self.bake_job, Some(j) if j.id == id) {
            self.abort_bake();
        }
        self.leave_runs(id);
        self.chunks.remove(&id);
    }

    /// The chunk's sources changed: what it was built from is gone, and a
    /// tier built before now is stale.
    pub fn source(&mut self, world: &mut World, id: u32, structures: bool) {
        self.drop_work(world, id);
        if let Some(c) = self.chunks.get_mut(&id) {
            c.structures = structures;
            c.generation = c.generation.wrapping_add(1);
        }
    }

    /// The tiers the chunk still lacks. A bake of a tier not wanted
    /// gives the slot back now.
    /// The chunk the one bake open is for, and the cube and generation it
    /// bakes; none while no bake is open.
    pub fn baking(&self) -> Option<(u32, Cube, u32)> {
        self.bake_job.as_ref().map(|j| (j.id, j.cube, j.generation))
    }

    pub fn want(&mut self, world: &mut World, id: u32, want: [Option<Cube>; 2]) {
        let Some(c) = self.chunks.get_mut(&id) else { return };
        c.want = want;
        let stale = matches!(&self.bake_job, Some(j) if j.id == id && !c.wants(j.cube));
        if stale {
            self.abort_bake();
        }
        if want == [None, None] {
            self.abort_scan_of(world, id);
            self.leave_runs(id);
        }
    }

    pub fn set_edits(&mut self, hide: Option<EntryAt>, carve: Option<EntryAt>) {
        self.hide = hide;
        self.carve = carve;
    }

    /// The chunk's sources changed: its column's runs are stale for every
    /// chunk over it, which share them.
    fn drop_work(&mut self, world: &mut World, id: u32) {
        self.abort_scan_of(world, id);
        if matches!(&self.bake_job, Some(j) if j.id == id) {
            self.abort_bake();
        }
        self.drop_runs(id);
    }

    fn abort_scan_of(&mut self, world: &mut World, id: u32) {
        if let Some(job) = self.scan_job.take_if(|j| j.id == id) {
            self.scan.abort();
            world.volumes.release(job.volume);
            self.leave_runs(id);
        }
    }

    /// The run set chunk `id`'s column holds, if one does.
    fn hold_of(&self, id: u32) -> Option<usize> {
        let column = self.chunks.get(&id)?.at.column();
        self.run_holds.iter().position(|h| h.as_ref().is_some_and(|h| h.column == column))
    }

    /// The runs chunk `id`'s column holds, once made.
    fn runs_of(&self, id: u32) -> Option<Runs> {
        self.run_holds[self.hold_of(id)?].as_ref()?.runs
    }

    /// A run set for chunk `id`: its column's, joined, or a free one its
    /// column now holds. None while every set is another column's.
    fn run_set_for(&mut self, id: u32) -> Option<usize> {
        if let Some(set) = self.hold_of(id) {
            let users = &mut self.run_holds[set].as_mut()?.users;
            if !users.contains(&id) {
                users.push(id);
            }
            return Some(set);
        }
        let set = self.run_holds.iter().position(Option::is_none)?;
        let column = self.chunks.get(&id)?.at.column();
        self.run_holds[set] = Some(RunHold { column, users: vec![id], runs: None });
        Some(set)
    }

    /// Chunk `id` uses its column's runs no more; the set is given back with
    /// its last user.
    fn leave_runs(&mut self, id: u32) {
        for slot in self.run_holds.iter_mut() {
            if let Some(h) = slot {
                h.users.retain(|&u| u != id);
                if h.users.is_empty() {
                    *slot = None;
                }
            }
        }
    }

    /// Chunk `id`'s column's runs are stale: the set is given back for every
    /// chunk over the column, each of which makes them again, once.
    fn drop_runs(&mut self, id: u32) {
        if let Some(set) = self.hold_of(id) {
            self.run_holds[set] = None;
        }
        self.leave_runs(id);
    }

    /// The run sets lent now, and the bytes their runs fill.
    pub fn run_sets(&self) -> (u32, u32) {
        let held = self.run_holds.iter().flatten();
        let bytes = held.clone().filter_map(|h| h.runs.map(|r| r.bytes)).sum();
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
        self.leave_runs(id);
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
    /// A block refused ends the tick: the heap does not change until the
    /// scheduler reclaims after, so every later request would meet the same
    /// heap, and a refusal is not free — the direct runs are written whole
    /// before their block is asked for. Whether any chunk did work.
    pub fn tick(&mut self, world: &mut World, order: &[u32], budget_ms: f64, now: &impl Fn() -> f64, emit: &mut impl FnMut(BuildEvent) -> bool) -> bool {
        let t0 = now();
        let starved = self.stat.starved;
        let mut stepped = false;
        let mut worked = false;
        // The chunks holding the scan, the bake or a run set go first, each
        // once: held, a slot is every other chunk's wait.
        let mut first = [None; 2 + RUN_SETS];
        let users = self.run_holds.iter().flatten().flat_map(|h| h.users.first().copied());
        for (n, id) in self.holders().into_iter().chain(users.map(Some)).enumerate() {
            if !first[..n].contains(&id) {
                first[n] = id;
            }
        }
        let is_first = |id: u32| first.contains(&Some(id));
        for id in first.iter().flatten().copied().chain(order.iter().copied().filter(|&id| !is_first(id))) {
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
            }
        }
        worked
    }

    fn advance(&mut self, world: &mut World, id: u32, emit: &mut impl FnMut(BuildEvent) -> bool) -> Progress {
        let Some(c) = self.chunks.get(&id) else { return Progress::Done };
        if c.wanted().next().is_none() {
            return Progress::Done;
        }
        if c.refused.is_some_and(|r| r == crate::host::heap_stat().releases) {
            return Progress::Blocked;
        }
        // Runs whose objects were folded for another cube than the next one
        // wanted draw that cube's objects, not this one's: made again.
        let next = c.wanted().next().or_bug(Errno::ENOTRECOVERABLE);
        if self.runs_of(id).is_some_and(|r| r.fold.is_some_and(|f| f != object_fold(next))) && !matches!(&self.bake_job, Some(j) if j.id == id) {
            self.drop_runs(id);
        }
        if self.runs_of(id).is_none() {
            return self.advance_runs(world, id);
        }
        // Another chunk of the column made them: this one bakes its cut of
        // them too, and holds the set while it does.
        self.run_set_for(id);
        self.advance_bake(world, id, emit)
    }

    fn advance_runs(&mut self, world: &mut World, id: u32) -> Progress {
        let c = &self.chunks[&id];
        // A page gone under a scan: the scan is given up and the slot with
        // it, since held, it is every other chunk's wait, and the page may
        // not come back before the chunk goes.
        let Some(read) = c.read() else {
            self.abort_scan_of(world, id);
            return Progress::Blocked;
        };
        if c.level() > 0 {
            return self.level_runs(world, id, read);
        }
        let holding = matches!(&self.scan_job, Some(j) if j.id == id);
        // A column with nothing placed is one run of terrain, floor to top,
        // whichever way it is made: the direct runs and the scan bake the
        // same mesh at every tier, and the scan's
        // exactness — a cube carving the terrain, a sub-cell object — is
        // only ever about what is placed, so the direct path serves every
        // tier: a zoom-in would otherwise scan every chunk in view, most of
        // them empty of objects.
        if !holding && !self.has_objects(world, c.col_idx()) {
            return self.direct_runs(world, id, read);
        }
        if !holding {
            let col_idx = c.col_idx();
            if self.scan_job.is_some() || self.run_set_for(id).is_none() {
                return Progress::Blocked;
            }
            let Some(volume) = self.paint_terrain(world, id, read) else {
                self.refuse(world, id);
                return Progress::Blocked;
            };
            self.claimed.clear();
            let fold = object_fold(self.chunks[&id].wanted().next().or_bug(Errno::ENOTRECOVERABLE));
            let folded = fold > OBJECT_FINEST;
            if folded {
                self.fold_objects(world, col_idx, fold);
            }
            let paint = PaintJob {
                next: 0,
                folded,
                revision: object_col_revision(col_idx),
                hide: self.hide,
                carve: self.carve.filter(|c| c.col_idx == col_idx),
            };
            self.scan_job = Some(ScanJob { id, volume, fold, paint: Some(paint) });
            return Progress::Busy;
        }
        let ScanJob { volume, paint, fold, .. } = *self.scan_job.as_ref().unwrap();
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
            ScanStep::Done => xpute_core::bug!(ENOTRECOVERABLE),
            ScanStep::NeedSet(bytes) => {
                let Some((set, at)) = self.set_of(id, bytes) else {
                    self.refuse(world, id);
                    return Progress::Blocked;
                };
                // SAFETY: the run set the chunk holds, past the bytes asked for.
                unsafe { self.scan.provide_set(ptr_at::<u8>(at), bytes) };
                let count = self.scan.finish();
                let job = self.scan_job.take().unwrap();
                world.volumes.release(job.volume);
                self.stat.scans += 1;
                self.keep_runs(id, Runs { set, bytes, count, fold: Some(fold) });
                Progress::Busy
            }
        }
    }

    fn has_objects(&self, world: &World, col_idx: IVec) -> bool {
        world.store.entries(col_idx).is_some() || self.carve.is_some_and(|c| c.col_idx == col_idx)
    }

    /// A page's lanes as the paint reads them.
    pub fn page_view(page: &Terrain) -> PageView<'_> {
        PageView {
            cells: &page.ground.cells,
            ele: &page.ele,
        }
    }

    /// A new volume with the chunk's terrain painted in, open; none when a
    /// paint was refused.
    fn paint_terrain(&mut self, world: &mut World, id: u32, read: Read) -> Option<u32> {
        let show = self.chunks[&id].structures;
        self.sample.read(&read.around(), read.first, hidden_ground(show));
        let volume = world.volumes.open();
        if !self.sample.paint(world.volumes, volume) {
            world.volumes.release(volume);
            return None;
        }
        Some(volume)
    }

    /// A column's voxel entries folded to `scale` by the fold every level
    /// shares, as the store keeps it (layer/object/store.rs `fold_cell`), as
    /// entries of that scale in the builder's fold lanes: what a z16 chunk
    /// paints for a cube coarser than an object's finest tier. The hidden
    /// entry is left out, as the paint would leave it out.
    fn fold_objects(&mut self, world: &mut World, col_idx: IVec, scale: Scale) {
        let o = FROM_L2_IDX(col_idx);
        let (x0, y0) = (ivec::X(o), ivec::Y(o));
        let cells = scale.count_in(L2) as i64;
        let at = [ivec::X(col_idx) * cells, ivec::Y(col_idx) * cells];
        world.store.fold_at(scale, at, [at[0] + cells, at[1] + cells], self.hide, &mut self.folded);
        // A cell's corner in finest steps: absolute, then from the column's
        // corner and its band's floor.
        let n = OBJECT_FINEST.count_in(scale) as i64;
        let band_steps = OBJECT_FINEST.count_in(BAND) as i64;
        let (fx, fy) = (OBJECT_FINEST.index(x0), OBJECT_FINEST.index(y0));
        self.fold_cells.clear();
        self.fold_cells.extend(self.folded.iter().map(|(&c, &state)| (c, state)));
        self.fold_cells.sort_unstable();
        self.fold_band.clear();
        self.fold_pos.clear();
        self.fold_state.clear();
        for &([cx, cy, cz], state) in &self.fold_cells {
            let z = cz * n;
            self.fold_band.push(z.div_euclid(band_steps) as BandZ);
            self.fold_pos.push(pack_pos((cx * n - fx) as u32, (cy * n - fy) as u32, z.rem_euclid(band_steps) as u32, scale));
            self.fold_state.push(state);
        }
    }

    /// A step of the scan job's paint of the chunk's objects. Once every
    /// entry and the carve are painted, the volume is sealed and the scan
    /// begins on it.
    fn advance_paint(&mut self, world: &mut World, id: u32, volume: u32, mut paint: PaintJob) -> Progress {
        let col_idx = self.chunks[&id].col_idx();
        if object_col_revision(col_idx) != paint.revision {
            self.abort_scan_of(world, id);
            return Progress::Busy;
        }
        let mut any = false;
        let (entries, hide) = if paint.folded {
            let folded = Entries {
                col_idx,
                band: &self.fold_band,
                pos: &self.fold_pos,
                state: &self.fold_state,
            };
            (Some(folded), None)
        } else {
            (world.store.entries(col_idx), paint.hide)
        };
        match world.painter.paint_entries(entries, world.volumes, volume, hide, paint.next, PAINT_VOXELS, &mut self.claimed, &mut any) {
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
        xpute_core::ensure!(self.scan.begin(&self.bands), ENOTRECOVERABLE);
        self.scan_job.as_mut().unwrap().paint = None;
        Progress::Busy
    }

    fn direct_runs(&mut self, world: &mut World, id: u32, read: Read) -> Progress {
        if self.run_set_for(id).is_none() {
            return Progress::Blocked;
        }
        let show = self.chunks[&id].structures;
        self.sample.read(&read.around(), read.first, hidden_ground(show));
        xpute_core::ensure!(self.direct.begin_direct(), ENOTRECOVERABLE);
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
        unsafe { self.direct.provide_set(ptr_at::<u8>(at), bytes) };
        let count = self.direct.finish();
        self.stat.direct += 1;
        self.keep_runs(id, Runs { set, bytes, count, fold: None });
        Progress::Busy
    }

    /// A level chunk's runs, written the direct way: its level page's ground
    /// and the object layer folded to its level (pxl-render mesh/paint/level.rs),
    /// in the level's cells. Cheap to make again, like a leaf's direct runs.
    fn level_runs(&mut self, world: &mut World, id: u32, read: Read) -> Progress {
        if self.run_set_for(id).is_none() {
            return Progress::Blocked;
        }
        let c = &self.chunks[&id];
        let (show, level, base) = (c.structures, c.level(), c.base());
        self.sample.read(&read.around(), read.first, hidden_ground(show));
        let target = GROUND.scale(level);
        let (x0, y0) = (ivec::X(base), ivec::Y(base));
        // Its own cells and the apron of one a fold reads, which reaches into
        // the chunks beside it (world/chunk/source.rs `folds_into`).
        let (cx, cy, span) = (target.index(x0), target.index(y0), CHUNK_CELLS as i64);
        world.store.fold_at(target, [cx - 1, cy - 1], [cx + span + 1, cy + span + 1], self.hide, &mut self.folded);
        xpute_core::ensure!(self.direct.begin_direct(), ENOTRECOVERABLE);
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
        unsafe { self.direct.provide_set(ptr_at::<u8>(at), bytes) };
        let count = self.direct.finish();
        self.stat.direct += 1;
        self.keep_runs(id, Runs { set, bytes, count, fold: None });
        Progress::Busy
    }

    /// The run set chunk `id` holds and where it lies, when `bytes` fit it.
    /// A set past it is refused, and the set given back.
    fn set_of(&mut self, id: u32, bytes: u32) -> Option<(usize, usize)> {
        let set = self.run_set_for(id)?;
        match at(Range::Run(set), 0, bytes) {
            Some(at) => Some((set, at as usize)),
            None => {
                self.leave_runs(id);
                None
            }
        }
    }

    fn keep_runs(&mut self, id: u32, runs: Runs) {
        let set = self.hold_of(id).or_bug(Errno::ENOTRECOVERABLE);
        self.run_holds[set].as_mut().or_bug(Errno::ENOTRECOVERABLE).runs = Some(runs);
    }

    fn run_set(runs: &Runs) -> RunSet<'static> {
        let bytes = pxl_render::mesh::voxel_scan::run_set_bytes(runs.count, COL_SPAN) as usize;
        let set_at = at(Range::Run(runs.set), 0, bytes as u32).or_bug(Errno::ENOTRECOVERABLE) as usize;
        // SAFETY: the chunk's own set, held until its runs are let go.
        unsafe { RunSet::carve(core::slice::from_raw_parts(ptr_at::<u8>(set_at), bytes), runs.count, COL_SPAN, APRON) }
    }

    fn advance_bake(&mut self, world: &mut World, id: u32, emit: &mut impl FnMut(BuildEvent) -> bool) -> Progress {
        let holding = matches!(&self.bake_job, Some(j) if j.id == id);
        let c = &self.chunks[&id];
        let set = Self::run_set(&self.runs_of(id).or_bug(Errno::ENOTRECOVERABLE));
        if !holding {
            if self.bake_job.is_some() {
                return Progress::Blocked;
            }
            let cube = c.wanted().next().unwrap();
            // The runs are the column's, through every chunk over it; this
            // chunk draws what lies in its own heights (world/pyramid/
            // chunk.rs).
            let (z0, z1) = c.at.z_range();
            let clip = Some((z0 as i32, z1 as i32));
            xpute_core::ensure!(self.bake.begin(&set, mesh_step(cube, c.level()), 1 << c.level(), settings_store().textures, clip), ENOTRECOVERABLE);
            self.bake_job = Some(BakeJob {
                id,
                cube,
                generation: c.generation,
                mesh: None,
                handed: 0,
            });
        }
        match self.bake.step(&set) {
            Step::Working => Progress::Busy,
            Step::NeedMesh(words) => {
                // Every slot the mesh fills is held for it at once, or none
                // is and the bake is thrown away: a mesh is never cut, never
                // baked coarser than asked, and never waits holding the one
                // bake open (mesh_slot.rs).
                let job = self.bake_job.as_ref().unwrap();
                let step = mesh_step(job.cube, c.level());
                let (words_a_box, room_a_slot) = (box_words(step), capacity(c.level(), step));
                let boxes = words / words_a_box;
                let asked = BuildEvent::Reserve {
                    id,
                    generation: job.generation,
                    cube: job.cube,
                    boxes,
                };
                if !emit(asked) {
                    let job = self.bake_job.take().unwrap();
                    self.bake.abort();
                    let c = self.chunks.get_mut(&id).unwrap();
                    for w in c.want.iter_mut().filter(|w| **w == Some(job.cube)) {
                        *w = None;
                    }
                    if c.wanted().next().is_none() {
                        self.leave_runs(id);
                    }
                    return Progress::Busy;
                }
                // The one bake open writes a slot's worth at a time into
                // `.fixed.bake`, which holds any slot's (mesh_slot.rs).
                let block = boxes.min(room_a_slot) * words_a_box;
                let Some(mesh_at) = at(Range::Bake, 0, block * 4) else {
                    self.abort_bake();
                    self.refuse(world, id);
                    return Progress::Blocked;
                };
                // SAFETY: `.fixed.bake`, the open bake's alone until it is
                // finished and handed over, or aborted.
                unsafe { self.bake.provide_mesh(ptr_at::<u32>(mesh_at as usize), block) };
                self.bake_job.as_mut().unwrap().mesh = Some((mesh_at as usize, block * 4));
                Progress::Busy
            }
            Step::Full => {
                // A slot's worth written: handed over, and the block handed
                // back for the rest.
                let job = self.bake_job.as_ref().unwrap();
                let (at, bytes) = job.mesh.unwrap();
                if gpu().is_some() && device_ready() && !room(bytes) {
                    return Progress::Blocked;
                }
                let step = mesh_step(job.cube, c.level());
                let words_a_box = box_words(step);
                let n = bytes / 4 / words_a_box;
                emit(BuildEvent::Part {
                    id,
                    generation: job.generation,
                    cube: job.cube,
                    at: at as u32,
                    first: job.handed,
                    box_cnt: n,
                    heights: self.bake.heights(),
                });
                let job = self.bake_job.as_mut().unwrap();
                job.handed += n;
                let left = self.bake.left_words();
                let block = left.min(bytes / 4);
                // SAFETY: the same block of `.fixed.bake`, its boxes handed.
                unsafe { self.bake.provide_mesh(ptr_at::<u32>(at), block) };
                Progress::Busy
            }
            Step::Done => {
                let job = self.bake_job.as_ref().unwrap();
                // A mesh of nothing never asked for a block, and is handed
                // over as none: zero bytes at zero.
                debug_assert!(job.mesh.is_some() || mesh_words(0, FINE_BOX_WORDS) == 0);
                let (at, bytes) = job.mesh.unwrap_or((0, 0));
                // Handed over, the mesh goes to the device in this turn's
                // stream; with no room there it stays where it is, finished,
                // and is handed over from the next turn's. With no device
                // there is no stream to fill, and the hand-over goes on as
                // before one arrives.
                if gpu().is_some() && device_ready() && !room(bytes) {
                    return Progress::Blocked;
                }
                let mesh = self.bake.finish();
                let job = self.bake_job.take().unwrap();
                self.stat.bakes += 1;
                if mesh.box_cnt > job.handed {
                    emit(BuildEvent::Part {
                        id,
                        generation: job.generation,
                        cube: job.cube,
                        at: at as u32,
                        first: job.handed,
                        box_cnt: mesh.box_cnt - job.handed,
                        heights: mesh.heights,
                    });
                }
                let c = self.chunks.get_mut(&id).unwrap();
                for w in c.want.iter_mut() {
                    if *w == Some(job.cube) {
                        *w = None;
                    }
                }
                if c.wanted().next().is_none() {
                    self.leave_runs(id);
                }
                emit(BuildEvent::Tier {
                    id,
                    generation: job.generation,
                    cube: job.cube,
                    box_cnt: mesh.box_cnt,
                    heights: mesh.heights,
                });
                Progress::Busy
            }
        }
    }
}
