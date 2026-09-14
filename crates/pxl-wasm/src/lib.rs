//! The one WebAssembly module: pxl-core behind numeric exports.
//!
//! No wasm-bindgen. What crosses is integers and addresses into the core
//! memory, which the module imports and shares with the client (build.rs,
//! arena.rs): a run set is baked where it lies, and the mesh is written into
//! the block the client acquired for it, so nothing is copied across the
//! boundary. The boundary is the same shape the command frame already is,
//! and a thread that instantiates the module needs the memory and nothing
//! else.
//!
//! One bake is open at a time, as with the TypeScript context; the module is
//! single-threaded, so the state is a plain global.

mod arena;

use pxl_core::buddy::Pool;
use pxl_core::build::{Event as BuildEvent, PageLanes, World};
use pxl_core::change::ChangeLog;
use pxl_core::decor::{Kind as DecorKind, Rule as DecorRule};
use pxl_core::mesher::{BakeContext, Step};
use pxl_core::object::{Carve, Kind, Painter, Store};
use pxl_core::raycast::Raycast;
use pxl_core::ring::{ring_words, Entry, Ring, ARGS, OP, RESULT, USER};
use pxl_core::run_set::{run_set_bytes, RunSet};
use pxl_core::scan::{BandView, ColumnRuns, ExtraView, ScanContext, ScanStep};
use pxl_core::sched::{Event, Frame, Scheduler};
use pxl_core::terrain::{ChunkSample, Materials, PageView, CHUNK_SPAN, ELE_PADDED_SPAN, PAGE_CELL_SPAN};
use pxl_core::volume::{Cell, Volumes, Voxel, BAND_WORDS as VOLUME_BAND_WORDS};

#[global_allocator]
static ALLOC: arena::Arena = arena::Arena;

struct State {
    ctx: BakeContext,
    /// The open bake's run set: its address in the memory, and its shape.
    input: (usize, u32, u32),
    /// vertex_cnt, index_cnt, the six counters, then the arena's use.
    result: [u32; 9],
    scan: ScanContext,
    /// The open scan's band table: its address, and how many bands.
    bands: (usize, u32),
    /// The pools over the memory's sections, by the id pxl_pool_new gave.
    pools: Vec<Pool>,
    /// The volumes, once pxl_volume_pools has said which pools they draw on.
    volumes: Option<Volumes>,
    /// The open scan's bands, when it reads a volume of the module's.
    scan_rows: Vec<[u32; VOLUME_BAND_WORDS]>,
    /// The terrain tables, and the last chunk read.
    terrain: Materials,
    chunk: ChunkSample,
    /// The object store, its painter, and the rows its lookups answer in.
    objects: Store,
    painter: Painter,
    object_row: [u32; 4],
    /// The raycast's column, and the row naming its lanes.
    column: ColumnRuns,
    column_row: [u32; 4],
    /// The submission ring (glue → core) and the completion ring (core →
    /// glue), in the module's heap for as long as the module lives.
    sq: Ring,
    cq: Ring,
    /// The chunks' builds, and the events they made that the completion
    /// ring had no room for yet.
    sched: Scheduler,
    /// The world's change log (module/world/change.ts's topics).
    changes: ChangeLog,
    change_row: [u32; 6],
    /// The frame the glue writes before each tick (FRAME_WORDS doubles).
    frame: [f64; FRAME_WORDS],
    census: [f64; 12],
    events: std::collections::VecDeque<Event>,
    build_stat: [u32; 11],
    /// The world's raycast and its kept composites, and the last hit as
    /// doubles the glue reads.
    raycast: Raycast,
    hit: [f64; HIT_WORDS],
    draw_row: [u32; 8],
}

/// Entries each ring holds: a page mounting is 64 chunks a frame, each
/// with its tiers, decor and layout to come, and the glue reads the ring
/// dry between ticks — 4096 entries is 256 KB a ring.
pub const RING_CAPACITY: u32 = 4096;

fn new_ring(capacity: u32) -> Ring {
    let words = vec![0u32; ring_words(capacity as usize)].leak();
    // SAFETY: the words are leaked for the module's lifetime and only the
    // two sides of the ring touch them.
    unsafe { Ring::init(words.as_mut_ptr(), capacity) }
}

static mut STATE: Option<State> = None;

extern "C" {
    /// The client's: told a panic's message before the trap, since the trap
    /// itself carries none — `panic = "abort"` and no hook would leave only
    /// "unreachable" in the console.
    fn pxl_panic(ptr: *const u8, len: usize);
    /// The client's clock, in milliseconds: what a tick's budget is spent against.
    fn pxl_now() -> f64;
}

#[allow(static_mut_refs)]
fn state() -> &'static mut State {
    // SAFETY: the module runs on one thread and no export re-enters another.
    unsafe {
        STATE.get_or_insert_with(|| {
            std::panic::set_hook(Box::new(|info| {
                let text = info.to_string();
                pxl_panic(text.as_ptr(), text.len());
            }));
            State {
                ctx: BakeContext::new(),
                input: (0, 0, 0),
                result: [0; 9],
                scan: ScanContext::new(),
                bands: (0, 0),
                pools: Vec::new(),
                volumes: None,
                scan_rows: Vec::new(),
                terrain: Materials::default(),
                chunk: ChunkSample::default(),
                objects: Store::new(),
                painter: Painter::new(),
                object_row: [0; 4],
                column: ColumnRuns::new(),
                column_row: [0; 4],
                sq: new_ring(RING_CAPACITY),
                cq: new_ring(RING_CAPACITY),
                sched: Scheduler::new(),
                raycast: Raycast::default(),
                hit: [0.0; HIT_WORDS],
                draw_row: [0; 8],
                changes: ChangeLog::new(WORLD_TOPICS, JOURNAL_CAPACITY),
                change_row: [0; 6],
                frame: [0.0; FRAME_WORDS],
                census: [0.0; 12],
                events: std::collections::VecDeque::new(),
                build_stat: [0; 11],
            }
        })
    }
}

/// What a step reports: `pxl_bake_step`'s return.
pub const STEP_WORKING: u32 = 0;
pub const STEP_NEED_MESH: u32 = 1;
pub const STEP_DONE: u32 = 2;

