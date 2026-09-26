// pxl-native/surface.rs

//! What the app's view hands the kernel to draw into, and the device it draws
//! with (pxl_render::gpu::api, the device backend). The view lives on the
//! app's main thread and the kernel on its own (thread.rs), so the view only
//! leaves its layer here, under a lock; the kernel's thread opens the device
//! and binds the layer to the surface the renderer names, and every frame is
//! drawn and shown inside a turn.

use std::ffi::c_void;
use std::sync::Mutex;

use crate::thread::on_kernel;

/// The view's `CAMetalLayer`, while the view is in a window: an address, left
/// by the main thread and read by the kernel's.
static LAYER: Mutex<usize> = Mutex::new(0);

/// The view's layer is there to draw into.
///
/// # Safety
/// `layer` is a live `CAMetalLayer`, which the view keeps until it detaches.
#[no_mangle]
pub unsafe extern "C" fn pxl_surface_attach(layer: *mut c_void) {
    *LAYER.lock().unwrap_or_else(|e| e.into_inner()) = layer.expose_provenance();
}

/// The view is going: nothing is bound to its layer from now on.
#[no_mangle]
pub extern "C" fn pxl_surface_detach() {
    *LAYER.lock().unwrap_or_else(|e| e.into_inner()) = 0;
}

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

/// Binds the kernel's surface `handle` to the view's layer: 0 when bound, -1
/// when no view has left one, -2 when no device is open, -3 when the system
/// would not make a surface of the layer.
#[no_mangle]
pub extern "C" fn pxl_surface_bind(handle: u32) -> i32 {
    let layer = *LAYER.lock().unwrap_or_else(|e| e.into_inner());
    if layer == 0 {
        return -1;
    }
    let bound = on_kernel(move || {
        let target = wgpu::SurfaceTargetUnsafe::CoreAnimationLayer(core::ptr::with_exposed_provenance_mut(layer));
        // SAFETY: the view's layer, which it keeps until it detaches; the
        // shell unbinds the surface before the view goes.
        unsafe { pxl_render::gpu::api::surface_bind(handle, target) }
    });
    match bound {
        Ok(()) => 0,
        Err(pxl_render::gpu::api::BindRefused::NoDevice) => -2,
        Err(pxl_render::gpu::api::BindRefused::NoSurface) => -3,
    }
}

#[no_mangle]
pub extern "C" fn pxl_surface_unbind(handle: u32) {
    on_kernel(move || pxl_render::gpu::api::surface_unbind(handle));
}
