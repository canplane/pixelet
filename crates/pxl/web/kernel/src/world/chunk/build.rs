// pxl-kernel/world/chunk/build.rs

//! A chunk's build: the builder that makes its tiers from what it is made
//! of (source.rs).
//!
//! Every decision about a chunk, and everything it holds — its tiers and the
//! sources they were made from — is the scheduler's
//! (crates/pxl/web/kernel/src/world/chunk/table.rs). What is here acts on
//! what it asks.

use pxl_spec::procedural::Frame;
use pxl_spec::space::pyramid::{LevelPage, CHUNK_CELLS, GROUND};

use pxl_render::gpu::api::{gpu, room};
use pxl_render::gpu::renderer::device_ready;
use pxl_render::mesh::chunk_mesher::{box_words, mesh_words, BakeContext, Heights, Step, COARSE_BOX_WORDS, FINE_BOX_WORDS};
use pxl_render::mesh::paint::level::{write_level_runs, Stands};
use pxl_render::mesh::paint::object::adapter::{Claims, Entries, Painted, Painter};
use pxl_render::mesh::paint::terrain::adapter::{beside_index, Around, ChunkSample, PageSource, PageView};
use pxl_render::mesh::volume::Volumes;
use pxl_render::mesh::voxel_scan::{BandView, RunSet, ScanContext, ScanStep, APRON, COL_SPAN};
use pxl_spec::bake::constant::{pack_pos, BandZ, EntryAt, CELL, OBJECT_FINEST};
use pxl_spec::space::indexer::L2_IDX;
use pxl_spec::space::pvf3::ivec::{self, IVec};
use std::collections::{HashMap, TryReserveError};
use xpute_runtime::mem::base::ptr_at;
use xpute_runtime::mem::heap::{alloc, free, Block};

use crate::layer::object::store::{object_col_revision, Store};
use crate::mem::fixed::{at, Range};
use crate::shell::setting::store::settings_store;
use pxl_spec::bake::constant::BAND_CELLS;

use crate::world::chunk::mesh_slot::{capacity, COARSE_BOXES, FINE_BOXES, OCCUPANCY};
use crate::world::pyramid::chunk::Chunk as PyramidChunk;
use crate::world::pyramid::Layer as _;
use pxl_spec::bake::format::Terrain;
use pxl_spec::space::hierarchy::Scale;
use xpute_core::status::bug::OrBug;
use xpute_core::status::errno::Errno;

/// How many of a level's cells a scale spans along an axis (the mesher's step).
pub fn mesh_step(scale: Scale, level: u32) -> u32 {
    let own = GROUND.scale(level);
    own.count_in(scale.max(own))
}

