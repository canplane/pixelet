// pxl-gpu-host/jni.rs

//! This is the GPU's host as an Android app reaches it: the C ABI's calls
//! under the names JNI looks for, as the `external` functions of the Kotlin
//! class `me.plei.pixelet.gpu.Gpu` (`jni_abi!`), and the host's state as a call of its
//! static `onState(String)`, made through the JNI helpers the program's
//! assembly names. A view's `Surface` crosses as the native window wgpu draws
//! through (`target`), which the host holds from its bind until its unbind
//! (`native::bind`).

use std::ptr::NonNull;

use ndk_sys::{ANativeWindow_fromSurface, ANativeWindow_release};
use xpute_kit::status::errno::Errno;

/// JNI's types, which the exports `jni_abi!` expands to name through this
/// crate.
pub use jni_sys as sys;
use sys::{JNIEnv, jint, jobject};

/// This is `Gpu.surfaceBind`: it binds the guest's surface `handle` to a view's
/// `surface`, through the native window the surface is (`bind`, the program's
/// `pxl_gpu_surface_bind`), which holds its own reference on the window. It
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

/// Exports the GPU's host to JNI for the Kotlin class `me.plei.pixelet.gpu.Gpu`, which
/// `native_abi!` expands on Android over the JNI helpers `$jni` names (a
/// module with `Class`, `class`, `call` and `string`), JNI's types its own
/// (`sys`): `init()`, which
/// takes the class from `this` once and makes its static `onState(state:
/// String)` hear the host's state from the thread that tells it, attached to
/// the VM for good as a daemon (`native::state_on`), `state(): String?`, the
/// host's state now (`native::state`), `surfaceBind(handle: Int, surface:
/// Surface): Int` (`bind`), `surfaceUnbind(handle: Int)` and `vsync()`.
#[macro_export]
macro_rules! jni_abi {
    ($($jni:tt)+) => {
        /// The class `me.plei.pixelet.gpu.Gpu` and its static method
        /// `onState(Ljava/lang/String;)V`, its one.
        static PXL_GPU_CLASS: ::std::sync::OnceLock<$($jni)+::Class<1>> =
            ::std::sync::OnceLock::new();

        /// Calls `Gpu.onState` with the state's name, `len` bytes of UTF-8 at
        /// `state`, from the thread that tells it.
        extern "C" fn pxl_gpu_on_state(state: *const u8, len: usize) {
            let Some(c) = PXL_GPU_CLASS.get() else {
                return;
            };
            let args = |e| {
                // SAFETY: this thread's environment, and the name's UTF-8, the
                // host's for the call.
                let s = unsafe { $($jni)+::string(e, state, len) };
                vec![$crate::jni::sys::jvalue { l: s }]
            };
            // SAFETY: `onState(Ljava/lang/String;)V`, its one method, and its
            // one value.
            unsafe { $($jni)+::call(c, 0, args) }
        }

        /// # Safety
        /// Only JNI calls this, on a thread Java made.
        #[unsafe(no_mangle)]
        unsafe extern "system" fn Java_me_plei_pixelet_gpu_Gpu_init(
            env: *mut $crate::jni::sys::JNIEnv,
            this: $crate::jni::sys::jobject,
        ) {
            if PXL_GPU_CLASS.get().is_some() {
                return;
            }
            let methods = [(c"onState", c"(Ljava/lang/String;)V")];
            // SAFETY: the environment and the `me.plei.pixelet.gpu.Gpu` JNI called with.
            let Some(c) = (unsafe { $($jni)+::class(env, this, methods) }) else {
                return;
            };
            let _ = PXL_GPU_CLASS.set(c);
            $crate::native::state_on(Some(pxl_gpu_on_state));
        }

        /// # Safety
        /// Only JNI calls this, on a thread Java made.
        #[unsafe(no_mangle)]
        unsafe extern "system" fn Java_me_plei_pixelet_gpu_Gpu_state(
            env: *mut $crate::jni::sys::JNIEnv,
            _this: $crate::jni::sys::jobject,
        ) -> $crate::jni::sys::jstring {
            let Some(s) = $crate::native::state() else {
                return ::core::ptr::null_mut();
            };
            // SAFETY: the environment JNI called with, and the name's UTF-8.
            unsafe { $($jni)+::string(env, s.as_ptr(), s.len()) }
        }

        /// # Safety
        /// Only JNI calls this, with a live `android.view.Surface`.
        #[unsafe(no_mangle)]
        unsafe extern "system" fn Java_me_plei_pixelet_gpu_Gpu_surfaceBind(
            env: *mut $crate::jni::sys::JNIEnv,
            _this: $crate::jni::sys::jobject,
            handle: $crate::jni::sys::jint,
            surface: $crate::jni::sys::jobject,
        ) -> $crate::jni::sys::jint {
            // SAFETY: the environment and surface JNI called with.
            unsafe { $crate::jni::bind(env, handle as u32, surface, pxl_gpu_surface_bind) }
        }

        #[unsafe(no_mangle)]
        extern "system" fn Java_me_plei_pixelet_gpu_Gpu_surfaceUnbind(
            _env: *mut $crate::jni::sys::JNIEnv,
            _this: $crate::jni::sys::jobject,
            handle: $crate::jni::sys::jint,
        ) {
            pxl_gpu_surface_unbind(handle as u32);
        }

        #[unsafe(no_mangle)]
        extern "system" fn Java_me_plei_pixelet_gpu_Gpu_vsync(
            _env: *mut $crate::jni::sys::JNIEnv,
            _this: $crate::jni::sys::jobject,
        ) {
            pxl_gpu_vsync();
        }
    };
}
