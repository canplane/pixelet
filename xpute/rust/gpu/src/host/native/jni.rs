// xpute-gpu/host/native/jni.rs

//! The GPU's host as an Android app reaches it: the C ABI's calls under the
//! names JNI looks for, for the Kotlin class `xpute.Gpu`, whose `external`
//! functions they are (`native_abi!`). A view's `Surface` crosses as the
//! native window wgpu draws through (`target`), held from its bind until its
//! unbind.

use std::collections::HashMap;
use std::ptr::NonNull;
use std::sync::Mutex;

use xpute_host::jni::ndk::{ANativeWindow, ANativeWindow_fromSurface, ANativeWindow_release};
use xpute_host::jni::sys::{jint, jobject, JNIEnv};
use xpute_kit::status::bug::OrBug;
use xpute_kit::status::errno::Errno;

/// The native window each bound surface draws through, by the guest's
/// handle: acquired when a view's surface is bound and released once it is
/// not, since the guest's surface over it is gone by then.
static WINDOWS: Mutex<Option<HashMap<u32, usize>>> = Mutex::new(None);

/// `Gpu.surfaceBind`: the guest's surface `handle` bound to a view's
/// `surface` through the native window it is (`bind`, the program's
/// `xpute_gpu_surface_bind`): 0, `NOT_OPEN`, `NO_SURFACE` or `SLOT_TAKEN`.
///
/// # Safety
/// `e` is what JNI called with, and `surface` a live `android.view.Surface`,
/// which its view keeps until it has unbound it.
pub unsafe fn bind(
    e: *mut JNIEnv,
    handle: u32,
    surface: jobject,
    bind: unsafe fn(u32, *mut core::ffi::c_void) -> i32,
) -> jint {
    // SAFETY: the environment and the surface the call was given; a window
    // made this way is ours until released.
    let Some(window) = NonNull::new(unsafe { ANativeWindow_fromSurface(e, surface) }) else {
        return super::NO_SURFACE;
    };
    let at = window.as_ptr().expose_provenance();
    // SAFETY: the window is held until the surface over it is unbound.
    let bound = unsafe { bind(handle, window.as_ptr().cast()) };
    let let_go = if bound == 0 {
        WINDOWS
            .lock()
            .or_bug(Errno::ENOTRECOVERABLE)
            .get_or_insert_with(HashMap::default)
            .insert(handle, at)
    } else {
        Some(at)
    };
    if let Some(old) = let_go {
        // SAFETY: a window this module acquired, whose surface is gone.
        unsafe {
            ANativeWindow_release(core::ptr::with_exposed_provenance_mut::<ANativeWindow>(old))
        };
    }
    bound
}

/// `Gpu.surfaceUnbind`: `handle`'s surface let go (`unbind`, the program's
/// `xpute_gpu_surface_unbind`), and the window it drew through.
pub fn unbind(handle: u32, unbind: fn(u32)) {
    unbind(handle);
    let window = WINDOWS
        .lock()
        .or_bug(Errno::ENOTRECOVERABLE)
        .as_mut()
        .and_then(|w| w.remove(&handle));
    if let Some(at) = window {
        // SAFETY: a window this module acquired, whose surface is gone.
        unsafe {
            ANativeWindow_release(core::ptr::with_exposed_provenance_mut::<ANativeWindow>(at))
        };
    }
}