/// Objects fold to the scale, clamped between their finest tier and a cell;
/// past a cell the mesher averages them with the ground.
pub fn object_fold(scale: Scale) -> Scale {
    scale.clamp(OBJECT_FINEST, CELL)
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
// A chunk's runs are a block of the heap, exactly as large as the runs its
// column holds, held from the moment they are made until every tier of the
// chunk is baked from them: the tiers are what lasts, on the device, so a
// set is not kept for a tier asked later but the chunk's tiers are all baked
// while it is there. Two sets at most, so that one is filled while the other
// is baked from; a chunk that finds both held waits.
//
// Not a range of `.fixed`: how many runs a column holds is what stands in
// it, every band of it up to the top of the world, and no size that would
// fit a range holds every column. A range too small for a column refused
// its chunk, which then drew nothing — the world wrong for want of a
// workspace. The heap asked for the size the scan has counted refuses only
// when the memory is out.

/// The workspace: `.fixed.bake`, where the one bake open writes its mesh a
/// slot's worth at a time, each block handed to its slot before the next
/// is written (world/chunk/mesh_slot.rs). As large as any one slot, and no
/// larger, whatever the mesh. It keeps nothing across frames; what is kept
/// is the slots.
pub const BAKE_BYTES: u32 = {
    let (fine, coarse) = (FINE_BOXES * FINE_BOX_WORDS, COARSE_BOXES * COARSE_BOX_WORDS);
    4 * if fine > coarse { fine } else { coarse }
};

/// Run sets held at once: one filled while the other is baked from.
pub const RUN_SETS: usize = 2;

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum BuildEvent {
    /// All slots for a tier are asked for at once, before any is written. If
    /// refused, the bake is thrown away and the need recorded (`Slot::need`).
    Reserve {
        id: u32,
        generation: u32,
        scale: Scale,
        boxes: u32,
    },
    /// One slot's worth of boxes (less for the last) at `at` in `.fixed.bake`.
    Part {
        id: u32,
        generation: u32,
        scale: Scale,
        at: u32,
        first: u32,
        box_cnt: u32,
        heights: Heights,
    },
    Tier {
        id: u32,
        generation: u32,
        scale: Scale,
        box_cnt: u32,
        heights: Heights,
    },
}

/// A column split across scale boundaries is several chunks; its runs are
/// made once and each chunk bakes its cut of them.
struct RunHold {
    column: (Scale, [i64; 2]),
    users: Vec<u32>,
    runs: Option<Runs>,
}

impl Drop for RunHold {
    /// The runs' block goes back to the heap with the hold.
    fn drop(&mut self) {
        if let Some(r) = self.runs.take() {
            // SAFETY: the block the hold took for its runs; every view of it
            // is a bake from this hold, which ends before the hold does.
            unsafe { free(r.block) };
        }
    }
}

/// A chunk's runs: the heap block they are packed in, the bytes of it they
/// fill, how many — and the scale its objects were folded to where it
/// painted any (`object_fold`), which only the cubes that fold to it are
/// baked from.
#[derive(Clone, Copy)]
struct Runs {
    block: (usize, u32),
    bytes: u32,
    count: u32,
    fold: Option<Scale>,
}

/// Where a page's ground is read from: itself (`n` = 1) or the nearest
/// resident page over it (`drawn_from`).
#[derive(Clone, Copy)]
struct Source {
    page: &'static Terrain,
    frame: Frame,
    origin: [i64; 2],
    n: u32,
}

impl Source {
    /// Found by key, so a page that is gone never yields another page's bytes.
    fn of(p: LevelPage) -> Option<Source> {
        let pyramid = Terrain::pyramid();
        let src = pyramid.paging().drawn_from(p)?;
        let page = pyramid.paging().get(src)?;
        let corner = GROUND.cell_at(p, GROUND.cell_index(0, 0));
        Some(Source {
            page,
            frame: Frame::of(src),
            origin: corner.in_page_above(src.level - p.level),
            n: GROUND.scale(p.level).count_in(GROUND.scale(src.level)),
        })
    }

    fn view(&self) -> PageSource<'static> {
        PageSource {
            page: Builder::page_view(self.page),
            frame: self.frame,
            origin: self.origin,
            n: self.n,
        }
    }
}

/// The apron reads each neighbour's page from what that neighbour is drawn
/// from, so a shared edge is one surface.
#[derive(Clone, Copy)]
struct Read {
    own: Source,
    beside: [Option<Source>; 8],
    first: [i64; 2],
}

impl Read {
    fn around(&self) -> Around<'static> {
        Around {
            own: self.own.view(),
            beside: self.beside.map(|b| b.as_ref().map(Source::view)),
        }
    }
}

#[derive(Clone, Copy)]
struct Chunk {
    at: PyramidChunk,
    generation: u32,
    want: [Option<Scale>; 2],
    /// Heap release count at the last refusal: until something is freed the same
    /// request would fail, so the chunk is skipped.
    refused: Option<u32>,
}

impl Chunk {
    fn level(&self) -> u32 {
        self.at.level()
    }

    fn base(&self) -> IVec {
        self.at.corner()
    }

    fn col_idx(&self) -> IVec {
        L2_IDX(self.base())
    }

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

    fn wants(&self, scale: Scale) -> bool {
        self.want.contains(&Some(scale))
    }

    fn wanted(&self) -> impl Iterator<Item = Scale> + '_ {
        self.want.iter().copied().flatten()
    }
}

#[derive(Clone, Copy)]
struct ScanJob {
    id: u32,
    volume: u32,
    fold: Scale,
    paint: Option<PaintJob>,
}

/// A step-wise object paint. A store write between steps moves the entries,
/// so the paint restarts when the column revision changes.
#[derive(Clone, Copy)]
struct PaintJob {
    next: usize,
    folded: bool,
    revision: u32,
    hide: Option<EntryAt>,
    carve: Option<EntryAt>,
}

/// A band's cells, also the most one entry sets, so a step sets at most twice this.
const PAINT_VOXELS: usize = (BAND_CELLS * BAND_CELLS) as usize;

struct BakeJob {
    id: u32,
    scale: Scale,
    generation: u32,
    mesh: Option<Block>,
    handed: u32,
}

