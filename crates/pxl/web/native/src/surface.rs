// pxl-native/surface.rs

//! What the app's views hand the kernel to draw into, and the device it draws
//! with (pxl_render::gpu::api, the device backend). A view owns what it is
//! drawn through — an iPhone's view its layer, an Android view its surface's
//! window (android.rs): it binds the kernel's surface to it while it is in a
//! window and has a surface to show, and unbinds it before it lets it go, so
//! it is only ever drawn into while its view keeps it. The kernel's thread
//! (thread.rs) makes every call here, whichever thread the view is on, and
//! every frame is drawn and shown inside a turn. On an iPhone a surface is
//! configured on the main thread instead, since that sets its layer's
//! properties, which UIKit allows there alone.

#[cfg(target_vendor = "apple")]
use std::ffi::c_void;

use pxl_render::gpu::api::ConfigureAt;

use crate::thread::on_kernel;
#[cfg(target_vendor = "apple")]
use crate::thread::while_kernel_idle;

/// An iPhone's layer is the main thread's, an Android window anyone's.
#[cfg(target_vendor = "apple")]
const CONFIGURE_SURFACES: ConfigureAt = ConfigureAt::Elsewhere(configure_on_main);
#[cfg(not(target_vendor = "apple"))]
const CONFIGURE_SURFACES: ConfigureAt = ConfigureAt::Here;

/// Asks the main queue to configure what is stale, once the kernel is idle;
/// asking again before it has run configures nothing twice.
#[cfg(target_vendor = "apple")]
fn configure_on_main() {
    unsafe extern "C" {
        static _dispatch_main_q: c_void;
        fn dispatch_async_f(queue: *const c_void, context: *mut c_void, work: extern "C" fn(*mut c_void));
    }
    extern "C" fn work(_: *mut c_void) {
        while_kernel_idle(pxl_render::gpu::api::surfaces_configure);
    }
    // SAFETY: libdispatch's main queue, which lives as long as the process.
    unsafe { dispatch_async_f(&raw const _dispatch_main_q, core::ptr::null_mut(), work) };
}

/// Opens the device: its uniform offset alignment and whether it times its
/// passes, written through the two words. 0 when it is open, -1 when the
/// system has none.
///
/// # Safety
/// `align` and `timestamps` point at words the caller owns.
#[no_mangle]
pub unsafe extern "C" fn pxl_gpu_open(align: *mut u32, timestamps: *mut u32) -> i32 {
    let Some(info) = on_kernel(|| pxl_render::gpu::api::open_device(CONFIGURE_SURFACES)) else {
        return -1;
    };
    // SAFETY: the caller's words.
    unsafe {
        *align = info.align;
        *timestamps = info.timestamps as u32;
    }
    0
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
    // SAFETY: the caller's target, kept until the surface is unbound.
    let bound = on_kernel(move || unsafe { pxl_render::gpu::api::surface_bind(handle, made()) });
    match bound {
        Ok(()) => 0,
        Err(pxl_render::gpu::api::BindRefused::NoDevice) => -2,
        Err(pxl_render::gpu::api::BindRefused::NoSurface) => -3,
    }
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
    on_kernel(move || pxl_render::gpu::api::surface_unbind(handle));
}
