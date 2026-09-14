//! A chunk's build, from its sources to its meshes: what
//! module/world/chunk_node.ts's commit did a chunk at a time under the
//! frame's budget, in the core.
//!
//! The glue says which chunks exist, what each is made of (its terrain
//! page's lanes and the settings that change what they paint), and which
//! tiers it still lacks; each tick it hands over the order to take them in
//! and a budget. A chunk's runs come the direct way — one run a column
//! from the terrain, when no object stands in its column — or by painting
//! the terrain and the objects into a volume and scanning it. Its tiers are baked from those runs, one bake
//! and one scan open in the whole world at a time, as before. What comes
//! out is events: a tier's mesh in a block of the mesh section, handed over
//! whole, and what the chunk's runs hold, for the retain budget.
//!
//! A block the pool refuses leaves the work where it is until a later tick;
//! nothing is copied to a place the core cannot see.

use std::collections::BTreeMap;

use crate::buddy::Pool;
use crate::mesher::{mesh_words, BakeContext, Step};
use crate::object::{Carve, Painter, Store};
use crate::run_set::RunSet;
use crate::scan::{BandView, ScanContext, ScanStep};
use crate::terrain::{ChunkSample, Materials, PageView, CHANNELS, CHUNK_SPAN, ELE_PADDED_SPAN, PAGE_CELL_SPAN};
use crate::volume::Volumes;

/// What the glue is told.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Event {
    /// Tier `step` of chunk `id`, built from its sources as of `generation`:
    /// the mesh is `bytes` bytes at `at`, the glue's to release.
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

struct Runs {
    at: usize,
    bytes: u32,
    count: u32,
    /// Made the direct way, which is cheap to make again.
    direct: bool,
}

