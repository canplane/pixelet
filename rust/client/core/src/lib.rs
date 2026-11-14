// pxl-core/lib.rs

//! pixelet's core holds the world's state and runs everything that works on
//! it. On the page it is a WebAssembly module, and the browser draws its
//! frames. On a device it is a library the app links, and the GPU's host
//! runs beside it and admits each frame. It runs only when xpute's link enters
//! it (`xpute_guest::link!`): its boot and every entry's start and end are
//! link.rs's. No
//! other file of the core knows about WebAssembly, the browser or threads. The only function a device's app calls outside xpute's names is
//! the transport's decoder (sys/unpack.rs, include/pxl_native.h).

/// The link's bindings, generated from spec/client/core/wit/.
pub mod abi;
pub(crate) mod domain;
pub(crate) mod frame;
pub mod link;
pub(crate) mod perf;
pub(crate) mod pointer;
pub mod spec;
pub mod sys;
pub(crate) mod ui;

use spec::perf::limit;
use pxl_render::spec::api::device;
use xpute_gpu_guest::admit;
use xpute_gpu_link::GpuConfig;
use xpute_link::frame::WORD_BYTES;

/// Whether this is a build made for measuring: `PXL_PERF_DRIVE=1` where it is
/// compiled, read as the page's build reads it (ts/client babel.config.ts).
pub const MEASURING: bool = match option_env!("PXL_PERF_DRIVE") {
    Some(v) => matches!(v.as_bytes(), b"1"),
    None => false,
};

/// How a device's GPU host paces the renderer's stream, as
/// spec/client/render/api.json `device` sets it. A build made for measuring
/// has it, and the host that runs the core's entries, log a record, a run or
/// an entry slower than perf.json's `limit::SLOW_MS`, as the page's worker has
/// the page's hosts log.
const GPU: GpuConfig = GpuConfig {
    frame_latency: device::FRAME_LATENCY,
    upload_chunk_bytes: device::UPLOAD_CHUNK_BYTES as u64,
    slow_ms: if MEASURING {
        Some(limit::SLOW_MS)
    } else {
        None
    },
};

/// Whether the GPU's host takes a frame now without blocking (xpute-gpu-guest
/// `admit::frame`), by the device's block and the frame latency.
pub(crate) fn admit_frame() -> bool {
    admit::frame::<{ GPU.frame_latency }>()
}

// A frame's room holds its words and the largest packet the core lays
// (link/bound.rs).
xpute_guest::link!(
    link::bound::PKT_CAP + WORD_BYTES,
    link::boot,
    link::enter,
    link::end
);