#[derive(PartialEq, Eq)]
enum Progress {
    Busy,
    Done,
    /// Waiting on another chunk's slot, a refused heap block, or a page.
    Blocked,
}

pub struct World<'a> {
    pub volumes: &'a mut Volumes,
    pub store: &'a Store,
    pub painter: &'a mut Painter,
}

/// Per-mount state, sized once for every mount the table can give.
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
    hide: Option<EntryAt>,
    carve: Option<EntryAt>,
    /// The chunk holding each run set, whether it is being filled or baked
    /// from.
    run_holds: [Option<RunHold>; RUN_SETS],
    /// The scan job's bands, kept between the scan's steps.
    bands: Vec<BandView<'static>>,
    claimed: Claims,
    /// Objects folded to a level chunk's cells, as the store keeps them
    /// (layer/object/store.rs `Store::fold_at`), kept for its capacity.
    folded: HashMap<[i64; 3], u32>,
    /// What stands in each column of a level chunk, by column and height
    /// (pxl-render mesh/paint/level.rs `write_level_runs`).
    stands: Vec<(u32, Stands)>,
    /// A z16 chunk's objects folded to its scale, for scales coarser than an object's finest tier.
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
            stands: Vec::new(),
            fold_band: Vec::new(),
            fold_pos: Vec::new(),
            fold_state: Vec::new(),
            stat: BuildStat::default(),
        }
    }


    pub fn generation_of(&self, id: u32) -> Option<u32> {
        self.chunks.get(&id).map(|c| c.generation)
    }


    /// The chunks holding the scan slot and the bake slot.
    pub fn holders(&self) -> [Option<u32>; 2] {
        [self.scan_job.as_ref().map(|j| j.id), self.bake_job.as_ref().map(|j| j.id)]
    }

    pub fn bake_context(&self) -> &BakeContext {
        &self.bake
    }

    pub fn add(&mut self, id: u32, at: PyramidChunk) {
        self.chunks.insert(
            id,
            Chunk {
                at,
                generation: 0,
                want: [None; 2],
                refused: None,
            },
        );
    }

    pub fn remove(&mut self, world: &mut World, id: u32) {
        self.abort_work_of(world, id);
        self.leave_runs(id);
        self.chunks.remove(&id);
    }

    pub fn source(&mut self, world: &mut World, id: u32) {
        self.drop_work(world, id);
        if let Some(c) = self.chunks.get_mut(&id) {
            c.generation = c.generation.wrapping_add(1);
        }
    }

    pub fn baking(&self) -> Option<(u32, Scale, u32)> {
        self.bake_job.as_ref().map(|j| (j.id, j.scale, j.generation))
    }

    /// A bake of a tier no longer wanted gives its slot back now.
    pub fn want(&mut self, world: &mut World, id: u32, want: [Option<Scale>; 2]) {
        let Some(c) = self.chunks.get_mut(&id) else { return };
        c.want = want;
        let stale = matches!(&self.bake_job, Some(j) if j.id == id && !c.wants(j.scale));
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
        self.abort_work_of(world, id);
        self.drop_runs(id);
    }

    /// Aborts the scan and the bake chunk `id` holds, if it holds either.
    fn abort_work_of(&mut self, world: &mut World, id: u32) {
        self.abort_scan_of(world, id);
        if matches!(&self.bake_job, Some(j) if j.id == id) {
            self.abort_bake();
        }
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

    pub fn run_sets(&self) -> (u32, u32) {
        let held = self.run_holds.iter().flatten();
        let bytes = held.clone().filter_map(|h| h.runs.map(|r| r.bytes)).sum();
        (held.count() as u32, bytes)
    }

    /// Heap refused a block: give up every slot held, since each is another
    /// chunk's wait, and skip the chunk until a release.
    fn refuse(&mut self, world: &mut World, id: u32) {
        self.stat.starved += 1;
        self.abort_work_of(world, id);
        self.leave_runs(id);
        let releases = crate::host::heap_stat().releases;
        if let Some(c) = self.chunks.get_mut(&id) {
            c.refused = Some(releases);
        }
    }

    fn unwant(&mut self, id: u32, scale: Scale) {
        let Some(c) = self.chunks.get_mut(&id) else { return };
        for w in c.want.iter_mut().filter(|w| **w == Some(scale)) {
            *w = None;
        }
        if c.wanted().next().is_none() {
            self.leave_runs(id);
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
    /// A block refused ends the tick: the heap does not change within it, so
    /// every later request would meet the same heap, and a refusal is not free — the direct runs are written whole
    /// before their block is asked for. Whether any chunk did work.
    pub fn tick(&mut self, world: &mut World, order: &[u32], budget_ms: f64, now: &impl Fn() -> f64, emit: &mut impl FnMut(BuildEvent) -> bool) -> bool {
        let t0 = now();
        let starved = self.stat.starved;
        let mut stepped = false;
        let mut worked = false;
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
        // Runs folded for another scale draw the wrong objects: remake them.
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
        // Page gone under a scan: give up the scan slot, as the page may not return
        // before the chunk goes.
        let Some(read) = c.read() else {
            self.abort_scan_of(world, id);
            return Progress::Blocked;
        };
        if c.level() > 0 {
            return self.level_runs(world, id, read);
        }
        let holding = matches!(&self.scan_job, Some(j) if j.id == id);
        // A column with nothing placed is one terrain run per column, the same mesh
        // the scan would give at every tier; scanning would make a zoom-in scan
        // every mostly-empty chunk in view.
        if !holding && !self.has_objects(world, c.col_idx()) {
            return self.direct_runs(world, id, read);
        }
        if !holding {
            let col_idx = c.col_idx();
            if self.scan_job.is_some() || self.run_set_for(id).is_none() {
                return Progress::Blocked;
            }
            let Some(volume) = self.paint_terrain(world, read) else {
                self.refuse(world, id);
                return Progress::Blocked;
            };
            self.claimed.clear();
            let fold = object_fold(self.chunks[&id].wanted().next().or_bug(Errno::ENOTRECOVERABLE));
            let folded = fold > OBJECT_FINEST;
            if folded && self.fold_objects(world, col_idx, fold).is_err() {
                world.volumes.release(volume);
                self.refuse(world, id);
                return Progress::Blocked;
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
        if !unsafe { world.volumes.band_views(volume, &mut self.bands) } {
            self.refuse(world, id);
            return Progress::Blocked;
        }
        // The apron comes from the sample; the bands hold only the chunk's own cells.
        match self.scan.step(&self.bands, &self.sample) {
            ScanStep::Working => Progress::Busy,
            ScanStep::Done => xpute_core::bug!(ENOTRECOVERABLE),
            ScanStep::Refused => {
                self.refuse(world, id);
                Progress::Blocked
            }
            ScanStep::NeedSet(bytes) => {
                let Some(block) = self.set_of(id, bytes) else {
                    self.refuse(world, id);
                    return Progress::Blocked;
                };
                // SAFETY: the block just taken for the chunk's runs, the bytes asked for.
                unsafe { self.scan.provide_set(ptr_at::<u8>(block.0), bytes) };
                let count = self.scan.finish();
                let job = self.scan_job.take().unwrap();
                world.volumes.release(job.volume);
                self.stat.scans += 1;
                self.keep_runs(id, Runs { block, bytes, count, fold: Some(fold) });
                Progress::Busy
            }
        }
    }

    fn has_objects(&self, world: &World, col_idx: IVec) -> bool {
        world.store.entries(col_idx).is_some() || self.carve.is_some_and(|c| c.col_idx == col_idx)
    }

    pub fn page_view(page: &Terrain) -> PageView<'_> {
        PageView {
            cells: &page.ground.cells,
            ele: &page.ele,
        }
    }

    fn paint_terrain(&mut self, world: &mut World, read: Read) -> Option<u32> {
        self.sample.read(&read.around(), read.first).ok()?;
        let volume = world.volumes.open();
        if !self.sample.paint(world.volumes, volume) {
            world.volumes.release(volume);
            return None;
        }
        Some(volume)
    }

    /// A column's voxel entries as a paint at `scale` takes them, in the
    /// builder's fold lanes: those finer than `scale` folded to it, the rest
    /// at their own size (layer/object/store.rs `Store::fold_entries`) —
    /// what a z16 chunk paints for a cube coarser than an object's finest
    /// tier. The hidden entry is left out, as the paint would leave it out.
    /// An error where the heap refused the fold room.
    fn fold_objects(&mut self, world: &mut World, col_idx: IVec, scale: Scale) -> Result<(), TryReserveError> {
        let (band, pos, state) = (&mut self.fold_band, &mut self.fold_pos, &mut self.fold_state);
        band.clear();
        pos.clear();
        state.clear();
        let mut refused = Ok(());
        world.store.fold_entries(col_idx, scale, self.hide, &mut |b, [x, y, z], s, v| {
            if refused.is_ok() {
                refused = (|| {
                    band.try_reserve(1)?;
                    pos.try_reserve(1)?;
                    state.try_reserve(1)?;
                    band.push(b);
                    pos.push(pack_pos(x, y, z, s));
                    state.push(v);
                    Ok(())
                })();
            }
        });
        refused
    }

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
        // SAFETY: the volume, once sealed, is held by the scan job until the
        // scan finishes.
        if !world.volumes.seal(volume) || !unsafe { world.volumes.band_views(volume, &mut self.bands) } {
            self.refuse(world, id);
            return Progress::Blocked;
        }
        xpute_core::ensure!(self.scan.begin(&self.bands), ENOTRECOVERABLE);
        self.scan_job.as_mut().unwrap().paint = None;
        Progress::Busy
    }

    fn direct_runs(&mut self, world: &mut World, id: u32, read: Read) -> Progress {
        if self.run_set_for(id).is_none() {
            return Progress::Blocked;
        }
        if self.sample.read(&read.around(), read.first).is_err() {
            self.refuse(world, id);
            return Progress::Blocked;
        }
        xpute_core::ensure!(self.direct.begin_direct(), ENOTRECOVERABLE);
        // The chunk's columns and the apron's, in the writer's one order.
        for i in 0..(COL_SPAN * COL_SPAN) as usize {
            self.direct.direct_column(i as u32);
            let direct = &mut self.direct;
            if self.sample.column_runs(i, |lo, hi, material, layer| direct.direct_run(lo, hi, material, layer)).is_err() {
                self.direct.abort();
                self.refuse(world, id);
                return Progress::Blocked;
            }
        }
        self.close_direct(world, id)
    }

    /// A level chunk's runs, written directly from its page's ground and the
    /// objects folded to its level (mesh/paint/level.rs).
    fn level_runs(&mut self, world: &mut World, id: u32, read: Read) -> Progress {
        if self.run_set_for(id).is_none() {
            return Progress::Blocked;
        }
        let c = &self.chunks[&id];
        let (level, base) = (c.level(), c.base());
        if self.sample.read(&read.around(), read.first).is_err() {
            self.refuse(world, id);
            return Progress::Blocked;
        }
        let target = GROUND.scale(level);
        let (x0, y0) = (ivec::X(base), ivec::Y(base));
        // One cell of apron, which a fold reads (`folds_into`).
        let (cx, cy, span) = (target.index(x0), target.index(y0), CHUNK_CELLS as i64);
        if world.store.fold_at(target, [cx - 1, cy - 1], [cx + span + 1, cy + span + 1], self.hide, &mut self.folded).is_err() {
            self.refuse(world, id);
            return Progress::Blocked;
        }
        xpute_core::ensure!(self.direct.begin_direct(), ENOTRECOVERABLE);
        let written = write_level_runs(
            &mut self.direct,
            &self.sample,
            world.painter,
            &self.folded,
            (target.index(x0), target.index(y0)),
            GROUND.scale(0).count_in(target),
            &mut self.stands,
        );
        self.close_direct(world, id)
    }

    /// Closes the direct runs written for chunk `id` into its run set: kept
    /// as its runs, or the chunk refused where the heap has no room.
    fn close_direct(&mut self, world: &mut World, id: u32) -> Progress {
        let bytes = self.direct.close_direct();
        let Some(block) = self.set_of(id, bytes) else {
            self.direct.abort();
            self.refuse(world, id);
            return Progress::Blocked;
        };
        // SAFETY: as for a scan's set.
        unsafe { self.direct.provide_set(ptr_at::<u8>(block.0), bytes) };
        let count = self.direct.finish();
        self.stat.direct += 1;
        self.keep_runs(id, Runs { block, bytes, count, fold: None });
        Progress::Busy
    }

    /// A heap block of `bytes` for chunk `id`'s runs, in the run set its
    /// column holds; none, and the set given back, where every set is
    /// another column's or the heap refuses.
    fn set_of(&mut self, id: u32, bytes: u32) -> Option<(usize, u32)> {
        self.run_set_for(id)?;
        let block = alloc(bytes);
        if block.is_none() {
            self.leave_runs(id);
        }
        block
    }

    fn keep_runs(&mut self, id: u32, runs: Runs) {
        let set = self.hold_of(id).or_bug(Errno::ENOTRECOVERABLE);
        let hold = self.run_holds[set].as_mut().or_bug(Errno::ENOTRECOVERABLE);
        if let Some(old) = hold.runs.replace(runs) {
            // SAFETY: the block the hold's earlier runs were in, which nothing
            // bakes from now that the new runs stand in their place.
            unsafe { free(old.block) };
        }
    }

    fn run_set(runs: &Runs) -> RunSet<'static> {
        let bytes = pxl_render::mesh::voxel_scan::run_set_bytes(runs.count, COL_SPAN) as usize;
        xpute_core::ensure!(bytes as u32 <= runs.block.1, ENOTRECOVERABLE);
        // SAFETY: the chunk's own block, held until its runs are let go.
        unsafe { RunSet::carve(core::slice::from_raw_parts(ptr_at::<u8>(runs.block.0), bytes), runs.count, COL_SPAN, APRON) }
    }

    fn advance_bake(&mut self, world: &mut World, id: u32, emit: &mut impl FnMut(BuildEvent) -> bool) -> Progress {
        let holding = matches!(&self.bake_job, Some(j) if j.id == id);
        let c = &self.chunks[&id];
        let set = Self::run_set(&self.runs_of(id).or_bug(Errno::ENOTRECOVERABLE));
        if !holding {
            if self.bake_job.is_some() {
                return Progress::Blocked;
            }
            let scale = c.wanted().next().unwrap();
            // Runs span the whole column; this chunk bakes only its own heights.
            let (z0, z1) = c.at.z_range();
            let clip = Some((z0 as i32, z1 as i32));
            if self.bake.begin(&set, mesh_step(scale, c.level()), GROUND.finest_in(c.level()), settings_store().textures, clip).is_err() {
                self.refuse(world, id);
                return Progress::Blocked;
            }
            self.bake_job = Some(BakeJob {
                id,
                scale,
                generation: c.generation,
                mesh: None,
                handed: 0,
            });
        }
        match self.bake.step(&set) {
            Step::Working => Progress::Busy,
            Step::Refused => {
                self.bake_job = None;
                self.refuse(world, id);
                Progress::Blocked
            }
            Step::NeedMesh(words) => {
                // All slots are held at once or the bake is thrown away: a mesh is never
                // cut, never baked coarser, and never waits while holding the one bake.
                let job = self.bake_job.as_ref().unwrap();
                let step = mesh_step(job.scale, c.level());
                let (words_a_box, room_a_slot) = (box_words(step), capacity(c.level(), step));
                let boxes = words / words_a_box;
                let asked = BuildEvent::Reserve {
                    id,
                    generation: job.generation,
                    scale: job.scale,
                    boxes,
                };
                if !emit(asked) {
                    let job = self.bake_job.take().unwrap();
                    self.bake.abort();
                    self.unwant(id, job.scale);
                    return Progress::Busy;
                }
                let block = boxes.min(room_a_slot) * words_a_box;
                let Some(mesh_at) = at(Range::Bake, 0, block * 4) else {
                    self.abort_bake();
                    self.refuse(world, id);
                    return Progress::Blocked;
                };
                // SAFETY: `.fixed.bake` is the open bake's alone until finished or aborted.
                unsafe { self.bake.provide_mesh(ptr_at::<u32>(mesh_at as usize), block) };
                self.bake_job.as_mut().unwrap().mesh = Some((mesh_at as usize, block * 4));
                Progress::Busy
            }
            Step::Full => {
                let job = self.bake_job.as_ref().unwrap();
                let (at, bytes) = job.mesh.unwrap();
                if gpu().is_some() && device_ready() && !room(bytes) {
                    return Progress::Blocked;
                }
                let step = mesh_step(job.scale, c.level());
                let words_a_box = box_words(step);
                let n = bytes / 4 / words_a_box;
                emit(BuildEvent::Part {
                    id,
                    generation: job.generation,
                    scale: job.scale,
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
                // An empty mesh asked for no block and is handed over as zero bytes.
                debug_assert!(job.mesh.is_some() || mesh_words(0, FINE_BOX_WORDS) == 0);
                let (at, bytes) = job.mesh.unwrap_or((0, 0));
                // No room in this turn's upload stream: wait for the next turn. With no
                // device there is no stream, and the hand-over goes on.
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
                        scale: job.scale,
                        at: at as u32,
                        first: job.handed,
                        box_cnt: mesh.box_cnt - job.handed,
                        heights: mesh.heights,
                    });
                }
                self.unwant(id, job.scale);
                emit(BuildEvent::Tier {
                    id,
                    generation: job.generation,
                    scale: job.scale,
                    box_cnt: mesh.box_cnt,
                    heights: mesh.heights,
                });
                Progress::Busy
            }
        }
    }
}
