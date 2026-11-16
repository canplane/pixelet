// pxl-runtime/lib.rs

//! pixelet's library on a device: the core (pxl-core), linked in as a page
//! instantiates its module, and what xpute's runtime starts it with, as
//! ts/client/src/runtime/worker.ts is a page's. `start` lists the mounts of
//! what the host implements, the stop, the clocks, the text and the GPU, and
//! the runtime runs them, then mounts what the UI implements, enters the core
//! once every import of its world is implemented, and runs the loop for the
//! core's life (xpute-runtime-native native.rs `Boot`). The text is pixelet's
//! own, implemented on the host's side as ts/client/src/service implements it
//! on a page (text.rs); the UI implements the files, the settings among them,
//! and the objects, which a page's worker implements. Beside them is the
//! decoder the app's fetch calls (unpack.rs).

#![cfg(not(target_arch = "wasm32"))]

/// The bindings of pixelet's interfaces on the host's side, generated from
/// spec/client/core/wit/.
pub mod abi;
mod text;
mod unpack;

use std::path::PathBuf;

use pxl_core::spec::manifest::stack;
use pxl_core::sys::heap::Heap;
use pxl_gpu_host::{Ask, native};
use pxl_gpu_link::{GpuAsk, GpuConfig};
use pxl_render::spec::api::device;
use xpute_host::{clock, stop};
use xpute_runtime_native::native::Boot;

// The GPU's host under its C names, which the GPU's module calls (pxl_gpu.h,
// and on Android the Kotlin class `me.plei.pixelet.gpu.Gpu`, through the
// runtime's JNI helpers).
pxl_gpu_host::native_abi!(::xpute_runtime_native::jni);

// The runtime under xpute's names over `start`, which xpute's module calls
// (xpute.h, and on Android the Kotlin class `xpute.Runtime`).
xpute_runtime_native::native_abi!(start);

// The core's heap, the library's global allocator (pxl-core sys/heap.rs):
// pixelet's memory policy, not xpute's. It takes this whole library's
// allocations, not the core's alone as on a page (pxl-core sys/README.md
// "The memory, laid out once").
#[global_allocator]
static HEAP: Heap = Heap;

/// How a device's GPU host paces the renderer's stream, as
/// spec/client/render/api.json5 `device` sets it.
const GPU: GpuConfig = GpuConfig {
    frame_latency: device::FRAME_LATENCY,
    upload_chunk_sz: device::UPLOAD_CHUNK_SZ,
};

/// What the runtime starts the core with: `cache_dir` is the directory the
/// platform keeps the app's caches in.
fn start(cache_dir: PathBuf) -> Boot {
    Boot {
        stack_sz: stack::THREAD_SZ as usize,
        mounts: vec![
            Box::new(stop::mount),
            Box::new(clock::mount),
            Box::new(text::mount),
            Box::new(move |side| {
                let ask = GpuAsk {
                    power: device::POWER,
                    required_feats: device::REQUIRED_FEATURES,
                    optional_feats: device::OPTIONAL_FEATURES,
                };
                native::mount(side, Ask::of(ask), native::config(GPU), Some(cache_dir));
            }),
        ],
        imports: abi::IMPORTS,
        calls: abi::CALLS,
    }
}
