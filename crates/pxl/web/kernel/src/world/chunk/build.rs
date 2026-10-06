// pxl-kernel/world/chunk/build.rs

//! A chunk's build: makes its tiers from its sources (source.rs). Every
//! decision about a chunk is the table's (table.rs); this acts on what it asks.

use pxl_spec::procedural::Frame;
use pxl_spec::space::pyramid::{Tile, CHUNK_DRAWN, GROUND};

use pxl_render::gpu::api::{gpu, room};
use pxl_render::gpu::renderer::device_ready;
use pxl_render::mesh::chunk_mesher::{box_words, mesh_words, BakeContext, Heights, Step, COARSE_BOX_WORDS, FINE_BOX_WORDS};
use pxl_render::mesh::paint::column::{write_column_runs, Stands};
use pxl_render::mesh::paint::object::adapter::Painter;
use pxl_render::mesh::paint::terrain::adapter::{beside_index, Around, ChunkSample, TileSource, TileView};
use pxl_render::mesh::run::{RunSet, RunWriter, APRON, COL_SPAN};
use pxl_spec::bake::constant::{unpack_pos, EntryAt, BAND, CELL};
use pxl_spec::space::pvf3::ivec::{self, IVec};
use std::collections::HashMap;
use xpute_runtime::mem::base::ptr_at;
use xpute_runtime::mem::heap::{alloc, free, Block};

use crate::layer::marker::hover_cursor::render_edits;
use crate::layer::object::store::{At, Store, AIR};
use crate::mem::fixed::{at, Range};
use crate::shell::setting::store::settings_store;

use crate::world::chunk::mesh_slot::{capacity, COARSE_BOXES, FINE_BOXES, OCCUPANCY};
use crate::world::pyramid::chunk::Chunk as PyramidChunk;
use crate::world::pyramid::Layer as _;
use pxl_spec::bake::format::Terrain;
use pxl_spec::space::hierarchy::offset_in;
use pxl_spec::space::indexer::idx;
use pxl_spec::space::zoom::{count, index, index_of, units, Zoom};
use xpute_core::status::bug::OrBug;
use xpute_core::status::errno::Errno;

/// How many of a chunk's drawn cells a tier drawing `cell_zoom` spans along
/// an axis (the mesher's step): one, or the whole chunk for a cover piece.
pub fn mesh_step(cell_zoom: Zoom, c: PyramidChunk) -> u32 {
    count(c.drawn_zoom(), cell_zoom)
}

/// The world's units a run's cell is tall: a leaf cell's, or past the
/// leaves the chunk's drawn cell.
pub fn z_unit(c: PyramidChunk) -> f64 {
    units(c.drawn_zoom().max(CELL))
}

// A chunk's runs live in a heap block exactly as large as its column needs,
// held until all its tiers are baked. Not a `.fixed` range: a column's runs
// are unbounded, and a range too small would leave a chunk undrawn; the heap
// refuses only when memory is out, and then the build is `Blocked`.

/// `.fixed.bake`: the open bake writes its mesh here one slot's worth at a
/// time, so it is sized to the largest slot.
pub const BAKE_BYTES: u32 = {
    let (fine, coarse) = (FINE_BOXES * FINE_BOX_WORDS, COARSE_BOXES * COARSE_BOX_WORDS);
    4 * if fine > coarse { fine } else { coarse }
};

pub const RUN_SETS: usize = 2;

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum BuildEvent {
    /// All slots for a tier are asked for at once, before any is written. If
    /// refused, the bake is thrown away and the need recorded (`Slot::need`).
    Reserve { id: u32, generation: u32, cell_zoom: Zoom, boxes: u32 },
    /// One slot's worth of boxes (less for the last) at `at` in `.fixed.bake`.
    Part {
        id: u32,
        generation: u32,
        cell_zoom: Zoom,
        at: u32,
        first: u32,
        box_cnt: u32,
        heights: Heights,
    },
    Tier {
        id: u32,
        generation: u32,
        cell_zoom: Zoom,
        box_cnt: u32,
        heights: Heights,
    },
}