fn input_view((at, count, span): (usize, u32, u32)) -> RunSet<'static> {
    // SAFETY: the client points the bake at a run set in a pool block, which
    // starts 8-aligned and holds run_set_bytes(count, span), and keeps the
    // block until the bake closes.
    unsafe {
        let raw = core::slice::from_raw_parts(at as *const u8, run_set_bytes(count, span) as usize);
        RunSet::carve(raw, count, span)
    }
}

/// Opens a bake at `lod_step` of the run set of `count` runs over `span`²
/// columns at address `at`, in column_run.ts's layout. 0 when a bake is
/// already open.
#[no_mangle]
pub extern "C" fn pxl_bake_begin(at: u32, count: u32, span: u32, lod_step: u32) -> u32 {
    let s = state();
    if s.ctx.is_open() {
        return 0;
    }
    s.input = (at as usize, count, span);
    s.ctx.begin(&input_view(s.input), lod_step) as u32
}

/// One step of the open bake: STEP_WORKING, STEP_NEED_MESH — the mesh's
/// words are known (`pxl_bake_mesh_words`) and it has nowhere to go until
/// `pxl_bake_mesh` — or STEP_DONE.
#[no_mangle]
pub extern "C" fn pxl_bake_step() -> u32 {
    let s = state();
    match s.ctx.step(&input_view(s.input)) {
        Step::Working => STEP_WORKING,
        Step::NeedMesh(_) => STEP_NEED_MESH,
        Step::Done => STEP_DONE,
    }
}

/// The words the open bake's mesh takes, once STEP_NEED_MESH has said so.
#[no_mangle]
pub extern "C" fn pxl_bake_mesh_words() -> u32 {
    let s = state();
    match s.ctx.step(&input_view(s.input)) {
        Step::NeedMesh(words) => words,
        _ => 0,
    }
}

/// Where the mesh goes: `words` words at address `at`, 4-aligned, the
/// client's until the bake finishes or aborts.
#[no_mangle]
pub extern "C" fn pxl_bake_mesh(at: u32, words: u32) {
    // SAFETY: the client's contract, above.
    unsafe { state().ctx.provide_mesh(at as *mut u32, words) }
}

/// Closes the complete bake and returns the address of nine words: vertex
/// count, index count, the six counters so far, and the module heap's use.
#[no_mangle]
pub extern "C" fn pxl_bake_finish() -> *const u32 {
    let s = state();
    let mesh = s.ctx.finish();
    let c = s.ctx.counters;
    s.result = [
        mesh.vertex_cnt,
        mesh.index_cnt,
        c.total_runs,
        c.culled,
        c.faces_possible,
        c.faces_kept,
        c.coarse_total,
        c.coarse_low_fill,
        arena::used() as u32,
    ];
    s.result.as_ptr()
}

#[no_mangle]
pub extern "C" fn pxl_bake_abort() {
    state().ctx.abort();
}

/// How much of the module's heap has been cut, for the ledger.
#[no_mangle]
pub extern "C" fn pxl_heap_used() -> u32 {
    arena::used() as u32
}

/// The section the module keeps for itself at the front of the memory, for
/// the client to check its map against.
#[no_mangle]
pub extern "C" fn pxl_module_bytes() -> u32 {
    arena::MODULE_BYTES as u32
}

/// Words a band takes in the table `pxl_scan_begin` reads (wasm_scan.ts
/// writes the same, and a volume of the module's says its bands in): band_z;
/// the addresses of the material, layer and sub-cell lanes (0 for no
/// sub-cells); the extras' count and the addresses of their five lanes.
pub const BAND_WORDS: usize = VOLUME_BAND_WORDS;

fn band_views((at, count): (usize, u32)) -> Vec<BandView<'static>> {
    // SAFETY: the table is either the client's, written into a lane of its
    // own with every address naming a lane of a band it keeps until the scan
    // closes, or the module's own, copied from a volume it holds;
    // ScanContext::begin checks each lane's length against the band's.
    unsafe {
        let table = core::slice::from_raw_parts(at as *const u32, count as usize * BAND_WORDS);
        table
            .chunks_exact(BAND_WORDS)
            .map(|w| {
                let voxels = (pxl_core::scan::BAND_SPAN * pxl_core::scan::BAND_SPAN * pxl_core::scan::BAND_SPAN) as usize;
                let extras = w[4] as usize;
                BandView {
                    band_z: w[0] as i32,
                    material: core::slice::from_raw_parts(w[1] as usize as *const u32, voxels),
                    layer_idx: core::slice::from_raw_parts(w[2] as usize as *const u8, voxels),
                    sub_cell: (w[3] != 0).then(|| core::slice::from_raw_parts(w[3] as usize as *const u8, voxels * 4)),
                    extra: ExtraView {
                        col: core::slice::from_raw_parts(w[5] as usize as *const u32, extras),
                        z: core::slice::from_raw_parts(w[6] as usize as *const i32, extras),
                        material: core::slice::from_raw_parts(w[7] as usize as *const u32, extras),
                        layer_idx: core::slice::from_raw_parts(w[8] as usize as *const u8, extras),
                        sub: core::slice::from_raw_parts(w[9] as usize as *const u32, extras),
                    },
                }
            })
            .collect()
    }
}

/// Opens a scan of the `count` bands described at `at`, ascending by
/// band_z. 0 when a scan is already open.
#[no_mangle]
pub extern "C" fn pxl_scan_begin(at: u32, count: u32) -> u32 {
    let s = state();
    if s.scan.is_open() {
        return 0;
    }
    s.bands = (at as usize, count);
    s.scan.begin(&band_views(s.bands)) as u32
}

/// One step of the open scan: STEP_WORKING, STEP_NEED_MESH — here the run
/// set, whose bytes `pxl_scan_set_bytes` says, to be given by
/// `pxl_scan_set` — or STEP_DONE.
#[no_mangle]
pub extern "C" fn pxl_scan_step() -> u32 {
    let s = state();
    match s.scan.step(&band_views(s.bands)) {
        ScanStep::Working => STEP_WORKING,
        ScanStep::NeedSet(_) => STEP_NEED_MESH,
        ScanStep::Done => STEP_DONE,
    }
}

#[no_mangle]
pub extern "C" fn pxl_scan_set_bytes() -> u32 {
    let s = state();
    match s.scan.step(&band_views(s.bands)) {
        ScanStep::NeedSet(bytes) => bytes,
        _ => 0,
    }
}

