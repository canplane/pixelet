// pxl-native/surface.rs

//! The device the kernel draws with, and what the app's views hand it to draw
//! into. The renderer writes its calls into a stream as it does in a page
//! (pxl_render::gpu::api), and xpute's host (xpute-gpu host) runs them on the
//! device after each turn, as the page's runs them on WebGPU. A view owns
//! what it is drawn through — an iPhone's view its layer, an Android view its
//! surface's window (android.rs): it binds the kernel's surface to it while
//! it is in a window and has a surface to show, and unbinds it before it lets
//! it go, so it is only ever drawn into while its view keeps it. The kernel's
//! thread (lib.rs `on_kernel`) makes every call here, whichever thread the view is on,
//! and every frame is drawn and shown inside a turn. On an iPhone a surface
//! is configured on the main thread instead, since that sets its layer's
//! properties, which UIKit allows there alone.

#[cfg(target_vendor = "apple")]
use std::ffi::c_void;
#[cfg(target_vendor = "apple")]
use std::sync::atomic::{AtomicUsize, Ordering};

use pxl_render::api_spec::{device as numbers, stream};
use xpute_gpu::host::{Ask, ConfigureAt, GpuHost, Opened, Plan};
use xpute_guest::global::Global;
use xpute_host::mem::Memory;

use crate::on_kernel;

/// An iPhone's layer is the main thread's, an Android window anyone's.
#[cfg(target_vendor = "apple")]
const CONFIGURE_SURFACES: ConfigureAt = ConfigureAt::Main(configure);
#[cfg(not(target_vendor = "apple"))]
const CONFIGURE_SURFACES: ConfigureAt = ConfigureAt::Here;

/// BGRA on Apple, RGBA on Android, whose Vulkan surfaces offer no BGRA.
#[cfg(target_vendor = "apple")]
const PREFERRED: wgpu::TextureFormat = wgpu::TextureFormat::Bgra8Unorm;
#[cfg(not(target_vendor = "apple"))]
const PREFERRED: wgpu::TextureFormat = wgpu::TextureFormat::Rgba8Unorm;

/// What the host runs by: the renderer's numbers (spec/client/render/gpu/api.json
/// `device`) and the platform's surface.
const PLAN: Plan = Plan {
    preferred: PREFERRED,
    configure_at: CONFIGURE_SURFACES,
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

struct Device {
    instance: wgpu::Instance,
    host: GpuHost,
}

static DEVICE: Global<Option<Device>> = Global::new();

fn device() -> Option<&'static mut Device> {
    DEVICE.get(|| None).as_mut()
}

/// What is stale configured, on the main thread (xpute-gpu host
/// `ConfigureAt::Main`), once the kernel is idle.
#[cfg(target_vendor = "apple")]
fn configure() {
    crate::guest().while_idle(|| {
        if let Some(d) = device() {
            d.host.configure_surfaces();
        }
    });
}

/// What Swift gave to be called for a turn (`pxl_wake_on`), as an address.
#[cfg(target_vendor = "apple")]
static WAKE: AtomicUsize = AtomicUsize::new(0);

/// `wake` calls `f`, from whichever thread learns of a completion: Swift sends
/// the app's event from there, which Expo carries to the JavaScript thread.
#[cfg(target_vendor = "apple")]
#[no_mangle]
pub extern "C" fn pxl_wake_on(f: extern "C" fn()) {
    WAKE.store(f as usize, Ordering::Release);
}

/// The app's loop asked for a turn, from whichever thread learned of a
/// completion the host writes at the next turn's poll (xpute-gpu host
/// `GpuHost::new`): the platform sends the app an event, and JavaScript asks
/// the strobe for a frame (ts/app/native ts/app/native/src/gpu.ts).
#[cfg(target_vendor = "apple")]
fn wake() {
    let f = WAKE.load(Ordering::Acquire);
    if f != 0 {
        // SAFETY: an `extern "C" fn()` Swift gave, stored as its address.
        let f: extern "C" fn() = unsafe { core::mem::transmute(f) };
        f();
    }
}
#[cfg(target_os = "android")]
use crate::android::wake;