/// A column of chunks one above another shares its runs: they are made once
/// and each chunk bakes its cut of them.
struct RunHold {
    column: (Zoom, [i64; 2]),
    users: Vec<u32>,
    runs: Option<Runs>,
}

impl Drop for RunHold {
    fn drop(&mut self) {
        if let Some(r) = self.runs.take() {
            // SAFETY: the hold's own block; every bake from it ends before the hold does.
            unsafe { free(r.block) };
        }
    }
}

#[derive(Clone, Copy)]
struct Runs {
    block: (usize, u32),
    bytes: u32,
    count: u32,
}

/// Where a tile's ground is read from: itself (`n` = 1) or the nearest
/// resident tile over it (`drawn_from`).
#[derive(Clone, Copy)]
struct Source {
    tile: &'static Terrain,
    frame: Frame,
    origin: [i64; 2],
    n: u32,
}

impl Source {
    /// Found by key, so a tile that is gone never yields another tile's bytes.
    fn of(p: Tile) -> Option<Source> {
        let pyramid = Terrain::pyramid();
        let src = pyramid.residency().drawn_from(p)?;
        let tile = pyramid.residency().get(src)?;
        let corner = GROUND.cell_at(p, GROUND.cell_index(0, 0));
        Some(Source {
            tile,
            frame: Frame::of(src),
            origin: corner.in_tile_above(p.zoom - src.zoom),
            n: count(GROUND.cell_zoom(p.zoom), GROUND.cell_zoom(src.zoom)),
        })
    }

    fn view(&self) -> TileSource<'static> {
        TileSource {
            tile: Builder::tile_view(self.tile),
            frame: self.frame,
            origin: self.origin,
            n: self.n,
        }
    }
}

/// The apron reads each neighbour's tile from what that neighbour is drawn
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
    want: [Option<Zoom>; 2],
    /// Heap release count at the last refusal: until something is freed the same
    /// request would fail, so the chunk is skipped.
    refused: Option<u32>,
}

impl Chunk {
    fn base(&self) -> IVec {
        self.at.corner()
    }

    fn col_idx(&self) -> IVec {
        idx(BAND, self.base())
    }

    /// The chunk's tile and the apron beside it; past the leaves, its leaf
    /// chunk's, which its sample refines (`ChunkSample::refine`).
    fn read(&self) -> Option<Read> {
        let leaf = self.leaf();
        let mut beside = [None; 8];
        for ([qx, qy], p) in leaf.tiles_beside(APRON) {
            beside[beside_index(qx, qy)] = Source::of(p);
        }
        let (x0, y0) = leaf.in_tile();
        Some(Read {
            own: Source::of(leaf.tile())?,
            beside,
            first: [x0 as i64, y0 as i64],
        })
    }

    /// Itself, or past the leaves the leaf chunk it lies in.
    fn leaf(&self) -> PyramidChunk {
        core::iter::once(self.at).chain(self.at.ancestors()).find(|k| k.finer() == 0).or_bug(Errno::ENOTRECOVERABLE)
    }

    /// Where its first drawn cell lies in its leaf chunk, in its drawn cells.
    fn in_leaf(&self) -> [u32; 2] {
        let k = self.at.finer();
        offset_in(self.at.column().1, k).map(|v| v as u32 * CHUNK_DRAWN)
    }

    fn wants(&self, cell_zoom: Zoom) -> bool {
        self.want.contains(&Some(cell_zoom))
    }

    fn wanted(&self) -> impl Iterator<Item = Zoom> + '_ {
        self.want.iter().copied().flatten()
    }
}

struct BakeJob {
    id: u32,
    cell_zoom: Zoom,
    generation: u32,
    mesh: Option<Block>,
    handed: u32,
}

#[derive(PartialEq, Eq)]
enum Progress {
    Busy,
    Done,
    /// Waiting on another chunk's slot, a refused heap block, or a tile.
    Blocked,
}

