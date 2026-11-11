// xpute-gpu/runtime/jni.rs

//! This is the GPU's runtime as an Android app reaches it: the C ABI's calls
//! under the names JNI looks for, as the `external` functions of the Kotlin
//! class `xpute.Gpu` (`native_abi!`), and a device lost as a call of its static
//! `onLost()`. A view's `Surface` crosses as the native window wgpu draws
//! through (`target`), which the runtime holds from its bind until its unbind
//! (`native::bind`).

use std::ffi::CStr;
use std::path::PathBuf;
use std::ptr::NonNull;
use std::sync::OnceLock;

use xpute_kit::status::bug::OrBug;
use xpute_kit::status::errno::Errno;
use xpute_runtime::jni::env;
use xpute_runtime::jni::ndk::{ANativeWindow_fromSurface, ANativeWindow_release};
use xpute_runtime::jni::sys::{jclass, jint, jmethodID, jobject, jstring, JNIEnv, JavaVM, JNI_OK};

use super::native::lost_on;

/// This holds the VM, a global reference to the class `xpute.Gpu`, and the ID
/// of its static method `onLost()V`. The class is taken on a thread Java made,
/// because a thread the runtime made cannot find the app's classes by name.
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

/// Calls `Gpu.onLost` from the thread that found the loss (`lost_on`): a
/// turn's, or the one the stream runs on. JNI may be called only from a thread
/// attached to the VM, so that thread is attached for good, as a daemon, as
/// the core's thread is for `Core.onNotify`.
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

/// This is `Gpu.surfaceBind`: it binds the core's surface `handle` to a view's
/// `surface`, through the native window the surface is (`bind`, the program's
/// `xpute_gpu_surface_bind`), which holds its own reference on the window. It
/// returns 0, `NOT_OPEN`, `NO_SURFACE` or `SLOT_TAKEN`.
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
        return crate::runtime::native::NO_SURFACE;
    };
    // SAFETY: a live window, on which `bind` takes a reference of its own.
    let bound = unsafe { bind(handle, window.as_ptr().cast()) };
    // SAFETY: the reference `ANativeWindow_fromSurface` gave this call.
    unsafe { ANativeWindow_release(window.as_ptr()) };
    bound
}

/// The path the Java string `s` names, or none for a null string.
///
/// # Safety
/// `e` is what JNI called with, and `s` null or a live `java.lang.String`.
pub unsafe fn path(e: *mut JNIEnv, s: jstring) -> Option<PathBuf> {
    if s.is_null() {
        return None;
    }
    // SAFETY: the environment JNI called with.
    let f = unsafe { xpute_runtime::jni::env(e) };
    let (get, release) = (f.GetStringUTFChars?, f.ReleaseStringUTFChars?);
    // SAFETY: a live string; the chars it gives are released below.
    let chars = unsafe { get(e, s, core::ptr::null_mut()) };
    if chars.is_null() {
        return None;
    }
    // SAFETY: the string's modified UTF-8, NUL-terminated, until released.
    let path = unsafe { CStr::from_ptr(chars) }
        .to_str()
        .ok()
        .map(PathBuf::from);
    // SAFETY: the chars `get` gave for `s`.
    unsafe { release(e, s, chars) };
    path
}