/// The device opened as asked (xpute-gpu host `open`), once.
fn open() -> bool {
    if device().is_some() {
        return true;
    }
    let Ok(Opened { instance, adapter, device, queue }) = xpute_gpu::host::open(&ASK) else {
        return false;
    };
    let host = GpuHost::new(adapter, device, queue, PLAN, wake);
    DEVICE.set(Some(Device { instance, host }));
    true
}

/// Opens the device: 0 when it is open, -1 when the system has none. What it
/// is, the kernel reads from the block its first call asks the host for.
#[no_mangle]
pub extern "C" fn pxl_gpu_open() -> i32 {
    if on_kernel(open) {
        0
    } else {
        -1
    }
}

/// The turn's edge, on the kernel's thread: what the device has answered
/// since, written into `memory` before the turn reads it.
pub fn before_turn(memory: &mut Memory<'_>) {
    if let Some(d) = device() {
        d.host.poll(memory);
    }
}

/// The bracket's answer (pxl_kernel::door::Bracket::admit_frame): a frame
/// while the host admits one (xpute-gpu host `GpuHost::admit`), so taking a
/// layer's drawable never blocks the JavaScript thread, which waits for the
/// turn (lib.rs `on_kernel`).
pub fn admit_frame() -> bool {
    device().is_some_and(|d| d.host.admit())
}

/// The turn's calls run on the device, and its frames shown.
pub fn after_turn(memory: &mut Memory<'_>) {
    if let (Some(d), Some(at)) = (device(), pxl_render::gpu::api::stream_at()) {
        d.host.run(memory, at, stream::HEAD);
        d.host.present();
    }
}

/// Binds the kernel's surface `handle` to the target `made` makes on the
/// kernel's thread, where every surface is made: 0 when bound, -2 when no
/// device is open, -3 when the system would not make a surface of it. The
/// target is made there, not handed there, since it is raw pointers and no
/// thread's but the one that uses it.
///
/// # Safety
/// What `made` makes is live, and its view keeps it until it has unbound the
/// surface.
pub(crate) unsafe fn bind(handle: u32, made: impl FnOnce() -> wgpu::SurfaceTargetUnsafe + Send) -> i32 {
    on_kernel(move || {
        let Some(d) = device() else { return -2 };
        // SAFETY: the caller's target, kept until the surface is unbound.
        let Ok(surface) = (unsafe { d.instance.create_surface_unsafe(made()) }) else { return -3 };
        if !d.host.bind(handle, surface) {
            return -4;
        }
        0
    })
}

/// Binds the kernel's surface `handle` to `layer`, a view's `CAMetalLayer`: 0
/// when bound, -2 when no device is open, -3 when the system would not make a
/// surface of the layer, -4 when a newer handle has bound its slot since.
///
/// # Safety
/// `layer` is a live `CAMetalLayer`, which its view keeps until it has
/// unbound the surface.
#[cfg(target_vendor = "apple")]
#[no_mangle]
pub unsafe extern "C" fn pxl_surface_bind(handle: u32, layer: *mut c_void) -> i32 {
    let layer = layer.expose_provenance();
    // SAFETY: the caller's layer, kept until the surface is unbound.
    unsafe { bind(handle, move || wgpu::SurfaceTargetUnsafe::CoreAnimationLayer(core::ptr::with_exposed_provenance_mut(layer))) }
}

/// Lets `handle`'s layer go, if it is still the one bound under its slot
/// (xpute-gpu host `GpuHost::unbind`).
#[no_mangle]
pub extern "C" fn pxl_surface_unbind(handle: u32) {
    on_kernel(move || {
        if let Some(d) = device() {
            d.host.unbind(handle);
        }
    });
}