pub struct World<'a> {
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
    pub runs: u32,
    pub bakes: u32,
    pub starved: u32,
}

pub struct Builder {
    chunks: ByMount<Chunk>,
    direct: RunWriter,
    bake: BakeContext,
    bake_job: Option<BakeJob>,
    sample: ChunkSample,
    /// A leaf chunk's, which a chunk past the leaves refines.
    leaf_sample: ChunkSample,
    run_holds: [Option<RunHold>; RUN_SETS],
    folded: HashMap<[i64; 3], u32>,
    stands: Vec<(u32, Stands)>,
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
            direct: RunWriter::new(),
            bake: BakeContext::new(),
            bake_job: None,
            sample: ChunkSample::default(),
            leaf_sample: ChunkSample::default(),
            run_holds: [const { None }; RUN_SETS],
            folded: HashMap::new(),
            stands: Vec::new(),
            stat: BuildStat::default(),
        }
    }

    pub fn generation_of(&self, id: u32) -> Option<u32> {
        self.chunks.get(&id).map(|c| c.generation)
    }

    pub fn holders(&self) -> [Option<u32>; 1] {
        [self.bake_job.as_ref().map(|j| j.id)]
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

    pub fn remove(&mut self, id: u32) {
        self.abort_work_of(id);
        self.leave_runs(id);
        self.chunks.remove(&id);
    }

    pub fn source(&mut self, id: u32) {
        self.drop_work(id);
        if let Some(c) = self.chunks.get_mut(&id) {
            c.generation = c.generation.wrapping_add(1);
        }
    }

    pub fn baking(&self) -> Option<(u32, Zoom, u32)> {
        self.bake_job.as_ref().map(|j| (j.id, j.cell_zoom, j.generation))
    }

    /// A bake of a tier no longer wanted gives its slot back now.
    pub fn want(&mut self, id: u32, want: [Option<Zoom>; 2]) {
        let Some(c) = self.chunks.get_mut(&id) else { return };
        c.want = want;
        let stale = matches!(&self.bake_job, Some(j) if j.id == id && !c.wants(j.cell_zoom));
        if stale {
            self.abort_bake();
        }
        if want == [None, None] {
            self.leave_runs(id);
        }
    }

    /// The column's runs are shared by every chunk over it, so all of them rebuild.
    fn drop_work(&mut self, id: u32) {
        self.abort_work_of(id);
        self.drop_runs(id);
    }

    fn abort_work_of(&mut self, id: u32) {
        if matches!(&self.bake_job, Some(j) if j.id == id) {
            self.abort_bake();
        }
    }

    fn hold_of(&self, id: u32) -> Option<usize> {
        let column = self.chunks.get(&id)?.at.column();
        self.run_holds.iter().position(|h| h.as_ref().is_some_and(|h| h.column == column))
    }

    fn runs_of(&self, id: u32) -> Option<Runs> {
        self.run_holds[self.hold_of(id)?].as_ref()?.runs
    }

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
    fn refuse(&mut self, id: u32) {
        self.stat.starved += 1;
        self.abort_work_of(id);
        self.leave_runs(id);
        let releases = crate::host::heap_stat().releases;
        if let Some(c) = self.chunks.get_mut(&id) {
            c.refused = Some(releases);
        }
    }

    fn unwant(&mut self, id: u32, cell_zoom: Zoom) {
        let Some(c) = self.chunks.get_mut(&id) else { return };
        for w in c.want.iter_mut().filter(|w| **w == Some(cell_zoom)) {
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

    /// Advances chunks under the budget, slot holders first. At least one step
    /// is taken. A heap refusal ends the tick: the heap will not change within
    /// it, and a refusal is not free (direct runs are written before their
    /// block is asked for).
    pub fn tick(&mut self, world: &mut World, order: &[u32], budget_ms: f64, now: &impl Fn() -> f64, emit: &mut impl FnMut(BuildEvent) -> bool) -> bool {
        let t0 = now();
        let starved = self.stat.starved;
        let mut stepped = false;
        let mut worked = false;
        let mut first = [None; 1 + RUN_SETS];
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
        if self.runs_of(id).is_none() {
            return self.advance_runs(world, id);
        }
        // Another chunk of the column made the runs; this one bakes its cut too.
        self.run_set_for(id);
        self.advance_bake(id, emit)
    }

    /// A chunk's runs, written directly from its tile's ground and the objects
    /// folded to its cells (mesh/paint/column.rs).
    fn advance_runs(&mut self, world: &mut World, id: u32) -> Progress {
        let c = &self.chunks[&id];
        let Some(read) = c.read() else {
            return Progress::Blocked;
        };
        if self.run_set_for(id).is_none() {
            return Progress::Blocked;
        }
        let c = self.chunks[&id];
        let k = c.at.finer();
        let sampled = if k == 0 {
            self.sample.read(&read.around(), read.first)
        } else {
            self.leaf_sample.read(&read.around(), read.first).and_then(|()| self.sample.refine(&self.leaf_sample, k, c.in_leaf()))
        };
        if sampled.is_err() {
            self.refuse(id);
            return Progress::Blocked;
        }
        let (target, base) = (c.at.drawn_zoom(), c.base());
        let (x0, y0) = (ivec::X(base), ivec::Y(base));
        // One cell of apron, which a fold reads (`tiers_draw`).
        let (cx, cy, span) = (index(target, x0), index(target, y0), CHUNK_DRAWN as i64);
        let edits = render_edits();
        if world.store.fold_at(target, [cx - 1, cy - 1], [cx + span + 1, cy + span + 1], edits.hide, &mut self.folded).is_err() {
            self.refuse(id);
            return Progress::Blocked;
        }
        if let Some(carve) = edits.carve.filter(|e| e.col_idx == c.col_idx()) {
            carve_into(&mut self.folded, carve, target);
        }
        xpute_core::ensure!(self.direct.begin(), ENOTRECOVERABLE);
        let written = write_column_runs(
            &mut self.direct,
            &self.sample,
            world.painter,
            &self.folded,
            (cx, cy),
            (units(target) / z_unit(c.at)) as u32,
            &mut self.stands,
        );
        if written.is_err() {
            self.direct.abort();
            self.refuse(id);
            return Progress::Blocked;
        }
        self.close(id)
    }

    fn close(&mut self, id: u32) -> Progress {
        let bytes = self.direct.close();
        let Some(block) = self.set_of(id, bytes) else {
            self.direct.abort();
            self.refuse(id);
            return Progress::Blocked;
        };
        // SAFETY: as for a scan's set.
        unsafe { self.direct.provide_set(ptr_at::<u8>(block.0), bytes) };
        let count = self.direct.finish();
        self.stat.runs += 1;
        self.keep_runs(id, Runs { block, bytes, count });
        Progress::Busy
    }

    pub fn tile_view(tile: &Terrain) -> TileView<'_> {
        TileView {
            cells: &tile.ground.cells,
            ele: &tile.ele,
            decks: &tile.decks,
        }
    }

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
            // SAFETY: the earlier runs' block, which nothing bakes from once replaced.
            unsafe { free(old.block) };
        }
    }

    fn run_set(runs: &Runs) -> RunSet<'static> {
        let bytes = pxl_render::mesh::run::run_set_bytes(runs.count, COL_SPAN) as usize;
        xpute_core::ensure!(bytes as u32 <= runs.block.1, ENOTRECOVERABLE);
        // SAFETY: the chunk's own block, held until its runs are let go.
        unsafe { RunSet::carve(core::slice::from_raw_parts(ptr_at::<u8>(runs.block.0), bytes), runs.count, COL_SPAN, APRON) }
    }

    fn advance_bake(&mut self, id: u32, emit: &mut impl FnMut(BuildEvent) -> bool) -> Progress {
        let holding = matches!(&self.bake_job, Some(j) if j.id == id);
        let c = &self.chunks[&id];
        let set = Self::run_set(&self.runs_of(id).or_bug(Errno::ENOTRECOVERABLE));
        if !holding {
            if self.bake_job.is_some() {
                return Progress::Blocked;
            }
            let cell_zoom = c.wanted().next().unwrap();
            // Runs span the whole column; this chunk bakes only its own heights, in its runs' cells.
            let (z0, z1) = c.at.z_range();
            let z = z_unit(c.at);
            let clip = Some(((z0 / z) as i32, (z1 / z) as i32));
            let (unit, fine) = (units(c.at.drawn_zoom()), c.at.drawn_zoom() >= CELL);
            if self.bake.begin(&set, mesh_step(cell_zoom, c.at), unit, z, fine, settings_store().textures, clip).is_err() {
                self.refuse(id);
                return Progress::Blocked;
            }
            self.bake_job = Some(BakeJob {
                id,
                cell_zoom,
                generation: c.generation,
                mesh: None,
                handed: 0,
            });
        }
        match self.bake.step(&set) {
            Step::Working => Progress::Busy,
            Step::Refused => {
                self.bake_job = None;
                self.refuse(id);
                Progress::Blocked
            }
            Step::NeedMesh(words) => {
                // All slots are held at once or the bake is thrown away: a mesh is never
                // cut, never baked coarser, and never waits while holding the one bake.
                let job = self.bake_job.as_ref().unwrap();
                let fine = mesh_step(job.cell_zoom, c.at) == 1 && c.at.drawn_zoom() >= CELL;
                let (words_a_box, room_a_slot) = (box_words(fine), capacity(c.at.zoom(), job.cell_zoom));
                let boxes = words / words_a_box;
                let asked = BuildEvent::Reserve {
                    id,
                    generation: job.generation,
                    cell_zoom: job.cell_zoom,
                    boxes,
                };
                if !emit(asked) {
                    let job = self.bake_job.take().unwrap();
                    self.bake.abort();
                    self.unwant(id, job.cell_zoom);
                    return Progress::Busy;
                }
                let block = boxes.min(room_a_slot) * words_a_box;
                let Some(mesh_at) = at(Range::Bake, 0, block * 4) else {
                    self.abort_bake();
                    self.refuse(id);
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
                let fine = mesh_step(job.cell_zoom, c.at) == 1 && c.at.drawn_zoom() >= CELL;
                let words_a_box = box_words(fine);
                let n = bytes / 4 / words_a_box;
                emit(BuildEvent::Part {
                    id,
                    generation: job.generation,
                    cell_zoom: job.cell_zoom,
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
                        cell_zoom: job.cell_zoom,
                        at: at as u32,
                        first: job.handed,
                        box_cnt: mesh.box_cnt - job.handed,
                        heights: mesh.heights,
                    });
                }
                self.unwant(id, job.cell_zoom);
                emit(BuildEvent::Tier {
                    id,
                    generation: job.generation,
                    cell_zoom: job.cell_zoom,
                    box_cnt: mesh.box_cnt,
                    heights: mesh.heights,
                });
                Progress::Busy
            }
        }
    }
}

/// The eraser's preview: the cells at `zoom` its entry covers, cut to air.
fn carve_into(cells: &mut HashMap<[i64; 3], u32>, carve: EntryAt, zoom: Zoom) {
    let (x, y, z, own) = unpack_pos(carve.pos);
    let corner = At {
        col_idx: carve.col_idx,
        band: carve.band_idx,
        step: [x, y, z],
    }
    .corner();
    let [ix, iy, iz] = index_of(zoom, &corner);
    let n = if own < zoom { i64::from(count(zoom, own)) } else { 1 };
    for dz in 0..n {
        for dy in 0..n {
            for dx in 0..n {
                cells.insert([ix + dx, iy + dy, iz + dz], AIR);
            }
        }
    }
}