/// Where the run set goes: `bytes` bytes at `at`, 4-aligned, the client's.
/// Packs it now; the next step is STEP_DONE.
#[no_mangle]
pub extern "C" fn pxl_scan_set(at: u32, bytes: u32) {
    // SAFETY: the client's contract, above.
    unsafe { state().scan.provide_set(at as *mut u8, bytes) }
}

/// Closes the packed scan: the run count of the set it packed.
#[no_mangle]
pub extern "C" fn pxl_scan_finish() -> u32 {
    state().scan.finish()
}

#[no_mangle]
pub extern "C" fn pxl_scan_abort() {
    state().scan.abort();
}

/// A pool over `bytes` bytes of the memory at `start` — a section of the
/// map — cut in regions of `region_bytes` with blocks of at least
/// `min_block_bytes`: its id, for the calls below. The client's
/// BuddyAllocator delegates its backed range here and keeps what it does
/// past it (buddy.ts).
#[no_mangle]
pub extern "C" fn pxl_pool_new(start: u32, bytes: u32, region_bytes: u32, min_block_bytes: u32) -> u32 {
    let s = state();
    // SAFETY: the memory is the module's own linear memory, addressed from
    // 0, and the client keeps the range for this pool alone.
    let pool = unsafe { Pool::new(core::ptr::null_mut(), start as usize, bytes as usize, region_bytes, min_block_bytes) };
    s.pools.push(pool);
    s.pools.len() as u32 - 1
}

/// Borrows at least `bytes` from pool `id`: the block's address, or 0 when
/// the range is used up or the request is larger than a region.
#[no_mangle]
pub extern "C" fn pxl_pool_acquire(id: u32, bytes: u32) -> u32 {
    state().pools[id as usize].acquire(bytes).map_or(0, |at| at as u32)
}

/// Returns the block of `bytes` at `at` to pool `id`: 1, or 0 for a block
/// the pool does not hold or holds already.
#[no_mangle]
pub extern "C" fn pxl_pool_release(id: u32, at: u32, bytes: u32) -> u32 {
    state().pools[id as usize].release(at as usize, bytes) as u32
}

/// Whether the block of `bytes` at `at` is free in pool `id`.
#[no_mangle]
pub extern "C" fn pxl_pool_is_free(id: u32, at: u32, bytes: u32) -> u32 {
    state().pools[id as usize].is_free(at as usize, bytes) as u32
}

/// The address of pool `id`'s stat: twelve words in PoolStat's order.
#[no_mangle]
pub extern "C" fn pxl_pool_stat(id: u32) -> *const u32 {
    &state().pools[id as usize].stat as *const _ as *const u32
}

fn volumes() -> &'static mut Volumes {
    state().volumes.as_mut().expect("pxl_volume_pools before any volume")
}

/// Which pools volumes draw on: the standing band stores from `fixed`,
/// the rest and the sub-cell lanes from `mesh` (ids from pxl_pool_new).
#[no_mangle]
pub extern "C" fn pxl_volume_pools(fixed: u32, mesh: u32) {
    let s = state();
    // SAFETY: the pools were cut from this module's memory, addressed from 0.
    s.volumes = Some(unsafe { Volumes::new(core::ptr::null_mut(), fixed, mesh) });
}

/// A volume with no bands: its id plus one (0 is never a volume).
#[no_mangle]
pub extern "C" fn pxl_volume_open() -> u32 {
    volumes().open() as u32 + 1
}

/// Paints one solid column of `material`/`layer_idx` from z0 to z1. 0 when a
/// band could not be made for it — no pool had room — and the volume is
/// then not the composite asked for.
#[no_mangle]
pub extern "C" fn pxl_volume_fill_column(id: u32, cx: u32, cy: u32, z0: i32, z1: i32, material: u32, layer_idx: u32) -> u32 {
    let s = state();
    let v = s.volumes.as_mut().expect("pxl_volume_pools before any volume");
    v.fill_column(
        &mut s.pools,
        id as usize - 1,
        (cx, cy),
        z0,
        z1,
        Voxel {
            material,
            layer_idx: layer_idx as u8,
            sub: 0,
        },
    ) as u32
}

/// Full replacement at one voxel; `sub` is pack_sub_cell's word, 0 for a
/// full cell. 0 as pxl_volume_fill_column.
#[no_mangle]
pub extern "C" fn pxl_volume_set_voxel(id: u32, cx: u32, cy: u32, z: i32, material: u32, layer_idx: u32, sub: u32) -> u32 {
    let s = state();
    let v = s.volumes.as_mut().expect("pxl_volume_pools before any volume");
    v.set_voxel(
        &mut s.pools,
        id as usize - 1,
        Cell { cx, cy, z },
        Voxel {
            material,
            layer_idx: layer_idx as u8,
            sub,
        },
    ) as u32
}

/// Another sub-cell object on a base voxel already claimed.
#[no_mangle]
pub extern "C" fn pxl_volume_append_sub_voxel(id: u32, cx: u32, cy: u32, z: i32, material: u32, layer_idx: u32, sub: u32) -> u32 {
    let s = state();
    let v = s.volumes.as_mut().expect("pxl_volume_pools before any volume");
    v.append_sub_voxel(
        &mut s.pools,
        id as usize - 1,
        Cell { cx, cy, z },
        Voxel {
            material,
            layer_idx: layer_idx as u8,
            sub,
        },
    ) as u32
}

/// Closes the volume to painting and sorts its overflow by column.
#[no_mangle]
pub extern "C" fn pxl_volume_seal(id: u32) {
    volumes().seal(id as usize - 1);
}

#[no_mangle]
pub extern "C" fn pxl_volume_release(id: u32) {
    let s = state();
    let v = s.volumes.as_mut().expect("pxl_volume_pools before any volume");
    v.release(&mut s.pools, id as usize - 1);
}

#[no_mangle]
pub extern "C" fn pxl_volume_bands(id: u32) -> u32 {
    volumes().band_count(id as usize - 1) as u32
}

/// The address of band `i`'s row — BAND_WORDS words, as pxl_scan_begin
/// reads them — valid until the volume is painted again.
#[no_mangle]
pub extern "C" fn pxl_volume_band(id: u32, i: u32) -> *const u32 {
    volumes().band_row(id as usize - 1, i as usize).as_ptr()
}

