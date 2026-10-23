// pxl-native/surface.rs

//! The device the kernel draws with, and what the app's views hand it to draw
//! into. The renderer writes its calls into a stream as it does in a page
//! (pxl_render::gpu::api), and xpute's host (xpute-gpu host) runs them on the
//! device after each turn, as the page's runs them on WebGPU. A view owns
//! what it is drawn through — an iPhone's view its layer, an Android view its
//! surface's window (android.rs): it binds the kernel's surface to it while
//! it is in a window and has a surface to show, and unbinds it before it lets
//! it go, so it is only ever drawn into while its view keeps it. The kernel's
//! thread (thread.rs) makes every call here, whichever thread the view is on,
//! and every frame is drawn and shown inside a turn. On an iPhone a surface
//! is configured on the main thread instead, since that sets its layer's
//! properties, which UIKit allows there alone.

#[cfg(target_vendor = "apple")]
use std::ffi::c_void;
#[cfg(target_vendor = "apple")]
use std::sync::atomic::AtomicUsize;
use std::sync::atomic::{AtomicU32, Ordering};

use pxl_render::api_spec::stream;
use xpute_gpu::host::{ConfigureAt, GpuHost};
use xpute_guest::abi::handle::handle_slot;
use xpute_guest::global::Global;

use crate::thread::on_kernel;
#[cfg(target_vendor = "apple")]
use crate::thread::while_kernel_idle;

/// An iPhone's layer is the main thread's, an Android window anyone's.
#[cfg(target_vendor = "apple")]
const CONFIGURE_SURFACES: ConfigureAt = ConfigureAt::Elsewhere(configure_on_main);
#[cfg(not(target_vendor = "apple"))]
const CONFIGURE_SURFACES: ConfigureAt = ConfigureAt::Here;

/// BGRA on Apple, RGBA on Android, whose Vulkan surfaces offer no BGRA.
#[cfg(target_vendor = "apple")]
const PREFERRED: wgpu::TextureFormat = wgpu::TextureFormat::Bgra8Unorm;
#[cfg(not(target_vendor = "apple"))]
const PREFERRED: wgpu::TextureFormat = wgpu::TextureFormat::Rgba8Unorm;

/// A layer keeps three drawables, so with two frames unfinished a third is
/// always free, and taking it never blocks the JavaScript thread, which waits
/// for the turn (thread.rs).
const FRAMES_IN_FLIGHT: u32 = 2;

/// The frames drawn and not yet done on the GPU. The queue's callback takes
/// one off, run by the host's poll before a turn; it captures nothing, so
/// handing it over allocates nothing.
static UNFINISHED: AtomicU32 = AtomicU32::new(0);

struct Device {
    instance: wgpu::Instance,
    host: GpuHost,
    /// The host's queue, asked when a frame's work is done.
    queue: wgpu::Queue,
    /// This turn's frame was admitted, so it is drawn and counted.
    admitted: bool,
    /// The kernel's handle bound under each surface slot: a slot outlives its
    /// handle, and unbinding an old one must not take the one bound since.
    bound: Vec<u32>,
}

static DEVICE: Global<Option<Device>> = Global::new();

fn device() -> Option<&'static mut Device> {
    DEVICE.get(|| None).as_mut()
}

/// Asks the main queue to configure what is stale, once the kernel is idle;
/// asking again before it has run configures nothing twice.
#[cfg(target_vendor = "apple")]
fn configure_on_main() {
    unsafe extern "C" {
        static _dispatch_main_q: c_void;
        fn dispatch_async_f(queue: *const c_void, context: *mut c_void, work: extern "C" fn(*mut c_void));
    }
    extern "C" fn work(_: *mut c_void) {
        while_kernel_idle(|| {
            if let Some(d) = device() {
                d.host.configure_surfaces();
            }
        });
    }
    // SAFETY: libdispatch's main queue, which lives as long as the process.
    unsafe { dispatch_async_f(&raw const _dispatch_main_q, core::ptr::null_mut(), work) };
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
/// the strobe for a frame (apps/native apps/native/src/gpu.ts).
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

/// Asks for the adapter's own limits: the simulator's Metal falls short of
/// WebGPU's defaults and refuses a device that asks for them.
fn open() -> bool {
    if device().is_some() {
        return true;
    }
    let instance = wgpu::Instance::new(wgpu::InstanceDescriptor {
        backends: wgpu::Backends::PRIMARY,
        ..wgpu::InstanceDescriptor::new_without_display_handle()
    });
    let Ok(adapter) = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
        power_preference: wgpu::PowerPreference::HighPerformance,
        ..Default::default()
    })) else {
        return false;
    };
    let Ok((device, queue)) = pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor {
        required_features: adapter.features() & wgpu::Features::TIMESTAMP_QUERY,
        required_limits: adapter.limits(),
        ..Default::default()
    })) else {
        return false;
    };
    let host = GpuHost::new(adapter, device, queue.clone(), PREFERRED, CONFIGURE_SURFACES, wake);
    DEVICE.set(Some(Device {
        instance,
        host,
        queue,
        admitted: false,
        bound: Vec::new(),
    }));
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
pub fn before_turn(memory: &mut [u8]) {
    if let Some(d) = device() {
        d.host.poll(memory);
    }
}

/// The bracket's answer (pxl_kernel::door::Bracket::admit_frame): a frame
/// while the GPU has fewer than `FRAMES_IN_FLIGHT` unfinished, so the
/// surface's drawable is free and taking it does not block.
pub fn admit_frame() -> bool {
    device().is_some_and(|d| {
        d.admitted = UNFINISHED.load(Ordering::Relaxed) < FRAMES_IN_FLIGHT;
        d.admitted
    })
}

/// The turn's calls run on the device, and its frames shown; an admitted
/// frame is unfinished until the queue has done the work submitted with it.
pub fn after_turn(memory: &mut [u8]) {
    if let (Some(d), Some(at)) = (device(), pxl_render::gpu::api::stream_at()) {
        d.host.run(memory, at, stream::HEAD);
        d.host.present();
        if core::mem::take(&mut d.admitted) {
            UNFINISHED.fetch_add(1, Ordering::Relaxed);
            d.queue.on_submitted_work_done(|| {
                UNFINISHED.fetch_sub(1, Ordering::Relaxed);
            });
        }
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
        let slot = handle_slot(handle);
        if d.bound.len() <= slot as usize {
            d.bound.resize(slot as usize + 1, 0);
        }
        d.bound[slot as usize] = handle;
        d.host.bind_surface(slot, surface);
        0
    })
}

/// Binds the kernel's surface `handle` to `layer`, a view's `CAMetalLayer`: 0
/// when bound, -2 when no device is open, -3 when the system would not make a
/// surface of the layer.
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

/// Lets `handle`'s layer go, if it is still the one bound under its slot.
#[no_mangle]
pub extern "C" fn pxl_surface_unbind(handle: u32) {
    on_kernel(move || {
        let Some(d) = device() else { return };
        let slot = handle_slot(handle);
        if d.bound.get(slot as usize) == Some(&handle) {
            d.bound[slot as usize] = 0;
            d.host.unbind_surface(slot);
        }
    });
}
