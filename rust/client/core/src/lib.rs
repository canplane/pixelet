// pxl-core/lib.rs

//! pixelet's core holds the world's state and runs everything that works on
//! it. `xpute_core::core!` builds it in the form its target takes. On the
//! page it is a WebAssembly module, and the browser draws its frames. On a
//! device it is a library the app links, and xpute's GPU runtime runs beside
//! it and admits each frame. Every turn enters the core through door.rs
//! `run`. No other file of the core knows about WebAssembly, the browser or
//! threads. The only function a device's app calls outside xpute's names is
//! the transport's decoder (unpack.rs, include/pxl_native.h).

pub(crate) mod domain;
pub mod door;
#[path = "machine.spec.rs"]
pub mod machine;
pub(crate) mod perf;
pub(crate) mod pointer;
pub mod sys;
pub(crate) mod turn;
pub(crate) mod ui;
#[cfg(not(target_arch = "wasm32"))]
mod unpack;

use door::layout::FIXED_BYTES;
use machine::MACHINE;
use pxl_render::api_spec::device;
use xpute_gpu::Numbers;

/// How a device's GPU runtime paces the renderer's stream, as
/// spec/client/render/gpu/api.json `device` sets it.
const GPU: Numbers = Numbers {
    frame_latency: device::FRAME_LATENCY,
    upload_chunk_bytes: device::UPLOAD_CHUNK_BYTES as u64,
};

// The heap starts past the core's fixed ranges (door/layout.rs). The first
// turn opens the front the runtime laid out where the machine places it.
xpute_core::core!(MACHINE, FIXED_BYTES, door::run, gpu: GPU);