/// The address of the volumes' stat: allocated, reused, refused.
#[no_mangle]
pub extern "C" fn pxl_volume_stat() -> *const u32 {
    &volumes().stat as *const _ as *const u32
}

/// Opens a scan of a volume of the module's: its bands' rows are copied
/// out, so the scan reads them as it reads a client's table. 0 when a
/// scan is already open.
#[no_mangle]
pub extern "C" fn pxl_scan_begin_volume(id: u32) -> u32 {
    let s = state();
    if s.scan.is_open() {
        return 0;
    }
    let v = s.volumes.as_mut().expect("pxl_volume_pools before any volume");
    let count = v.band_count(id as usize - 1);
    s.scan_rows.clear();
    for i in 0..count {
        s.scan_rows.push(*v.band_row(id as usize - 1, i));
    }
    s.bands = (s.scan_rows.as_ptr() as usize, count as u32);
    s.scan.begin(&band_views(s.bands)) as u32
}

/// The material every terrain cell starts from: bare land.
#[no_mangle]
pub extern "C" fn pxl_terrain_base(r: u32, g: u32, b: u32, layer: u32, class: u32) {
    let t = &mut state().terrain;
    t.base_rgb = [r as u8, g as u8, b as u8];
    t.base_layer = layer as u8;
    t.base_class = class as u8;
}

/// The material class channel `ch` paints (0 land, 1 water, 2 structure,
/// 3 transport).
#[no_mangle]
pub extern "C" fn pxl_terrain_channel(ch: u32, class: u32) {
    state().terrain.channels[ch as usize].class = class as u8;
}

/// What type `type_id` of channel `ch` looks like: its colour, if it has
/// one, and its texture-array layer, if it has one.
#[no_mangle]
pub extern "C" fn pxl_terrain_type(ch: u32, type_id: u32, r: u32, g: u32, b: u32, has_color: u32, layer: u32, has_layer: u32) {
    let c = &mut state().terrain.channels[ch as usize];
    let t = type_id as usize;
    c.rgb[t] = [r as u8, g as u8, b as u8];
    c.has_color[t] = has_color != 0;
    c.layer[t] = layer as u8;
    c.has_layer[t] = has_layer != 0;
}

/// Reads the chunk at page cell (tx0, ty0) of the page whose ground lanes
/// (land, water, structure, transport; a type byte a cell) and padded
/// elevation raster are at the addresses given: its cells' materials and
/// tops, for pxl_terrain_paint and the lanes below. Returns the floor.
#[no_mangle]
pub extern "C" fn pxl_terrain_read(land_at: u32, water_at: u32, structure_at: u32, transport_at: u32, ele_at: u32, tx0: u32, ty0: u32, show_structures: u32) -> i32 {
    let s = state();
    let cells = (PAGE_CELL_SPAN * PAGE_CELL_SPAN) as usize;
    let samples = (ELE_PADDED_SPAN * ELE_PADDED_SPAN) as usize;
    assert!(tx0 + CHUNK_SPAN <= PAGE_CELL_SPAN && ty0 + CHUNK_SPAN <= PAGE_CELL_SPAN, "a chunk past its page");
    // SAFETY: the client's page lanes, kept in the core memory for as long
    // as the page is resident; a page's shape is fixed.
    let page = unsafe {
        PageView {
            ground: [
                core::slice::from_raw_parts(land_at as usize as *const u8, cells),
                core::slice::from_raw_parts(water_at as usize as *const u8, cells),
                core::slice::from_raw_parts(structure_at as usize as *const u8, cells),
                core::slice::from_raw_parts(transport_at as usize as *const u8, cells),
            ],
            ele: core::slice::from_raw_parts(ele_at as usize as *const i16, samples),
        }
    };
    s.chunk.read(&page, &s.terrain, tx0, ty0, show_structures != 0);
    s.chunk.floor
}

/// Paints the chunk last read into `volume`, one column a cell from the
/// floor. 0 when a band could not be made.
#[no_mangle]
pub extern "C" fn pxl_terrain_paint(volume: u32) -> u32 {
    let s = state();
    let v = s.volumes.as_mut().expect("pxl_volume_pools before any volume");
    s.chunk.paint(v, &mut s.pools, volume as usize - 1) as u32
}

/// The chunk last read, for the direct path: the addresses of its 4096
/// packed materials, layer bytes and elevations.
#[no_mangle]
pub extern "C" fn pxl_terrain_material_at() -> u32 {
    state().chunk.material.as_ptr() as u32
}

#[no_mangle]
pub extern "C" fn pxl_terrain_layer_at() -> u32 {
    state().chunk.layer_idx.as_ptr() as u32
}

#[no_mangle]
pub extern "C" fn pxl_terrain_ele_at() -> u32 {
    state().chunk.ele.as_ptr() as u32
}

// ---- Objects ----
//
// The client lowers a coordinate before it crosses: the chunk's key (an
// IVec, passed as it is), the band's Z, the band-local lattice position in
// finest-tier steps, and the level. What comes back is lanes to take views
// over, valid until the next write or reindex.

/// Writes one leaf; the entries are not re-derived until pxl_object_reindex.
#[no_mangle]
pub extern "C" fn pxl_object_write(key: u64, band_z: i32, x: u32, y: u32, z: u32, level: i32, value: u32) {
    state().objects.write(key, band_z, [x, y, z], level, value);
}

/// module/world/change.ts's WorldTopic numbers the store bumps, and the
/// log's shape.
pub const TOPIC_OBJECT_COLUMN: u16 = 0;
pub const TOPIC_DECOR_COLUMN: u16 = 1;
pub const TOPIC_OBJECT_CELL: u16 = 2;
pub const WORLD_TOPICS: usize = 6;
pub const JOURNAL_CAPACITY: usize = 4096;

/// A lattice word's X and Y lanes, signed: pvf3/ivec.ts's layout, X in the
/// top 24 bits, Y in the next 24, Z in the low 16.
fn lanes_xy(v: u64) -> (i64, i64) {
    ((v as i64) >> 40, ((v << 24) as i64) >> 40)
}

fn gather(x: i64, y: i64, z: i64) -> u64 {
    ((x as u64 & 0xff_ffff) << 40) | ((y as u64 & 0xff_ffff) << 16) | (z as u64 & 0xffff)
}

