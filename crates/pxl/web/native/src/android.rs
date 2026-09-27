// pxl-native/android.rs

//! The kernel as an Android app reaches it. Kotlin's `PxlCore`
//! (apps/native/modules/platform/android/src/main/java/me/plei/pixelet/platform/PxlCore.kt)
//! declares these as `external` functions, and each is the C ABI's call an
//! iPhone's Swift makes (lib.rs, surface.rs, image.rs) under the name JNI
//! looks for. Only JNI's own values cross: a number, an array, the memory as
//! a direct buffer over the region — which Kotlin hands JavaScript as it is,
//! as Swift hands it the region — and a view's `Surface`, made the native
//! window wgpu draws through.

use std::collections::HashMap;
use std::ptr::{null_mut, NonNull};
use std::sync::Mutex;

use jni_sys::{jbyteArray, jclass, jdouble, jint, jintArray, jobject, JNIEnv, JNINativeInterface_};
use ndk_sys::{ANativeWindow, ANativeWindow_fromSurface, ANativeWindow_release};
use wgpu::rwh::{AndroidDisplayHandle, AndroidNdkWindowHandle, RawDisplayHandle, RawWindowHandle};
use xpute_core::status::bug::OrBug;
use xpute_core::status::errno::Errno;

/// The native window each bound surface draws through, by the kernel's
/// handle: acquired when a view's surface is bound and released once it is
/// not, since the kernel's surface over it is gone by then.
static WINDOWS: Mutex<Option<HashMap<u32, usize>>> = Mutex::new(None);

/// JNI's functions, as the environment a call is given holds them.
///
/// # Safety
/// `env` is the environment JNI called with.
unsafe fn jni<'e>(env: *mut JNIEnv) -> &'e JNINativeInterface_ {
    // SAFETY: an environment points at its function table for the call.
    unsafe { &**env }
}

/// A turn of `quota_ms` (lib.rs `pxl_interrupt`).
#[no_mangle]
pub extern "system" fn Java_me_plei_pixelet_platform_PxlCore_interrupt(_env: *mut JNIEnv, _class: jclass, quota_ms: jdouble) -> jdouble {
    crate::pxl_interrupt(quota_ms)
}

/// The memory, as a direct buffer over the whole region (lib.rs `pxl_memory`).
///
/// # Safety
/// Called by JNI.
#[no_mangle]
pub unsafe extern "system" fn Java_me_plei_pixelet_platform_PxlCore_memory(env: *mut JNIEnv, _class: jclass) -> jobject {
    let mut bytes = 0u32;
    // SAFETY: our own word.
    let at = unsafe { crate::pxl_memory(&raw mut bytes) };
    let make = unsafe { jni(env) }.NewDirectByteBuffer.or_bug(Errno::ENOSYS);
    // SAFETY: the region lives as long as the process, and is `bytes` long.
    unsafe { make(env, at.cast(), i64::from(bytes)) }
}

/// Opens the device: its uniform offset alignment and whether it times its
/// passes, or null where the system has no GPU (surface.rs `pxl_gpu_open`).
///
/// # Safety
/// Called by JNI.
#[no_mangle]
pub unsafe extern "system" fn Java_me_plei_pixelet_platform_PxlCore_gpuOpen(env: *mut JNIEnv, _class: jclass) -> jintArray {
    let (mut align, mut timestamps) = (0u32, 0u32);
    // SAFETY: our own words.
    if unsafe { crate::surface::pxl_gpu_open(&raw mut align, &raw mut timestamps) } != 0 {
        return null_mut();
    }
    // SAFETY: the environment JNI called with.
    let table = unsafe { jni(env) };
    let words = [align as jint, timestamps as jint];
    // SAFETY: a fresh array of the two words it is filled with.
    unsafe {
        let array = table.NewIntArray.or_bug(Errno::ENOSYS)(env, 2);
        table.SetIntArrayRegion.or_bug(Errno::ENOSYS)(env, array, 0, 2, words.as_ptr());
        array
    }
}

