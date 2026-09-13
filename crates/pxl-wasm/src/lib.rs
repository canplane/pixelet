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
use pxl_core::mesher::{BakeContext, Step};
use pxl_core::run_set::{run_set_bytes, RunSet};
use pxl_core::scan::{BandView, ExtraView, ScanContext, ScanStep};
use pxl_core::terrain::{ChunkSample, Materials, PageView, CHUNK_SPAN, ELE_PADDED_SPAN, PAGE_CELL_SPAN};
use pxl_core::volume::{Cell, Volumes, Voxel, BAND_WORDS as VOLUME_BAND_WORDS};

#[global_allocator]
static ALLOC: arena::Arena = arena::Arena;

/// The most a mesh or a set may be for the module to stage it in its own
/// heap when the block the client acquired is out of reach (the mesh
/// section spilled past its range): the client copies it out afterwards.
/// A phone's largest mesh block was 2 MB; past this the bake goes the
/// TypeScript way rather than crowd the heap.
pub const STAGE_BYTES: u32 = 8 << 20;

struct State {
    ctx: BakeContext,
    /// Where a staged mesh or set is written, kept at its high-water mark.
    stage: Vec<u32>,
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
}

static mut STATE: Option<State> = None;

extern "C" {
    /// The client's: told a panic's message before the trap, since the trap
    /// itself carries none — `panic = "abort"` and no hook would leave only
    /// "unreachable" in the console.
    fn pxl_panic(ptr: *const u8, len: usize);
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
                stage: Vec::new(),
                input: (0, 0, 0),
                result: [0; 9],
                scan: ScanContext::new(),
                bands: (0, 0),
                pools: Vec::new(),
                volumes: None,
                scan_rows: Vec::new(),
                terrain: Materials::default(),
                chunk: ChunkSample::default(),
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

fn stage(words: u32) -> u32 {
    if words * 4 > STAGE_BYTES {
        return 0;
    }
    let s = state();
    if s.stage.len() < words as usize {
        s.stage.resize(words as usize, 0);
    }
    s.stage.as_mut_ptr() as u32
}

/// The mesh goes into the module's own heap instead, for the client to
/// copy out after `pxl_bake_finish`: returns where, or 0 when the mesh is
/// too large to stage.
#[no_mangle]
pub extern "C" fn pxl_bake_mesh_staged(words: u32) -> u32 {
    let at = stage(words);
    if at != 0 {
        // SAFETY: the stage is this module's, sized for `words`, and no other
        // export touches it until the bake closes.
        unsafe { state().ctx.provide_mesh(at as *mut u32, words) }
    }
    at
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

/// The set is packed into the module's own heap instead, for the client to
/// copy out: returns where, or 0 when the set is too large to stage.
#[no_mangle]
pub extern "C" fn pxl_scan_set_staged(bytes: u32) -> u32 {
    let at = stage(bytes.div_ceil(4));
    if at != 0 {
        // SAFETY: as pxl_bake_mesh_staged.
        unsafe { state().scan.provide_set(at as *mut u8, bytes) }
    }
    at
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
