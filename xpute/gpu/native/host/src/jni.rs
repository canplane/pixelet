// xpute-gpu-native-host/jni.rs

//! This is the GPU's host as an Android app reaches it: the C ABI's calls
//! under the names JNI looks for, as the `external` functions of the Kotlin
//! class `xpute.Gpu` (`jni_abi!`), and the host's state as a call of its
//! static `onState(String)`. A view's `Surface` crosses as the native window wgpu draws
//! through (`target`), which the host holds from its bind until its unbind
//! (`native::bind`).

use std::ptr::NonNull;
use std::sync::OnceLock;

use xpute_kit::status::errno::Errno;
use xpute_native_runtime::jni::ndk::{ANativeWindow_fromSurface, ANativeWindow_release};
use xpute_native_runtime::jni::sys::{jint, jobject, jvalue, JNIEnv};
use xpute_native_runtime::jni::{call, class, string, Class};

use super::native::state_on;

/// The class `xpute.Gpu` and its static method `onState(Ljava/lang/String;)V`,
/// at `ON_STATE`.
static CLASS: OnceLock<Class<1>> = OnceLock::new();
const ON_STATE: usize = 0;

/// Takes the class `xpute.Gpu` from `this`, once, and makes its static method
/// hear the host's state (`state_on`).
///
/// # Safety
/// `e` and `this` are what JNI called with, on a thread Java made, `this` an
/// `xpute.Gpu`.
pub unsafe fn init(e: *mut JNIEnv, this: jobject) {
    if CLASS.get().is_some() {
        return;
    }
    let mut methods = [(c"", c""); 1];
    methods[ON_STATE] = (c"onState", c"(Ljava/lang/String;)V");
    // SAFETY: what JNI called with, on a thread Java made.
    let Some(c) = (unsafe { class(e, this, methods) }) else {
        return;
    };
    let _ = CLASS.set(c);
    state_on(Some(state));
}

/// Calls `Gpu.onState` from the thread that tells the state (`state_on`), the
/// GPU's (native.rs), attached to the VM for good as a daemon, as the guest's
/// thread is for `Runtime.onNotify`.
extern "C" fn state(state: *const u8, len: usize) {
    let Some(c) = CLASS.get() else { return };
    let args = |e| {
        // SAFETY: this thread's environment, and the name's UTF-8, the
        // host's for the call.
        let s = unsafe { string(e, state, len) };
        vec![jvalue { l: s }]
    };
    // SAFETY: `onState(Ljava/lang/String;)V`, at its index, and its one value.
    unsafe { call(c, ON_STATE, args) }
}

/// This is `Gpu.surfaceBind`: it binds the guest's surface `handle` to a view's
/// `surface`, through the native window the surface is (`bind`, the program's
/// `xpute_gpu_surface_bind`), which holds its own reference on the window. It
/// returns 0, as `bind` does, or EINVAL where the surface has no native
/// window.
///
/// # Safety
/// `e` is what JNI called with, and `surface` a live `android.view.Surface`.
pub unsafe fn bind(
    e: *mut JNIEnv,
    handle: u32,
    surface: jobject,
    bind: unsafe extern "C" fn(u32, *mut core::ffi::c_void) -> i32,
) -> jint {
    // SAFETY: the environment and the surface the call was given; the window
    // made this way is this call's until released below.
    let Some(window) = NonNull::new(unsafe { ANativeWindow_fromSurface(e, surface) }) else {
        return Errno::EINVAL as i32;
    };
    // SAFETY: a live window, on which `bind` takes a reference of its own.
    let bound = unsafe { bind(handle, window.as_ptr().cast()) };
    // SAFETY: the reference `ANativeWindow_fromSurface` gave this call.
    unsafe { ANativeWindow_release(window.as_ptr()) };
    bound
}

/// Exports the GPU's host to JNI for the Kotlin class `xpute.Gpu`, which
/// `native_abi!` expands on Android: `init()`, which makes its static
/// `onState(state: String)` hear the host's state (`init`), `state(): String?`,
/// the host's state now (`native::state`), `surfaceBind(handle: Int,
/// surface: Surface): Int`, `surfaceUnbind(handle: Int)` and `vsync()`.
#[macro_export]
macro_rules! jni_abi {
    () => {
        /// # Safety
        /// Only JNI calls this, on a thread Java made.
        #[no_mangle]
        unsafe extern "system" fn Java_xpute_Gpu_init(
            env: *mut ::xpute_native_runtime::jni::sys::JNIEnv,
            this: ::xpute_native_runtime::jni::sys::jobject,
        ) {
            // SAFETY: the environment and the `xpute.Gpu` JNI called with.
            unsafe { $crate::jni::init(env, this) }
        }

        /// # Safety
        /// Only JNI calls this, on a thread Java made.
        #[no_mangle]
        unsafe extern "system" fn Java_xpute_Gpu_state(
            env: *mut ::xpute_native_runtime::jni::sys::JNIEnv,
            _this: ::xpute_native_runtime::jni::sys::jobject,
        ) -> ::xpute_native_runtime::jni::sys::jstring {
            let Some(s) = $crate::native::state() else {
                return ::core::ptr::null_mut();
            };
            // SAFETY: the environment JNI called with, and the name's UTF-8.
            unsafe { ::xpute_native_runtime::jni::string(env, s.as_ptr(), s.len()) }
        }

        /// # Safety
        /// Only JNI calls this, with a live `android.view.Surface`.
        #[no_mangle]
        unsafe extern "system" fn Java_xpute_Gpu_surfaceBind(
            env: *mut ::xpute_native_runtime::jni::sys::JNIEnv,
            _this: ::xpute_native_runtime::jni::sys::jobject,
            handle: ::xpute_native_runtime::jni::sys::jint,
            surface: ::xpute_native_runtime::jni::sys::jobject,
        ) -> ::xpute_native_runtime::jni::sys::jint {
            // SAFETY: the environment and surface JNI called with.
            unsafe {
                $crate::jni::bind(env, handle as u32, surface, xpute_gpu_surface_bind)
            }
        }

        #[no_mangle]
        extern "system" fn Java_xpute_Gpu_surfaceUnbind(
            _env: *mut ::xpute_native_runtime::jni::sys::JNIEnv,
            _this: ::xpute_native_runtime::jni::sys::jobject,
            handle: ::xpute_native_runtime::jni::sys::jint,
        ) {
            xpute_gpu_surface_unbind(handle as u32);
        }

        #[no_mangle]
        extern "system" fn Java_xpute_Gpu_vsync(
            _env: *mut ::xpute_native_runtime::jni::sys::JNIEnv,
            _this: ::xpute_native_runtime::jni::sys::jobject,
        ) {
            xpute_gpu_vsync();
        }
    };
}