/// Re-derives the chunk's entries and cells, and records the change: the
/// column in both of its topics, and every cell whose entries changed, by
/// its corner.
#[no_mangle]
pub extern "C" fn pxl_object_reindex(key: u64) {
    let s = state();
    s.objects.reindex(key);
    s.changes.bump(TOPIC_OBJECT_COLUMN, key);
    s.changes.bump(TOPIC_DECOR_COLUMN, key);
    let (at, n) = s.objects.take_touched();
    let (x0, y0) = lanes_xy(key);
    for r in 0..n {
        // SAFETY: the rows take_touched just handed back, three words each.
        let row = unsafe { core::slice::from_raw_parts(at.add(r * 3), 3) };
        let corner = gather(x0 + row[0] as i64, y0 + row[1] as i64, row[2] as i64);
        s.changes.bump(TOPIC_OBJECT_CELL, corner);
    }
}

// ---- The change log ----

/// The log's lanes as a row of six words: the topic (u16), key (u64) and
/// revision (u32) lanes, the head word, the ticks (u32 a topic), and the
/// capacity.
#[no_mangle]
pub extern "C" fn pxl_change_lanes() -> *const u32 {
    let s = state();
    let l = s.changes.lanes();
    s.change_row = [l[0] as u32, l[1] as u32, l[2] as u32, l[3] as u32, l[4] as u32, s.changes.capacity() as u32];
    s.change_row.as_ptr()
}

#[no_mangle]
pub extern "C" fn pxl_change_bump(topic: u32, key: u64) -> u32 {
    state().changes.bump(topic as u16, key)
}

#[no_mangle]
pub extern "C" fn pxl_change_forget(topic: u32, key: u64) {
    state().changes.forget(topic as u16, key);
}

#[no_mangle]
pub extern "C" fn pxl_change_revision(topic: u32, key: u64) -> u32 {
    state().changes.revision_of(topic as u16, key)
}

#[no_mangle]
pub extern "C" fn pxl_object_read(key: u64, band_z: i32, x: u32, y: u32, z: u32) -> u32 {
    state().objects.read(key, band_z, [x, y, z])
}

/// The chunk's entry lanes as a row — count, then the addresses of the
/// band, position and state lanes — or 0 where there is no chunk.
#[no_mangle]
pub extern "C" fn pxl_object_chunk(key: u64) -> u32 {
    let s = state();
    match s.objects.chunk_lanes(key) {
        Some(lanes) => {
            s.object_row = [lanes.count, lanes.band_at as u32, lanes.pos_at as u32, lanes.state_at as u32];
            s.object_row.as_ptr() as u32
        }
        None => 0,
    }
}

/// The run of a chunk's entries in one cell as a row — start, count, how
/// many occupy it — or 0 where the cell holds none.
#[no_mangle]
pub extern "C" fn pxl_object_cell(key: u64, cx: u32, cy: u32, cz: i32) -> u32 {
    let s = state();
    match s.objects.cell(key, cx, cy, cz) {
        Some(rec) => {
            s.object_row = [rec.start, rec.count, rec.solid, 0];
            s.object_row.as_ptr() as u32
        }
        None => 0,
    }
}

#[no_mangle]
pub extern "C" fn pxl_object_chunks() -> u32 {
    state().objects.chunk_count() as u32
}

#[no_mangle]
pub extern "C" fn pxl_object_chunk_key(i: u32) -> u64 {
    state().objects.chunk_key(i as usize)
}

#[no_mangle]
pub extern "C" fn pxl_object_clear() {
    state().objects.clear();
}

/// Chunks, bands and cells held, as a row.
#[no_mangle]
pub extern "C" fn pxl_object_stat() -> u32 {
    let s = state();
    s.object_row = [s.objects.chunk_count() as u32, s.objects.band_count() as u32, s.objects.cell_count() as u32, 0];
    s.object_row.as_ptr() as u32
}

/// Teaches what a state paints as: 1 a solid of `material` (packed
/// 0xCC_RRGGBB) on `layer`, 2 air, 3 decor, which paints nothing.
#[no_mangle]
pub extern "C" fn pxl_object_kind(state_id: u32, kind: u32, material: u32, layer: u32) {
    let kind = match kind {
        1 => Kind::Solid,
        2 => Kind::Air,
        3 => Kind::Decor,
        _ => Kind::Untaught,
    };
    state().painter.teach(state_id, kind, material, layer as u8);
}

/// Paints the column `key` into `volume`, leaving out the entry at
/// (hide_band, hide_pos) when `hide` is set and adding the carve preview
/// at a corner relative to the column's base when `carve` is. Bit 0: a
/// voxel was painted; bit 1: a paint was refused.
#[no_mangle]
#[allow(clippy::too_many_arguments)]
pub extern "C" fn pxl_object_paint(volume: u32, key: u64, hide: u32, hide_band: i32, hide_pos: u32, carve: u32, lx: f64, ly: f64, lz: f64, size: f64) -> u32 {
    let s = state();
    let v = s.volumes.as_mut().expect("pxl_volume_pools before any volume");
    let hide = if hide != 0 { Some((hide_band, hide_pos)) } else { None };
    let carve = if carve != 0 { Some(Carve { lx, ly, lz, size }) } else { None };
    let made = s.painter.paint(&s.objects, v, &mut s.pools, volume as usize - 1, key, hide, carve);
    made.any as u32 | ((made.refused as u32) << 1)
}

// ---- The raycast's column ----

/// Walks column (cx, cy) of the module's volume `id` into lanes: how many
/// runs, at the addresses pxl_column_runs_at names — z0, z1 (i32), the
/// material class (u8) and the packed inset (u32) — valid until the next
/// walk.
#[no_mangle]
pub extern "C" fn pxl_column_runs(id: u32, cx: u32, cy: u32) -> u32 {
    let s = state();
    let v = s.volumes.as_mut().expect("pxl_volume_pools before any volume");
    let count = v.band_count(id as usize - 1);
    let mut rows: Vec<[u32; VOLUME_BAND_WORDS]> = Vec::with_capacity(count);
    for i in 0..count {
        rows.push(*v.band_row(id as usize - 1, i));
    }
    let n = s.column.walk(&band_views((rows.as_ptr() as usize, count as u32)), cx, cy);
    s.column_row = [
        s.column.z0.as_ptr() as u32,
        s.column.z1.as_ptr() as u32,
        s.column.material_class.as_ptr() as u32,
        s.column.sub.as_ptr() as u32,
    ];
    n as u32
}

