// pxl-native/surface.rs

//! The device the kernel draws with, and what the app's views hand it to draw
//! into. The renderer writes its calls into a stream as it does in a page
//! (pxl_render::gpu::api), and xpute's host (xpute-gpu host) runs them on the
//! device after each turn, as the page's runs them on WebGPU. A view owns
//! what it is drawn through — an iPhone's view its layer, an Android view its
//! surface's window (android.rs): it binds the kernel's surface to it while
//! it is in a window and has a surface to show, and unbinds it before it lets
//! it go, so it is only ever drawn into while its view keeps it. Holding the
//! device and binding a window are xpute's (xpute-gpu host native), on the
//! kernel's thread whichever thread the view is on, and every frame is drawn
//! and shown inside a turn. What is pixelet's here is its numbers, what a
//! view hands over and the turn's edges.

use pxl_render::api_spec::{device as numbers, stream};
use xpute_gpu::host::native;
use xpute_gpu::host::{Ask, Config};
use xpute_host::mem::Memory;

#[cfg(target_os = "android")]
use crate::android::target;
use pxl_render::gpu::api;

/// BGRA on Apple, RGBA on Android, whose Vulkan surfaces offer no BGRA.
#[cfg(target_vendor = "apple")]
const PREFERRED: wgpu::TextureFormat = wgpu::TextureFormat::Bgra8Unorm;
#[cfg(not(target_vendor = "apple"))]
const PREFERRED: wgpu::TextureFormat = wgpu::TextureFormat::Rgba8Unorm;

/// What the host runs by: the renderer's numbers (spec/client/render/gpu/api.json
/// `device`) and the platform's surface.
const CFG: Config = Config {
    preferred: PREFERRED,
    configure_at: native::CONFIGURE_AT,
    frame_latency: numbers::FRAME_LATENCY,
    upload_chunk_bytes: numbers::UPLOAD_CHUNK_BYTES as u64,
    present_mode: wgpu::PresentMode::Fifo,
};

/// The device the kernel asks for: the timestamps its spans are timed by
/// where the adapter has them, and the adapter's own limits, since the
/// simulator's Metal falls short of WebGPU's defaults and refuses a device
/// that asks for them.
const ASK: Ask = Ask {
    backends: wgpu::Backends::PRIMARY,
    power_preference: wgpu::PowerPreference::HighPerformance,
    features: wgpu::Features::TIMESTAMP_QUERY,
    limits: wgpu::Adapter::limits,
    memory_hints: wgpu::MemoryHints::Performance,
};

/// What an iPhone's view hands over to be drawn into: its `CAMetalLayer`.
#[cfg(target_vendor = "apple")]
fn target(layer: core::ptr::NonNull<core::ffi::c_void>) -> wgpu::SurfaceTargetUnsafe {
    wgpu::SurfaceTargetUnsafe::CoreAnimationLayer(layer.as_ptr())
}

// The device opened on the kernel's thread, a view's window bound to a
// surface and let go, and what a completion calls to ask a turn, exported
// under xpute's names as `xpute_gpu_open`, `xpute_gpu_wake_on`,
// `xpute_gpu_surface_bind` and `xpute_gpu_surface_unbind`
// (include/pxl_native.h): the app's loop then sends the app an event, and
// JavaScript asks the strobe for a frame (ts/app/native/src/gpu.ts).
xpute_gpu::native_abi!(crate::guest, ASK, CFG, target);

/// The turn's edge, on the kernel's thread: what the device has answered
/// since, written into `memory` before the turn reads it.
pub fn before_turn(mem: &mut Memory<'_>) {
    if let Some(h) = native::host() {
        h.poll(mem);
    }
}

/// The host's answer (xpute_gpu::admit), handed the kernel with its entry: a
/// frame while the host admits one (xpute-gpu host `GpuHost::admit`), so
/// taking a layer's drawable never blocks the JavaScript thread, which waits
/// for the turn (xpute-host native `Guest::run`); every frame before the
/// device opens.
pub fn admit() -> bool {
    native::host().is_none_or(|h| h.admit())
}

/// The turn's calls run on the device, and its frames shown.
pub fn after_turn(mem: &mut Memory<'_>) {
    if let (Some(h), Some(at)) = (native::host(), api::stream_at()) {
        h.run(mem, at, stream::HEAD);
        h.present();
    }
}
