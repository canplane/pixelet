// pxl-kernel/lib.rs

//! pixelet's kernel: the world's state and what runs over it, and the
//! artifact it is built as. Its form is xpute's for the target
//! (`xpute_guest::guest!`): a WebAssembly module a page instantiates, whose
//! browser draws its frames, or a library a device's app links, with xpute's
//! GPU host around each turn and its admission handed the kernel's entry
//! (door.rs `run`). Nothing else here knows of WebAssembly, a browser or
//! threads. What a device's app calls besides xpute's names is the
//! transport's decoder (unpack.rs, include/pxl_native.h).

pub(crate) mod domain;
pub mod door;
#[path = "machine.spec.rs"]
pub mod machine;
pub(crate) mod perf;
pub(crate) mod pointer;
pub(crate) mod shell;
pub mod sys;
pub(crate) mod turn;
#[cfg(not(target_arch = "wasm32"))]
mod unpack;

use door::layout::FIXED_BYTES;
use machine::MACHINE;
use pxl_render::api_spec::device;
use xpute_gpu::Numbers;

/// What a device's GPU host runs the renderer's stream by
/// (spec/client/render/gpu/api.json `device`).
const GPU: Numbers = Numbers {
    frame_latency: device::FRAME_LATENCY,
    upload_chunk_bytes: device::UPLOAD_CHUNK_BYTES as u64,
};

// The kernel as its machine's guest: the front its host laid, as the machine
// places it, opened on the first turn, the heap past `.fixed`
// (door/layout.rs), and its door's turn as the entry.
xpute_guest::guest!(MACHINE, FIXED_BYTES, door::run, gpu: GPU);
