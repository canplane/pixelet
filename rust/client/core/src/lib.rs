// pxl-core/lib.rs

//! pixelet's core holds the world's state and runs everything that works on
//! it. On the page it is a WebAssembly module, and the browser draws its
//! frames. On a device it is linked into the app with what xpute's runtime
//! starts it with (pxl-runtime), and the GPU's host runs beside it and admits
//! each frame. It runs only when xpute's link enters it
//! (`xpute_guest::link!`): its boot and every entry's start and end are
//! link.rs's.

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

use xpute_gpu_guest::admit;
use xpute_link::frame::HDR_SZ;

/// Whether this is a build made for measuring: `PXL_PERF_DRIVE=1` where it is
/// compiled, read as the page's build reads it (ts/client babel.config.ts).
pub const MEASURING: bool = match option_env!("PXL_PERF_DRIVE") {
    Some(v) => matches!(v.as_bytes(), b"1"),
    None => false,
};

/// Whether the GPU's host takes a frame now without blocking (xpute-gpu-guest
/// `admit::frame`), by the device's block, where the host writes the frame
/// latency spec/client/render/api.json5 `device` sets.
pub(crate) fn admit_frame() -> bool {
    admit::frame()
}

// On a page the core is its own module, so it declares its heap the
// module's global allocator here; on a device pxl-runtime declares it.
#[cfg(target_arch = "wasm32")]
#[global_allocator]
static HEAP: sys::heap::Heap = sys::heap::Heap;

// A frame's cap holds its words and the largest packet the core lays
// (link/bound.rs).
xpute_guest::link!(
    link::bound::PKT_CAP + HDR_SZ,
    link::boot,
    link::enter,
    link::end
);
