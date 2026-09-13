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

#[global_allocator]
static ALLOC: arena::Arena = arena::Arena;

struct State {
    ctx: BakeContext,
    /// The open bake's run set: its address in the memory, and its shape.
    input: (usize, u32, u32),
    /// vertex_cnt, index_cnt, the six counters, then the arena's use.
    result: [u32; 9],
}

static mut STATE: Option<State> = None;

#[allow(static_mut_refs)]
fn state() -> &'static mut State {
    // SAFETY: the module runs on one thread and no export re-enters another.
    unsafe {
        STATE.get_or_insert_with(|| State {
            ctx: BakeContext::new(),
            input: (0, 0, 0),
            result: [0; 9],
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

/// The section the module keeps for itself at the front of the memory, for
/// the client to check its map against.
#[no_mangle]
pub extern "C" fn pxl_module_bytes() -> u32 {
    arena::MODULE_BYTES as u32
}
