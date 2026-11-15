// pxl-runtime/lib.rs

//! pixelet's library on a device: the core (pxl-core), linked in as a page
//! instantiates its module, and what xpute's runtime starts it with, as
//! ts/client/src/runtime/worker.ts is a page's. `start` lists the servers of
//! what the host serves, the stop, the clocks, the text and the GPU, and the
//! runtime mounts them, then what the UI serves, enters the core once every
//! import of its world is served, and runs the loop for the core's life
//! (xpute-native-runtime native.rs `Boot`). The text is pixelet's own, served
//! on the host's side as ts/client/src/service serves it on a page (text.rs);
//! the UI serves the files, the settings among them, and the objects over the
//! remote, as the page's worker does. Beside them is the decoder the app's
//! fetch calls (unpack.rs).

#![cfg(not(target_arch = "wasm32"))]

/// The bindings of pixelet's interfaces on the host's side, generated from
/// spec/client/core/wit/.
pub mod abi;
mod text;
mod unpack;

use std::path::PathBuf;

use pxl_core::spec::manifest::stack;
use pxl_core::sys::heap::Heap;
use pxl_render::spec::api::device;
use xpute_gpu_link::{GpuAsk, GpuConfig};
use xpute_gpu_native_host::{native, Ask};
use xpute_native_host::{clock, stop};
use xpute_native_runtime::native::Boot;

// The GPU's host under xpute's names, which the platform's GPU module calls
// (xpute_gpu.h, and on Android the Kotlin class `xpute.Gpu`, through the
// runtime's JNI helpers).
xpute_gpu_native_host::native_abi!(::xpute_native_runtime::jni);

// The runtime under xpute's names over `start`, which xpute's module calls
// (xpute.h, and on Android the Kotlin class `xpute.Runtime`).
xpute_native_runtime::native_abi!(start);

// The core's heap, the library's global allocator (pxl-core sys/heap.rs).
#[global_allocator]
static HEAP: Heap = Heap;

/// How a device's GPU host paces the renderer's stream, as
/// spec/client/render/api.json5 `device` sets it.
const GPU: GpuConfig = GpuConfig {
    frame_latency: device::FRAME_LATENCY,
    upload_chunk_sz: device::UPLOAD_CHUNK_BYTES,
};

/// What the runtime starts the core with: `cache_dir` is the directory the
/// platform keeps the app's caches in.
fn start(cache_dir: PathBuf) -> Boot {
    Boot {
        stack_sz: stack::THREAD_SZ as usize,
        servers: vec![
            Box::new(stop::serve),
            Box::new(clock::serve),
            Box::new(text::serve),
            Box::new(move |side| {
                let ask = GpuAsk {
                    power: device::POWER,
                    required_feats: device::REQUIRED_FEATURES,
                    optional_feats: device::OPTIONAL_FEATURES,
                };
                native::serve(side, Ask::of(ask), native::config(GPU), Some(cache_dir));
            }),
        ],
        imports: abi::IMPORTS,
        later: abi::LATER,
    }
}