/// Decodes an image and holds it for the renderer: its handle, or 0 (image.rs
/// `pxl_image_hold`). Off the kernel's thread, as on an iPhone.
///
/// # Safety
/// Called by JNI.
#[no_mangle]
pub unsafe extern "system" fn Java_me_plei_pixelet_platform_PxlCore_imageHold(env: *mut JNIEnv, _class: jclass, file: jbyteArray) -> jint {
    // SAFETY: the environment JNI called with.
    let table = unsafe { jni(env) };
    // SAFETY: `file` is the byte array the call was given, copied whole.
    let bytes = unsafe {
        let len = table.GetArrayLength.or_bug(Errno::ENOSYS)(env, file);
        let mut bytes = vec![0u8; len as usize];
        table.GetByteArrayRegion.or_bug(Errno::ENOSYS)(env, file, 0, len, bytes.as_mut_ptr().cast());
        bytes
    };
    // SAFETY: our own bytes, for the call.
    unsafe { crate::image::pxl_image_hold(bytes.as_ptr(), bytes.len() as u32) as jint }
}

/// Binds the kernel's surface `handle` to a view's `surface`, through the
/// native window it is: 0, -2 when no device is open, -3 when the system
/// would not make a surface of it (surface.rs `bind`).
///
/// # Safety
/// Called by JNI, with a live `android.view.Surface`, which its view keeps
/// until it has unbound it.
#[no_mangle]
pub unsafe extern "system" fn Java_me_plei_pixelet_platform_PxlCore_surfaceBind(env: *mut JNIEnv, _class: jclass, handle: jint, surface: jobject) -> jint {
    let handle = handle as u32;
    // SAFETY: the environment and the surface the call was given; a window
    // made this way is ours until released.
    let Some(window) = NonNull::new(unsafe { ANativeWindow_fromSurface(env, surface) }) else {
        return -3;
    };
    let at = window.as_ptr().expose_provenance();
    let made = move || wgpu::SurfaceTargetUnsafe::RawHandle {
        raw_display_handle: Some(RawDisplayHandle::Android(AndroidDisplayHandle::new())),
        raw_window_handle: RawWindowHandle::AndroidNdk(AndroidNdkWindowHandle::new(NonNull::new(core::ptr::with_exposed_provenance_mut(at)).or_bug(Errno::EFAULT))),
    };
    // SAFETY: the window is held until the surface over it is unbound.
    let bound = unsafe { crate::surface::bind(handle, made) };
    let let_go = if bound == 0 {
        WINDOWS.lock().or_bug(Errno::ENOTRECOVERABLE).get_or_insert_with(HashMap::new).insert(handle, at)
    } else {
        Some(at)
    };
    if let Some(old) = let_go {
        // SAFETY: a window this module acquired, whose surface is gone.
        unsafe { ANativeWindow_release(core::ptr::with_exposed_provenance_mut::<ANativeWindow>(old)) };
    }
    bound
}

/// Lets `handle`'s surface go, and the window it drew through
/// (surface.rs `pxl_surface_unbind`).
#[no_mangle]
pub extern "system" fn Java_me_plei_pixelet_platform_PxlCore_surfaceUnbind(_env: *mut JNIEnv, _class: jclass, handle: jint) {
    let handle = handle as u32;
    crate::surface::pxl_surface_unbind(handle);
    let window = WINDOWS.lock().or_bug(Errno::ENOTRECOVERABLE).as_mut().and_then(|w| w.remove(&handle));
    if let Some(at) = window {
        // SAFETY: a window this module acquired, whose surface is gone.
        unsafe { ANativeWindow_release(core::ptr::with_exposed_provenance_mut::<ANativeWindow>(at)) };
    }
}

/// A line to the system's log, where an Android app's error stream goes
/// nowhere anyone reads.
pub fn say(line: &str) {
    let line = std::ffi::CString::new(line.replace('\0', " ")).unwrap_or_default();
    // SAFETY: two strings, ours for the call.
    unsafe { ndk_sys::__android_log_write(ndk_sys::android_LogPriority::ANDROID_LOG_ERROR.0 as i32, c"pxl".as_ptr(), line.as_ptr()) };
}