#[no_mangle]
pub extern "C" fn pxl_column_runs_at() -> u32 {
    state().column_row.as_ptr() as u32
}

// ---- Rings ----
//
// Commands and their effects cross in rings (crates/pxl-core/src/ring.rs);
// a query that needs its answer now stays a call. The glue pushes into the
// submission ring, rings `pxl_drain` once a tick, and reads the completion
// ring when the call returns.

/// No effect; completes with its first argument, for the rings' own test.
pub const OP_NOP: u32 = 0;
/// Submissions: a chunk's life and what it is built from.
pub const OP_CHUNK_ADD: u32 = 1; // id, base lo, base hi, tx0, ty0, base x (i32), base y (i32), stable hash
pub const OP_CHUNK_REMOVE: u32 = 2; // id
pub const OP_CHUNK_SOURCE: u32 = 3; // id, has page, show structures, land, water, structure, transport, ele
/// The builder's own ask, bypassing the scheduler: for the tests that hold
/// a build to the TypeScript pipeline, never sent by the glue.
pub const OP_CHUNK_WANT: u32 = 4; // id, step, step (0 is none)
pub const OP_PAGE: u32 = 7; // page lo, page hi, present
pub const OP_EDITS: u32 = 6; // hide, key lo, key hi, band, pos, carve, key lo, key hi, lx, ly, lz, size (f32 bits)
/// Completions.
pub const OP_TIER: u32 = 16; // user id; generation, step, at, bytes, vertex count, index count
pub const OP_RUNS: u32 = 17; // user id; bytes
pub const OP_VISIBLE: u32 = 18; // user id; on
pub const OP_LAYOUT: u32 = 19; // user id; primary, secondary, ready
pub const OP_BLEND: u32 = 20; // user id; primary weight, secondary weight (f32 bits), decor takes the secondary
pub const OP_EVICT: u32 = 21; // user id; step
pub const OP_MOUNT: u32 = 22; // user id; page lo, page hi, base x, base y
pub const OP_UNMOUNT: u32 = 23; // user id
pub const OP_DECOR: u32 = 24; // user id; step, at, bytes

/// Doubles in the frame block, in the order sched::Frame names them: the
/// root matrix (16), the origin (3), the planes (24), the camera (3), the
/// lead (3), then viewport height, fov, delta seconds, budget, hard slack,
/// pixel dither and the retain budget, then the range anchor's x and y,
/// whether there is one, the render distance and the range hysteresis, and
/// whether decor is scattered.
pub const FRAME_WORDS: usize = 62;

#[no_mangle]
pub extern "C" fn pxl_ring_sq() -> u32 {
    state().sq.address() as u32
}

#[no_mangle]
pub extern "C" fn pxl_ring_cq() -> u32 {
    state().cq.address() as u32
}

#[no_mangle]
pub extern "C" fn pxl_frame_at() -> u32 {
    state().frame.as_ptr() as u32
}

fn key(lo: u32, hi: u32) -> u64 {
    lo as u64 | ((hi as u64) << 32)
}

fn completion(e: &Event) -> Entry {
    let mut done: Entry = [0; 16];
    let mut put = |id: u32, op: u32, args: &[u32]| {
        done[USER] = id;
        done[OP] = op;
        done[ARGS..ARGS + args.len()].copy_from_slice(args);
    };
    match *e {
        Event::Build(BuildEvent::Tier {
            id,
            generation,
            step,
            at,
            bytes,
            vertex_cnt,
            index_cnt,
        }) => put(id, OP_TIER, &[generation, step, at, bytes, vertex_cnt, index_cnt]),
        Event::Build(BuildEvent::Runs { id, bytes }) => put(id, OP_RUNS, &[bytes]),
        Event::Visible { id, on } => put(id, OP_VISIBLE, &[on as u32]),
        Event::Layout { id, primary, secondary, ready } => put(id, OP_LAYOUT, &[primary, secondary, ready as u32]),
        Event::Blend {
            id,
            primary,
            secondary,
            decor_secondary,
        } => put(id, OP_BLEND, &[primary.to_bits(), secondary.to_bits(), decor_secondary as u32]),
        Event::Evict { id, step } => put(id, OP_EVICT, &[step]),
        Event::Mount { id, page, base_x, base_y } => put(id, OP_MOUNT, &[page as u32, (page >> 32) as u32, base_x, base_y]),
        Event::Unmount { id } => put(id, OP_UNMOUNT, &[]),
        Event::Decor { id, step, at, bytes } => put(id, OP_DECOR, &[step, at, bytes]),
    }
    done
}

/// Moves what fits of the waiting events into the completion ring.
fn flush(s: &mut State) -> u32 {
    let mut posted = 0;
    while let Some(e) = s.events.front() {
        if !s.cq.push(&completion(e)) {
            break;
        }
        s.events.pop_front();
        posted += 1;
    }
    posted
}

fn now() -> f64 {
    // SAFETY: the import is the client's clock and reads nothing of ours.
    unsafe { pxl_now() }
}

/// Applies one submission; none for an op nobody numbers, which completes
/// with ENOSYS.
fn apply(s: &mut State, sub: &Entry) -> Option<u32> {
    let a = &sub[ARGS..];
    let volumes = s.volumes.as_mut().expect("pxl_volume_pools before any build");
    let mut world = World {
        volumes,
        pools: &mut s.pools,
        materials: &s.terrain,
        store: &s.objects,
        painter: &mut s.painter,
    };
    let events = &mut s.events;
    let mut emit = |e: Event| events.push_back(e);
    match sub[OP] & 0xffff {
        OP_NOP => return Some(a[0]),
        OP_CHUNK_ADD => s.sched.add(a[0], key(a[1], a[2]), a[5] as i32 as f64, a[6] as i32 as f64, a[3], a[4], a[7]),
        OP_CHUNK_REMOVE => s.sched.remove(&mut world, a[0], &mut emit),
        OP_CHUNK_SOURCE => {
            let page = (a[1] != 0).then(|| PageLanes {
                ground: [a[3] as usize, a[4] as usize, a[5] as usize, a[6] as usize],
                ele: a[7] as usize,
            });
            s.sched.source(&mut world, a[0], page, a[2] != 0, &mut emit);
        }
        OP_CHUNK_WANT => s.sched.builder.want(&mut world, a[0], [a[1], a[2]]),
        OP_PAGE => s.sched.page(key(a[0], a[1]), a[2] != 0),
        OP_EDITS => {
            let f = |w: u32| f32::from_bits(w) as f64;
            let hide = (a[0] != 0).then(|| (key(a[1], a[2]), a[3] as i32, a[4]));
            let carve = (a[5] != 0).then(|| {
                (
                    key(a[6], a[7]),
                    Carve {
                        lx: f(a[8]),
                        ly: f(a[9]),
                        lz: f(a[10]),
                        size: f(a[11]),
                    },
                )
            });
            s.sched.builder.set_edits(hide, carve);
        }
        _ => return None,
    }
    Some(0)
}