struct Chunk {
    /// The column's base, the key the object store files it under.
    base: u64,
    /// Where the chunk starts in its page, in cells.
    tx0: u32,
    ty0: u32,
    page: Option<PageLanes>,
    show_structures: bool,
    generation: u32,
    /// The tiers the glue lacks, in the order it wants them; 0 is none.
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
    mesh: Option<(usize, u32)>,
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
    pub pools: &'a mut [Pool],
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
    hide: Option<(u64, i32, u32)>,
    carve: Option<(u64, Carve)>,
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
            stat: BuildStat::default(),
        }
    }

    /// What chunk `id` is made of, for a decor build: its column's base, where
    /// it starts in its page, and its page's lanes.
    pub fn source_of(&self, id: u32) -> Option<(u64, u32, u32, PageLanes, bool)> {
        let c = self.chunks.get(&id)?;
        Some((c.base, c.tx0, c.ty0, c.page?, c.show_structures))
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
    pub fn hide_in(&self, key: u64) -> Option<(i32, u32)> {
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

    pub fn add(&mut self, id: u32, base: u64, tx0: u32, ty0: u32) {
        assert!(tx0 + CHUNK_SPAN <= PAGE_CELL_SPAN && ty0 + CHUNK_SPAN <= PAGE_CELL_SPAN, "a chunk past its page");
        self.chunks.insert(
            id,
            Chunk {
                base,
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

    pub fn remove(&mut self, world: &mut World, id: u32, emit: &mut impl FnMut(Event)) {
        self.drop_work(world, id, emit);
        self.chunks.remove(&id);
    }

    /// The chunk's sources changed: what it was built from is gone, and a
    /// tier built before now is stale.
    pub fn source(&mut self, world: &mut World, id: u32, page: Option<PageLanes>, show_structures: bool, emit: &mut impl FnMut(Event)) {
        self.drop_work(world, id, emit);
        if let Some(c) = self.chunks.get_mut(&id) {
            c.page = page;
            c.show_structures = show_structures;
            c.generation = c.generation.wrapping_add(1);
        }
    }

    /// Gives back the chunk's runs and its work in flight, keeping its
    /// sources: the retain budget's reclaim.
    pub fn reclaim(&mut self, world: &mut World, id: u32, emit: &mut impl FnMut(Event)) {
        self.drop_work(world, id, emit);
    }

    /// The tiers the chunk still lacks. A bake of a tier no longer wanted
    /// gives the slot back now.
    pub fn want(&mut self, world: &mut World, id: u32, want: [u32; 2]) {
        let Some(c) = self.chunks.get_mut(&id) else { return };
        c.want = want;
        let stale = matches!(&self.bake_job, Some(j) if j.id == id && !c.wants(j.step));
        if stale {
            self.abort_bake(world);
        }
        if want == [0, 0] {
            self.abort_scan_of(world, id);
        }
    }

    pub fn set_edits(&mut self, hide: Option<(u64, i32, u32)>, carve: Option<(u64, Carve)>) {
        self.hide = hide;
        self.carve = carve;
    }

    fn drop_work(&mut self, world: &mut World, id: u32, emit: &mut impl FnMut(Event)) {
        self.abort_scan_of(world, id);
        if matches!(&self.bake_job, Some(j) if j.id == id) {
            self.abort_bake(world);
        }
        if let Some(c) = self.chunks.get_mut(&id) {
            if let Some(r) = c.runs.take() {
                world.pools[world.volumes.mesh_pool()].release(r.at, r.bytes);
                emit(Event::Runs { id, bytes: 0 });
            }
        }
    }

    fn abort_scan_of(&mut self, world: &mut World, id: u32) {
        if let Some(job) = self.scan_job.take_if(|j| j.id == id) {
            self.scan.abort();
            world.volumes.release(world.pools, job.volume);
        }
    }

    /// The pool refused chunk `id` a block: the slot it held is given up —
    /// held, it is every other chunk's wait, and the chunk cannot use it
    /// until the pool changes — and the chunk waits for a release.
    fn refuse(&mut self, world: &mut World, id: u32) {
        self.stat.starved += 1;
        self.abort_scan_of(world, id);
        if matches!(&self.bake_job, Some(j) if j.id == id) {
            self.abort_bake(world);
        }
        let releases = world.pools[world.volumes.mesh_pool()].stat.releases;
        if let Some(c) = self.chunks.get_mut(&id) {
            c.refused = Some(releases);
        }
    }

    fn abort_bake(&mut self, world: &mut World) {
        if let Some(job) = self.bake_job.take() {
            self.bake.abort();
            if let Some((at, bytes)) = job.mesh {
                world.pools[world.volumes.mesh_pool()].release(at, bytes);
            }
        }
    }

    // ---- the tick ----

    /// Advances the chunks in `order` under `budget_ms` of `now`, the
    /// holders of the scan and the bake slot first: a slot held is every
    /// other chunk's wait. At least one step is taken, whatever the budget.
    pub fn tick(&mut self, world: &mut World, order: &[u32], budget_ms: f64, now: &impl Fn() -> f64, emit: &mut impl FnMut(Event)) {
        self.tick_capped(world, order, budget_ms, u32::MAX, now, emit);
    }

    /// `tick`, stopping once `max_chunks` chunks have made progress: the
    /// scheduler's cap on how many chunks move in a frame. A chunk that
    /// waits on a slot or a block does not count. A block refused ends the
    /// tick: the pool does not change until the scheduler reclaims after,
    /// so every later request would meet the same pool, and a refusal is
    /// not free — the direct runs are written whole before their block is
    /// asked for. A thousand chunks pending on a starved pool spun a
    /// hundred milliseconds a frame this way (2026-09-14).
    pub fn tick_capped(&mut self, world: &mut World, order: &[u32], budget_ms: f64, max_chunks: u32, now: &impl Fn() -> f64, emit: &mut impl FnMut(Event)) {
        let t0 = now();
        let starved = self.stat.starved;
        let mut stepped = false;
        let mut moved = 0;
        let holders = self.holders();
        let mut seen: Vec<u32> = Vec::new();
        for id in holders.into_iter().flatten().chain(order.iter().copied()) {
            if seen.contains(&id) {
                continue;
            }
            seen.push(id);
            if moved >= max_chunks {
                return;
            }
            let mut progressed = false;
            loop {
                if stepped && now() - t0 >= budget_ms {
                    return;
                }
                let progress = self.advance(world, id, emit);
                if self.stat.starved != starved {
                    return;
                }
                if progress != Progress::Blocked {
                    stepped = true;
                }
                if progress != Progress::Busy {
                    break;
                }
                progressed = true;
            }
            if progressed {
                moved += 1;
            }
        }
    }

    fn advance(&mut self, world: &mut World, id: u32, emit: &mut impl FnMut(Event)) -> Progress {
        let Some(c) = self.chunks.get(&id) else { return Progress::Done };
        if c.wanted().next().is_none() {
            return Progress::Done;
        }
        if c.refused.is_some_and(|r| r == world.pools[world.volumes.mesh_pool()].stat.releases) {
            return Progress::Blocked;
        }
        if c.runs.is_none() {
            return self.advance_runs(world, id, emit);
        }
        self.advance_bake(world, id, emit)
    }

    fn advance_runs(&mut self, world: &mut World, id: u32, emit: &mut impl FnMut(Event)) -> Progress {
        let c = &self.chunks[&id];
        let Some(page) = c.page else { return Progress::Blocked };
        let holding = matches!(&self.scan_job, Some(j) if j.id == id);
        // A column with nothing placed is one run of terrain, floor to top,
        // whichever way it is made: the direct runs and the scan bake the
        // same mesh at every tier (core_build.test.ts), and the scan's
        // exactness — a cube carving the terrain, a sub-cell object — is
        // only ever about what is placed. The direct path used to stop at
        // step 8 on the principle that the fine tiers keep the voxel
        // pipeline; a zoom-in then scanned every chunk in view, about
        // 25 a second, most of them empty of objects (2026-09-14).
        if !holding && !self.has_objects(world, c.base) {
            return self.direct_runs(world, id, page, emit);
        }
        if !holding {
            if self.scan_job.is_some() {
                return Progress::Blocked;
            }
            match self.compose(world, id, page, true) {
                Some(volume) => {
                    let bands: Vec<BandView> = (0..world.volumes.band_count(volume)).map(|i| unsafe { world.volumes.band_view(volume, i) }).collect();
                    assert!(self.scan.begin(&bands), "a scan open with no job naming it");
                    self.scan_job = Some(ScanJob { id, volume });
                }
                None => {
                    self.refuse(world, id);
                    return Progress::Blocked;
                }
            }
        }
        let volume = self.scan_job.as_ref().unwrap().volume;
        let bands: Vec<BandView> = (0..world.volumes.band_count(volume)).map(|i| unsafe { world.volumes.band_view(volume, i) }).collect();
        match self.scan.step(&bands) {
            ScanStep::Working => Progress::Busy,
            ScanStep::Done => unreachable!("a scan is finished when its set is provided"),
            ScanStep::NeedSet(bytes) => {
                let pool = world.volumes.mesh_pool();
                let Some(at) = world.pools[pool].acquire(bytes) else {
                    self.refuse(world, id);
                    return Progress::Blocked;
                };
                // SAFETY: a block of the bytes asked for, the chunk's until released.
                unsafe { self.scan.provide_set(world.volumes.mem().add(at), bytes) };
                let count = self.scan.finish();
                let job = self.scan_job.take().unwrap();
                world.volumes.release(world.pools, job.volume);
                self.stat.scans += 1;
                self.keep_runs(id, Runs { at, bytes, count, direct: false }, emit);
                Progress::Busy
            }
        }
    }

    fn has_objects(&self, world: &World, base: u64) -> bool {
        world.store.chunk_lanes(base).is_some_and(|l| l.count > 0) || matches!(self.carve, Some((k, _)) if k == base)
    }

    pub fn page_view(world: &World, page: PageLanes) -> PageView<'static> {
        let mem = world.volumes.mem();
        let cells = (PAGE_CELL_SPAN * PAGE_CELL_SPAN) as usize;
        let samples = (ELE_PADDED_SPAN * ELE_PADDED_SPAN) as usize;
        // SAFETY: the glue names a page's lanes only while the page is
        // resident, and a page's shape is fixed.
        unsafe {
            PageView {
                ground: page.ground.map(|at| core::slice::from_raw_parts(mem.add(at), cells)),
                ele: core::slice::from_raw_parts(mem.add(page.ele) as *const i16, samples),
            }
        }
    }

    /// Paints the terrain and then the objects into a new volume, sealed;
    /// none when a paint was refused. With `edits`, the drawn build's hide
    /// and carve; without, the real set — what a raycast tests.
    fn compose(&mut self, world: &mut World, id: u32, page: PageLanes, edits: bool) -> Option<usize> {
        let c = &self.chunks[&id];
        let (base, tx0, ty0, show) = (c.base, c.tx0, c.ty0, c.show_structures);
        self.sample.read(&Self::page_view(world, page), world.materials, tx0, ty0, show);
        let volume = world.volumes.open();
        let mut ok = self.sample.paint(world.volumes, world.pools, volume);
        if ok {
            let hide = self.hide.filter(|h| edits && h.0 == base).map(|h| (h.1, h.2));
            let carve = self.carve.filter(|c| edits && c.0 == base).map(|c| c.1);
            ok = !world.painter.paint(world.store, world.volumes, world.pools, volume, base, hide, carve).refused;
        }
        if !ok {
            world.volumes.release(world.pools, volume);
            return None;
        }
        world.volumes.seal(volume);
        Some(volume)
    }

    fn direct_runs(&mut self, world: &mut World, id: u32, page: PageLanes, emit: &mut impl FnMut(Event)) -> Progress {
        let c = &self.chunks[&id];
        let (tx0, ty0, show) = (c.tx0, c.ty0, c.show_structures);
        self.sample.read(&Self::page_view(world, page), world.materials, tx0, ty0, show);
        assert!(self.direct.begin_direct(), "the direct writer left open");
        let floor = self.sample.floor;
        for i in 0..(CHUNK_SPAN * CHUNK_SPAN) as usize {
            self.direct.direct_column(i as u32);
            let top = self.sample.ele[i] as i32 - 1;
            if top >= floor {
                self.direct.direct_run(floor, top, self.sample.material[i], self.sample.layer_idx[i]);
            }
        }
        let bytes = self.direct.close_direct();
        let pool = world.volumes.mesh_pool();
        let Some(at) = world.pools[pool].acquire(bytes) else {
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

    fn keep_runs(&mut self, id: u32, runs: Runs, emit: &mut impl FnMut(Event)) {
        emit(Event::Runs { id, bytes: runs.bytes });
        self.chunks.get_mut(&id).unwrap().runs = Some(runs);
    }

    fn run_set(world: &World, runs: &Runs) -> RunSet<'static> {
        let bytes = crate::run_set::run_set_bytes(runs.count, CHUNK_SPAN) as usize;
        // SAFETY: the chunk's own block, held until its runs are let go.
        unsafe { RunSet::carve(core::slice::from_raw_parts(world.volumes.mem().add(runs.at), bytes), runs.count, CHUNK_SPAN) }
    }

    fn advance_bake(&mut self, world: &mut World, id: u32, emit: &mut impl FnMut(Event)) -> Progress {
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
        let pool = world.volumes.mesh_pool();
        match self.bake.step(&set) {
            Step::Working => Progress::Busy,
            Step::NeedMesh(words) => {
                let Some(at) = world.pools[pool].acquire(words * 4) else {
                    self.refuse(world, id);
                    return Progress::Blocked;
                };
                // SAFETY: a block of the words asked for, the bake's until it
                // is finished and handed over, or aborted and released.
                unsafe { self.bake.provide_mesh(world.volumes.mem().add(at) as *mut u32, words) };
                self.bake_job.as_mut().unwrap().mesh = Some((at, words * 4));
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
                // wanted: a chunk's worth is 0.15 ms to make again, and a
                // thousand of them held is a hundred megabytes of small
                // blocks the fine tiers' large ones then fail to find room
                // among (2026-09-14). A scan's runs stay: those cost the scan.
                if c.wanted().next().is_none() {
                    if let Some(r) = c.runs.take_if(|r| r.direct) {
                        world.pools[pool].release(r.at, r.bytes);
                        emit(Event::Runs { id, bytes: 0 });
                    }
                }
                emit(Event::Tier {
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
