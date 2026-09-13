//! The one WebAssembly module: pxl-core behind numeric exports.
//!
//! No wasm-bindgen. What crosses is integers and addresses into this
//! module's memory — the caller writes a run set's bytes where it is told,
//! steps the bake, and reads the mesh's words back out — so the boundary is
//! the same shape the command frame already is, and a thread that
//! instantiates the module needs nothing but the module.
//!
//! This piece keeps its own memory and copies in and out. Importing the core
//! memory instead is the move's next piece, and has a layout question of its
//! own: the module's data and stack sit at address 0, where the fixed
//! section begins today.
//!
//! One bake is open at a time, as with the TypeScript context; the module is
//! single-threaded, so the state is a plain global.

use pxl_core::mesher::BakeContext;
use pxl_core::run_set::{run_set_bytes, RunSet};

struct State {
    ctx: BakeContext,
    /// The run set's bytes, as words so the start is 4-aligned.
    input: Vec<u32>,
    count: u32,
    span: u32,
    /// vertex_cnt, index_cnt, the mesh's address, then the counters.
    result: [u32; 9],
}

static mut STATE: Option<State> = None;

#[allow(static_mut_refs)]
fn state() -> &'static mut State {
    // SAFETY: the module runs on one thread and no export re-enters another.
    unsafe {
        STATE.get_or_insert_with(|| State {
            ctx: BakeContext::new(),
            input: Vec::new(),
            count: 0,
            span: 0,
            result: [0; 9],
        })
    }
}

fn input_view(input: &[u32], count: u32, span: u32) -> RunSet<'_> {
    let bytes = run_set_bytes(count, span) as usize;
    // SAFETY: `input` is words, so its start is 4-aligned, and
    // pxl_bake_input sized it for these bytes.
    unsafe {
        let raw = core::slice::from_raw_parts(input.as_ptr() as *const u8, bytes);
        RunSet::carve(raw, count, span)
    }
}

/// Where to write the bytes of a run set of `count` runs over `span`²
/// columns, in column_run.ts's layout. Valid until the next call into the
/// module.
#[no_mangle]
pub extern "C" fn pxl_bake_input(count: u32, span: u32) -> *mut u8 {
    let s = state();
    assert!(!s.ctx.is_open(), "bake input replaced while a bake is open");
    s.count = count;
    s.span = span;
    s.input.clear();
    s.input.resize(run_set_bytes(count, span).div_ceil(4) as usize, 0);
    s.input.as_mut_ptr() as *mut u8
}

/// Opens a bake of the input at `lod_step`. 0 when one is already open.
#[no_mangle]
pub extern "C" fn pxl_bake_begin(lod_step: u32) -> u32 {
    let State { ctx, input, count, span, .. } = state();
    ctx.begin(&input_view(input, *count, *span), lod_step) as u32
}

/// One step of the open bake. 1 once the mesh is complete.
#[no_mangle]
pub extern "C" fn pxl_bake_step() -> u32 {
    let State { ctx, input, count, span, .. } = state();
    ctx.step(&input_view(input, *count, *span)) as u32
}

/// Closes the complete bake and returns the address of nine words: vertex
/// count, index count, the mesh words' address, and the six counters so far.
/// The mesh stays valid until the next bake begins.
#[no_mangle]
pub extern "C" fn pxl_bake_finish() -> *const u32 {
    let s = state();
    let mesh = s.ctx.finish();
    let (vertex_cnt, index_cnt, words) = (mesh.vertex_cnt, mesh.index_cnt, mesh.words.as_ptr() as u32);
    let c = s.ctx.counters;
    s.result = [vertex_cnt, index_cnt, words, c.total_runs, c.culled, c.faces_possible, c.faces_kept, c.coarse_total, c.coarse_low_fill];
    s.result.as_ptr()
}

#[no_mangle]
pub extern "C" fn pxl_bake_abort() {
    state().ctx.abort();
}
