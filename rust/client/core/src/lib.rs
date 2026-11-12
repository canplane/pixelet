// pxl-core/lib.rs

//! pixelet's core holds the world's state and runs everything that works on
//! it. On the page it is a WebAssembly module, and the browser draws its
//! frames. On a device it is a library the app links, and xpute's GPU runtime
//! runs beside it and admits each frame. It runs only when xpute's link enters
//! it (`xpute_core::link!`): its boot and every entry's start and end are
//! door.rs's. No
//! other file of the core knows about WebAssembly, the browser or threads. The only function a device's app calls outside xpute's names is
//! the transport's decoder (sys/unpack.rs, include/pxl_native.h).

/// The link's bindings, generated from spec/client/core/wit/.
pub mod abi;
pub(crate) mod domain;
pub mod door;
pub(crate) mod perf;
pub(crate) mod pointer;
pub mod sys;
pub(crate) mod turn;
pub(crate) mod ui;

use door::layout::FIXED_BYTES;
use perf::spec::LIMIT;
use pxl_render::api_spec::device;
use sys::manifest::{GPU_ASK, MANIFEST};
use xpute_gpu::CoreConfig;

/// Whether this is a build made for measuring: `PXL_PERF_DRIVE=1` where it is
/// compiled, read as the page's build reads it (ts/client babel.config.ts).
pub const MEASURING: bool = match option_env!("PXL_PERF_DRIVE") {
    Some(v) => matches!(v.as_bytes(), b"1"),
    None => false,
};

/// How a device's GPU runtime paces the renderer's stream, as
/// spec/client/render/api.json `device` sets it. A build made for measuring
/// has it, and the runtime that runs the core's turns, say a record, a run or
/// a turn slower than perf.json's `LIMIT::SLOW_MS`, as the page's worker and
/// its runtime's config have the page's runtimes say.
const GPU: CoreConfig = CoreConfig {
    frame_latency: device::FRAME_LATENCY,
    upload_chunk_bytes: device::UPLOAD_CHUNK_BYTES as u64,
    slow_ms: if MEASURING {
        Some(LIMIT::SLOW_MS)
    } else {
        None
    },
};

// A frame's room holds its words and the manifest's packet.
xpute_core::link!(
    MANIFEST.pkt_bytes + xpute_core::io::frame::WORD_BYTES,
    door::boot,
    door::enter,
    door::end
);

