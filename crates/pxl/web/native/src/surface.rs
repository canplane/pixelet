// pxl-native/surface.rs

//! What the app's views hand the kernel to draw into, and the device it draws
//! with (pxl_render::gpu::api, the device backend). A view owns its layer: it
//! binds the kernel's surface to it while it is in a window and has a
//! surface to show, and unbinds it before it lets the layer go, so a layer is
//! only ever drawn into while its view keeps it. The kernel's thread
//! (thread.rs) makes every call here, whichever thread the view is on, and
//! every frame is drawn and shown inside a turn.

use std::ffi::c_void;

use crate::thread::on_kernel;

/// Opens the device: its uniform offset alignment and whether it times its
/// passes, written through the two words. 0 when it is open, -1 when the
/// system has none.
///
/// # Safety
/// `align` and `timestamps` point at words the caller owns.
#[no_mangle]
pub unsafe extern "C" fn pxl_gpu_open(align: *mut u32, timestamps: *mut u32) -> i32 {
    let Some(info) = on_kernel(pxl_render::gpu::api::open_device) else { return -1 };
    // SAFETY: the caller's words.
    unsafe {
        *align = info.align;
        *timestamps = info.timestamps as u32;
    }
    0
}

/// Binds the kernel's surface `handle` to `layer`, a view's `CAMetalLayer`: 0
/// when bound, -2 when no device is open, -3 when the system would not make a
/// surface of the layer.
///
/// # Safety
/// `layer` is a live `CAMetalLayer`, which its view keeps until it has
/// unbound the surface.
#[no_mangle]
pub unsafe extern "C" fn pxl_surface_bind(handle: u32, layer: *mut c_void) -> i32 {
    let layer = layer.expose_provenance();
    let bound = on_kernel(move || {
        let target = wgpu::SurfaceTargetUnsafe::CoreAnimationLayer(core::ptr::with_exposed_provenance_mut(layer));
        // SAFETY: the caller's layer, kept until the surface is unbound.
        unsafe { pxl_render::gpu::api::surface_bind(handle, target) }
    });
    match bound {
        Ok(()) => 0,
        Err(pxl_render::gpu::api::BindRefused::NoDevice) => -2,
        Err(pxl_render::gpu::api::BindRefused::NoSurface) => -3,
    }
}

/// Lets `handle`'s layer go, if it is still the one bound under its slot.
#[no_mangle]
pub extern "C" fn pxl_surface_unbind(handle: u32) {
    on_kernel(move || pxl_render::gpu::api::surface_unbind(handle));
}