/// Applies every submission waiting, as far as the completion ring has room
/// for what they make: a submission left behind waits for the next call,
/// and nothing is dropped. Completes only what asks for an answer — the
/// no-op and an op nobody numbers — since a command's effect is its events.
fn drain(s: &mut State) -> u32 {
    let mut posted = flush(s);
    while !s.cq.is_full() && s.events.is_empty() {
        let Some(sub) = s.sq.peek() else { break };
        s.sq.advance();
        let op = sub[OP] & 0xffff;
        let result = apply(s, &sub);
        if op == OP_NOP || result.is_none() {
            let mut done: Entry = [0; 16];
            done[USER] = sub[USER];
            done[OP] = sub[OP];
            done[RESULT] = result.unwrap_or((-38i32) as u32); // ENOSYS
            s.cq.push(&done);
            posted += 1;
        }
        posted += flush(s);
    }
    posted
}

/// The doorbell without a tick: applies the submissions waiting.
#[no_mangle]
pub extern "C" fn pxl_drain() -> u32 {
    drain(state())
}

fn world_of(s: &mut State) -> (World<'_>, &mut std::collections::VecDeque<Event>, &mut Scheduler) {
    (
        World {
            volumes: s.volumes.as_mut().expect("pxl_volume_pools before any build"),
            pools: &mut s.pools,
            materials: &s.terrain,
            store: &s.objects,
            painter: &mut s.painter,
        },
        &mut s.events,
        &mut s.sched,
    )
}

fn after(s: &mut State) {
    let st = s.sched.builder.stat;
    let c = s.sched.builder.bake_context().counters;
    s.build_stat = [
        st.direct,
        st.scans,
        st.bakes,
        st.starved,
        c.total_runs,
        c.culled,
        c.faces_possible,
        c.faces_kept,
        c.coarse_total,
        c.coarse_low_fill,
        arena::used() as u32,
    ];
    let k = s.sched.census;
    s.census = [
        k.chunks as f64,
        k.in_frustum as f64,
        k.settling as f64,
        k.blend_active as f64,
        k.pending as f64,
        k.offscreen_pending as f64,
        k.oldest_visible_ms,
        k.oldest_release_ms,
        k.committed as f64,
        k.released as f64,
        k.retained_on,
        k.retained_off,
    ];
}

/// The frame's one call: applies the submissions waiting, then the
/// scheduler's pass over every chunk with the frame the glue wrote at
/// pxl_frame_at. Bit 0: the canvas has something new to draw. The glue reads
/// the completion ring when this returns.
#[no_mangle]
pub extern "C" fn pxl_tick() -> u32 {
    let s = state();
    drain(s);
    // Events the ring had no room for are posted first; a tick on top of
    // them would only queue more. The glue reads the ring dry and drains
    // again before the next tick, so this is a frame lost only when it
    // does not.
    if !s.events.is_empty() || s.volumes.is_none() {
        return 1;
    }
    let f = s.frame;
    let mut frame = Frame::default();
    frame.root.copy_from_slice(&f[0..16]);
    frame.origin.copy_from_slice(&f[16..19]);
    frame.planes.copy_from_slice(&f[19..43]);
    frame.camera.copy_from_slice(&f[43..46]);
    frame.lead.copy_from_slice(&f[46..49]);
    frame.viewport_h = f[49];
    frame.fov_deg = f[50];
    frame.delta_s = f[51];
    frame.budget_ms = f[52];
    frame.hard_slack_ms = f[53];
    frame.pixel_dither = f[54] != 0.0;
    frame.retain_budget_bytes = f[55];
    frame.anchor = (f[58] != 0.0).then_some([f[56], f[57]]);
    frame.render_dist = f[59];
    frame.range_hysteresis = f[60];
    frame.show_decor = f[61] != 0.0;
    let (mut world, events, sched) = world_of(s);
    let draw = sched.tick(&mut world, frame, &now, &mut |e| events.push_back(e));
    flush(s);
    after(s);
    draw as u32
}

/// Rebuilds chunk `id`'s layout to the end now, after the submissions
/// waiting are applied: what a placement shows the same frame by.
#[no_mangle]
pub extern "C" fn pxl_build_now(id: u32) {
    let s = state();
    drain(s);
    let (mut world, events, sched) = world_of(s);
    sched.build_now(&mut world, id, &now, &mut |e| events.push_back(e));
    flush(s);
    after(s);
}

/// The hover ghost as a box of `size` units with its min corner at (x, y,
/// z) in units, painted as object value `value` paints or, where that paints
/// nothing, as `rgb` (0xRRGGBB) untextured; `coplanar` when it sits in a
/// surface rather than clear of one.
#[no_mangle]
#[allow(clippy::too_many_arguments)]
pub extern "C" fn pxl_ghost_box(x: f64, y: f64, z: f64, size: f64, value: u32, rgb: u32, coplanar: u32) {
    let s = state();
    let (mut world, _, sched) = world_of(s);
    sched.ghost.set_box(&mut world, &mut sched.draw, [x, y, z], size, value, rgb, coplanar != 0);
}

/// The hover ghost as one instance of decor group `group` (every kind's
/// variants in teaching order), its base at (x, y, z) in units, turned
/// `rotation` about Z.
#[no_mangle]
pub extern "C" fn pxl_ghost_decor(x: f64, y: f64, z: f64, group: u32, rotation: f64) {
    let s = state();
    let (mut world, _, sched) = world_of(s);
    sched.ghost.set_decor(&mut world, &mut sched.draw, [x, y, z], group, rotation as f32);
}

