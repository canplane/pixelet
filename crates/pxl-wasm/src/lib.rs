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

use pxl_core::mesher::{BakeContext, Step};
use pxl_core::run_set::{run_set_bytes, RunSet};
use pxl_core::scan::{BandView, ExtraView, ScanContext, ScanStep};

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
/// writes the same): band_z; the addresses of the material, layer and
/// sub-cell lanes (0 for no sub-cells); the extras' count and the addresses
/// of their five lanes.
pub const BAND_WORDS: usize = 10;

fn band_views((at, count): (usize, u32)) -> Vec<BandView<'static>> {
    // SAFETY: the client writes the table into a lane of its own and every
    // address in it names a lane of a band it keeps until the scan closes;
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
