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
use pxl_core::build::{Builder, Event, PageLanes, World};
use pxl_core::mesher::{BakeContext, Step};
use pxl_core::object::{Carve, Kind, Painter, Store};
use pxl_core::ring::{ring_words, Entry, Ring, ARGS, OP, RESULT, USER};
use pxl_core::run_set::{run_set_bytes, RunSet};
use pxl_core::scan::{BandView, ColumnRuns, ExtraView, ScanContext, ScanStep};
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
    builder: Builder,
    events: std::collections::VecDeque<Event>,
    build_stat: [u32; 11],
}

/// Entries each ring holds.
pub const RING_CAPACITY: u32 = 1024;

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
                builder: Builder::new(),
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

/// Re-derives the chunk's entries and cells; the cells that changed are
/// then pxl_object_touched's.
#[no_mangle]
pub extern "C" fn pxl_object_reindex(key: u64) {
    state().objects.reindex(key);
}

/// Takes the cells touched since the last take: how many rows of
/// (cx, cy, absolute cell z) are at pxl_object_touched_at.
#[no_mangle]
pub extern "C" fn pxl_object_touched() -> u32 {
    let s = state();
    let (at, n) = s.objects.take_touched();
    s.object_row[0] = at as u32;
    n as u32
}

#[no_mangle]
pub extern "C" fn pxl_object_touched_at() -> u32 {
    state().object_row[0]
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
pub const OP_CHUNK_ADD: u32 = 1; // id, base lo, base hi, tx0, ty0
pub const OP_CHUNK_REMOVE: u32 = 2; // id
pub const OP_CHUNK_SOURCE: u32 = 3; // id, has page, show structures, land, water, structure, transport, ele
pub const OP_CHUNK_WANT: u32 = 4; // id, step, step (0 is none)
pub const OP_CHUNK_RECLAIM: u32 = 5; // id
pub const OP_EDITS: u32 = 6; // hide, key lo, key hi, band, pos, carve, key lo, key hi, lx, ly, lz, size (f32 bits)
/// Completions: a tier's mesh, and what a chunk's runs hold.
pub const OP_TIER: u32 = 16; // user id; generation, step, at, bytes, vertex count, index count
pub const OP_RUNS: u32 = 17; // user id; bytes

#[no_mangle]
pub extern "C" fn pxl_ring_sq() -> u32 {
    state().sq.address() as u32
}

#[no_mangle]
pub extern "C" fn pxl_ring_cq() -> u32 {
    state().cq.address() as u32
}

fn key(lo: u32, hi: u32) -> u64 {
    lo as u64 | ((hi as u64) << 32)
}

fn completion(e: &Event) -> Entry {
    let mut done: Entry = [0; 16];
    match *e {
        Event::Tier {
            id,
            generation,
            step,
            at,
            bytes,
            vertex_cnt,
            index_cnt,
        } => {
            done[USER] = id;
            done[OP] = OP_TIER;
            done[ARGS..ARGS + 6].copy_from_slice(&[generation, step, at, bytes, vertex_cnt, index_cnt]);
        }
        Event::Runs { id, bytes } => {
            done[USER] = id;
            done[OP] = OP_RUNS;
            done[ARGS] = bytes;
        }
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

/// Applies one submission; false for an op nobody numbers, which completes
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
        OP_CHUNK_ADD => s.builder.add(a[0], key(a[1], a[2]), a[3], a[4]),
        OP_CHUNK_REMOVE => s.builder.remove(&mut world, a[0], &mut emit),
        OP_CHUNK_SOURCE => {
            let page = (a[1] != 0).then(|| PageLanes {
                ground: [a[3] as usize, a[4] as usize, a[5] as usize, a[6] as usize],
                ele: a[7] as usize,
            });
            s.builder.source(&mut world, a[0], page, a[2] != 0, &mut emit);
        }
        OP_CHUNK_WANT => s.builder.want(&mut world, a[0], [a[1], a[2]]),
        OP_CHUNK_RECLAIM => s.builder.reclaim(&mut world, a[0], &mut emit),
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
            s.builder.set_edits(hide, carve);
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

/// The frame's one call: applies the submissions waiting, then advances the
/// builds of the `order_len` chunk ids at `order_at` — in that order, the
/// slots' holders first — for `budget_ms`, and returns how many completions
/// it posted. The glue reads the completion ring when this returns.
#[no_mangle]
pub extern "C" fn pxl_tick(order_at: u32, order_len: u32, budget_ms: f64) -> u32 {
    let s = state();
    let mut posted = drain(s);
    if !s.events.is_empty() || s.volumes.is_none() {
        return posted;
    }
    // SAFETY: the glue's lane of ids, read before anything else can write it.
    let order: Vec<u32> = unsafe { core::slice::from_raw_parts(order_at as usize as *const u32, order_len as usize) }.to_vec();
    let mut world = World {
        volumes: s.volumes.as_mut().unwrap(),
        pools: &mut s.pools,
        materials: &s.terrain,
        store: &s.objects,
        painter: &mut s.painter,
    };
    let events = &mut s.events;
    // SAFETY: the import is the client's clock and reads nothing of ours.
    s.builder.tick(&mut world, &order, budget_ms, &|| unsafe { pxl_now() }, &mut |e| events.push_back(e));
    posted += flush(s);
    let st = s.builder.stat;
    let c = s.builder.bake_context().counters;
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
    posted
}

/// Runs made the direct way, scans, bakes and refused blocks so far, the
/// bakes' six counters (pxl_bake_finish's), and the module heap's use, as a
/// row of eleven.
#[no_mangle]
pub extern "C" fn pxl_build_stat() -> *const u32 {
    state().build_stat.as_ptr()
}