#[no_mangle]
pub extern "C" fn pxl_ghost_clear() {
    let s = state();
    let (mut world, _, sched) = world_of(s);
    sched.ghost.clear(&mut world, &mut sched.draw);
}

/// Hands the renderer's lists over (crates/pxl-core/src/draw.rs): a row of
/// eight words — the uploads' address and count, the frees', the draws',
/// the uniforms' — valid until the next call. Uploads and frees are what
/// accumulated since the last call; the draws are the last tick's.
#[no_mangle]
pub extern "C" fn pxl_draw_take() -> *const u32 {
    let s = state();
    let lanes = s.sched.draw.take();
    for (k, v) in lanes.iter().enumerate() {
        s.draw_row[k] = *v as u32;
    }
    s.draw_row.as_ptr()
}

/// Rebuilds chunk `id`'s decor for the layout it shows, now, after the
/// submissions waiting are applied; `show_decor` is the setting as it is.
#[no_mangle]
pub extern "C" fn pxl_decor_now(id: u32, show_decor: u32) {
    let s = state();
    drain(s);
    let (mut world, events, sched) = world_of(s);
    sched.decor_now(&mut world, id, show_decor != 0, &mut |e| events.push_back(e));
    flush(s);
    after(s);
}

/// Forgets every decor kind taught, so the glue can teach the list again
/// (a module of it re-evaluated by the dev server) without the groups
/// doubling.
#[no_mangle]
pub extern "C" fn pxl_decor_forget() {
    state().sched.decor_forget();
}

/// Teaches a decor kind, in group order: its tag, density, variant count,
/// whether it aligns to transport, its extent in units, and the object
/// store's value a placed one carries (-1 for none). Its index.
#[no_mangle]
pub extern "C" fn pxl_decor_kind(tag_lo: u32, tag_hi: u32, density: f64, variant_count: u32, align_transport: u32, extent: f64, decor_value: i32) -> u32 {
    state().sched.decor_kind(DecorKind {
        tag: key(tag_lo, tag_hi),
        density,
        variant_count,
        align_transport: align_transport != 0,
        extent,
        decor_value: (decor_value >= 0).then_some(decor_value as u32),
        rules: [DecorRule::Any; 4],
    })
}

/// A kind's rule on a channel (land 0, water 1, structure 2, transport 3):
/// 0 any type, 1 void only, 2 one of the types `pxl_decor_allow` adds.
#[no_mangle]
pub extern "C" fn pxl_decor_rule(kind: u32, channel: u32, mode: u32) {
    let rule = match mode {
        0 => DecorRule::Any,
        1 => DecorRule::Void,
        _ => DecorRule::OneOf([0; 4]),
    };
    state().sched.decor_rule(kind, channel as usize & 3, rule);
}

#[no_mangle]
pub extern "C" fn pxl_decor_allow(kind: u32, channel: u32, t: u32) {
    state().sched.decor_allow(kind, channel as usize & 3, t as u8);
}

/// Doubles at pxl_raycast_at after a hit: the material class, the face
/// normal, the point, t, the box's min and max — 14.
pub const HIT_WORDS: usize = 14;

/// The world's raycast (crates/pxl-core/src/raycast.rs): the ray from
/// (ox, oy, oz) along (dx, dy, dz), relative to the map origin at
/// (origin_x, origin_y) in units, within `max_dist`. 1 when something was
/// struck, the hit written at pxl_raycast_at. Applies the submissions
/// waiting first, so a placement just made is what the ray tests.
#[no_mangle]
#[allow(clippy::too_many_arguments)]
pub extern "C" fn pxl_raycast(origin_x: f64, origin_y: f64, ox: f64, oy: f64, oz: f64, dx: f64, dy: f64, dz: f64, max_dist: f64) -> u32 {
    let s = state();
    drain(s);
    let hit = {
        let (mut world, raycast, sched) = raycast_of(s);
        raycast.hit(&mut world, sched, [origin_x, origin_y], [ox, oy, oz], [dx, dy, dz], max_dist)
    };
    let Some(h) = hit else { return 0 };
    s.hit = [
        h.material_class as f64,
        h.normal[0],
        h.normal[1],
        h.normal[2],
        h.point[0],
        h.point[1],
        h.point[2],
        h.t,
        h.box_min[0],
        h.box_min[1],
        h.box_min[2],
        h.box_max[0],
        h.box_max[1],
        h.box_max[2],
    ];
    1
}

#[no_mangle]
pub extern "C" fn pxl_raycast_at() -> u32 {
    state().hit.as_ptr() as u32
}

fn raycast_of(s: &mut State) -> (World<'_>, &mut Raycast, &mut Scheduler) {
    (
        World {
            volumes: s.volumes.as_mut().expect("pxl_volume_pools before any build"),
            pools: &mut s.pools,
            materials: &s.terrain,
            store: &s.objects,
            painter: &mut s.painter,
        },
        &mut s.raycast,
        &mut s.sched,
    )
}

/// The builder alone over the `order_len` ids at `order_at`, for the tests
/// that hold a build to the TypeScript pipeline: no scheduler.
#[no_mangle]
pub extern "C" fn pxl_build_tick(order_at: u32, order_len: u32, budget_ms: f64) {
    let s = state();
    drain(s);
    // SAFETY: the caller's lane of ids.
    let order: Vec<u32> = unsafe { core::slice::from_raw_parts(order_at as usize as *const u32, order_len as usize) }.to_vec();
    let (mut world, events, sched) = world_of(s);
    sched.builder.tick(&mut world, &order, budget_ms, &now, &mut |e| events.push_back(Event::Build(e)));
    flush(s);
    after(s);
}

/// Runs made the direct way, scans, bakes and refused blocks so far, the
/// bakes' six counters (pxl_bake_finish's), and the module heap's use, as a
/// row of eleven.
#[no_mangle]
pub extern "C" fn pxl_build_stat() -> *const u32 {
    state().build_stat.as_ptr()
}

/// The scheduler's census as twelve doubles: chunks, in the frustum,
/// settling, blending, pending, pending off screen, the oldest visible and
/// release waits in ms, committed this tick, released so far, and the bytes
/// retained on and off screen.
#[no_mangle]
pub extern "C" fn pxl_census() -> *const f64 {
    state().census.as_ptr()
}
