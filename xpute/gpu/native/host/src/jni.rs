// xpute-gpu-native-host/jni.rs

//! This is the GPU's host as an Android app reaches it: the C ABI's calls
//! under the names JNI looks for, as the `external` functions of the Kotlin
//! class `xpute.Gpu` (`jni_abi!`), and a device lost as a call of its static
//! `onLost()`. A view's `Surface` crosses as the native window wgpu draws
//! through (`target`), which the host holds from its bind until its unbind
//! (`native::bind`).

use std::ptr::NonNull;
use std::sync::OnceLock;

use xpute_kit::status::bug::OrBug;
use xpute_kit::status::errno::Errno;
use xpute_native_runtime::jni::env;
use xpute_native_runtime::jni::ndk::{ANativeWindow_fromSurface, ANativeWindow_release};
use xpute_native_runtime::jni::sys::{jclass, jint, jmethodID, jobject, JNIEnv, JavaVM, JNI_OK};

use super::native::lost_on;

/// This holds the VM, a global reference to the class `xpute.Gpu`, and the ID
/// of its static method `onLost()V`. The class is taken on a thread Java made,
/// because a thread the host made cannot find the app's classes by name.
struct Class {
    vm: *mut JavaVM,
    class: jclass,
    on_lost: jmethodID,
}

// SAFETY: JNI's VM is one for the process and may be used from any thread; a
// global reference and a method ID stay valid in every thread until let go.
unsafe impl Send for Class {}
// SAFETY: as for `Send`: nothing in it is changed once it is made.
unsafe impl Sync for Class {}

static CLASS: OnceLock<Class> = OnceLock::new();

/// Takes the class `xpute.Gpu` from `this`, once, and makes its static method
/// hear of each device lost (`lost_on`).
///
/// # Safety
/// `e` and `this` are what JNI called with, on a thread Java made, `this` an
/// `xpute.Gpu`.
pub unsafe fn init(e: *mut JNIEnv, this: jobject) {
    if CLASS.get().is_some() {
        return;
    }
    // SAFETY: the environment JNI called this with.
    let j = unsafe { env(e) };
    let mut vm: *mut JavaVM = core::ptr::null_mut();
    // SAFETY: JNI's own calls on the environment it called with; the class
    // is held globally, so it outlives this call.
    unsafe {
        if j.GetJavaVM.or_bug(Errno::ENOSYS)(e, &raw mut vm) != JNI_OK {
            return;
        }
        let class = j.NewGlobalRef.or_bug(Errno::ENOSYS)(
            e,
            j.GetObjectClass.or_bug(Errno::ENOSYS)(e, this),
        );
        let on_lost = j.GetStaticMethodID.or_bug(Errno::ENOSYS)(
            e,
            class,
            c"onLost".as_ptr(),
            c"()V".as_ptr(),
        );
        if on_lost.is_null() {
            j.ExceptionClear.or_bug(Errno::ENOSYS)(e);
            return;
        }
        let _ = CLASS.set(Class { vm, class, on_lost });
    }
    lost_on(Some(lost));
}

/// Calls `Gpu.onLost` from the thread that found the loss (`lost_on`), the
/// GPU's (native.rs). JNI may be called only from a thread
/// attached to the VM, so that thread is attached for good, as a daemon, as
/// the guest's thread is for `Runtime.onNotify`.
extern "C" fn lost() {
    let Some(&Class { vm, class, on_lost }) = CLASS.get() else {
        return;
    };
    let mut e: *mut JNIEnv = core::ptr::null_mut();
    // SAFETY: the VM GetJavaVM gave, which lives as long as the process; the
    // class `init` holds globally, and the ID of its `onLost()V`.
    unsafe {
        let attach = (**vm).AttachCurrentThreadAsDaemon.or_bug(Errno::ENOSYS);
        if attach(vm, (&raw mut e).cast(), core::ptr::null_mut()) != JNI_OK {
            return;
        }
        let j = env(e);
        j.CallStaticVoidMethodA.or_bug(Errno::ENOSYS)(e, class, on_lost, core::ptr::null());
        // An exception Kotlin threw has no Java caller here to reach, so it is
        // cleared.
        if j.ExceptionCheck.or_bug(Errno::ENOSYS)(e) != 0 {
            j.ExceptionClear.or_bug(Errno::ENOSYS)(e);
        }
    }
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
    bind: unsafe fn(u32, *mut core::ffi::c_void) -> i32,
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
/// `onLost()` hear each device lost (`init`), `surfaceBind(handle: Int,
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
