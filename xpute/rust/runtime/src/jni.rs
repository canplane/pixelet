// xpute-runtime/jni.rs

//! This is a native core as an Android app reaches it. `native_core!` exports
//! the C ABI's calls under the names JNI looks for, as the `external` functions
//! of the Kotlin class `xpute.Core`, and this module holds what they share.
//! Only JNI's own values cross: a number, an array of numbers, and the whole
//! memory as one direct buffer, which the app hands its JavaScript unchanged.
//! JNI's and the NDK's bindings are re-exported (`sys`, `ndk`), so that a
//! program's macro expansion and xpute-gpu name them through this crate.

use std::sync::OnceLock;

pub use jni_sys as sys;
pub use ndk_sys as ndk;

use sys::{
    jclass, jint, jmethodID, jobject, jvalue, JNIEnv, JNINativeInterface_, JavaVM, JNI_OK,
    JNI_VERSION_1_6,
};
use xpute_kit::status::bug::OrBug;
use xpute_kit::status::errno::Errno;

use crate::native::{answer_with, notify_on, Core};

/// This is the table of JNI's functions that the environment of a call holds.
///
/// # Safety
/// `env` is the environment JNI called with.
pub unsafe fn env<'e>(env: *mut JNIEnv) -> &'e JNINativeInterface_ {
    // SAFETY: an environment points at its function table for the call.
    unsafe { &**env }
}

/// This holds the VM, a global reference to the class `xpute.Core`, and the IDs
/// of its static methods `call(I)I` and `onNotify()V`. The class is taken on a
/// thread Java made, because a thread the runtime made cannot find the app's
/// classes by name.
struct Class {
    vm: *mut JavaVM,
    class: jclass,
    call: jmethodID,
    on_notify: jmethodID,
}

// SAFETY: JNI's VM is one for the process and may be used from any thread; a
// global reference and a method ID stay valid in every thread until let go.
unsafe impl Send for Class {}
// SAFETY: as for `Send`: nothing in it is changed once it is made.
unsafe impl Sync for Class {}

static CLASS: OnceLock<Class> = OnceLock::new();

/// Takes the class `xpute.Core` from `this`, once, and makes its static methods
/// answer the core's calls (`call`) and hear the core's news (`notify`).
///
/// # Safety
/// `e` and `this` are what JNI called with, `this` an `xpute.Core`.
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
        let method = |name: &core::ffi::CStr, sig: &core::ffi::CStr| {
            j.GetStaticMethodID.or_bug(Errno::ENOSYS)(e, class, name.as_ptr(), sig.as_ptr())
        };
        let call = method(c"call", c"(I)I");
        let on_notify = method(c"onNotify", c"()V");
        if call.is_null() || on_notify.is_null() {
            j.ExceptionClear.or_bug(Errno::ENOSYS)(e);
            return;
        }
        let _ = CLASS.set(Class {
            vm,
            class,
            call,
            on_notify,
        });
    }
    answer_with(Some(call));
    notify_on(Some(notify));
}

/// Calls `Core.call` on the thread that answers the core's call
/// (`answer_with`). Java made that thread, so it is attached to the VM. This
/// returns ENOSYS on a thread JNI does not know, and EIO when Kotlin threw.
extern "C" fn call(at: u32) -> i32 {
    let Some(&Class {
        vm, class, call, ..
    }) = CLASS.get()
    else {
        return Errno::ENOSYS as i32;
    };
    let mut e: *mut JNIEnv = core::ptr::null_mut();
    // SAFETY: the VM GetJavaVM gave, which lives as long as the process; the
    // class `init` holds globally, and the ID of its `call(I)I`.
    unsafe {
        let get = (**vm).GetEnv.or_bug(Errno::ENOSYS);
        if get(vm, (&raw mut e).cast(), JNI_VERSION_1_6) != JNI_OK {
            return Errno::ENOSYS as i32;
        }
        let j = env(e);
        let res =
            j.CallStaticIntMethodA.or_bug(Errno::ENOSYS)(e, class, call, &jvalue { i: at as jint });
        if j.ExceptionCheck.or_bug(Errno::ENOSYS)(e) != 0 {
            j.ExceptionClear.or_bug(Errno::ENOSYS)(e);
            return Errno::EIO as i32;
        }
        res
    }
}

/// Calls `Core.onNotify` from the core's thread (`notify_on`). JNI may be
/// called only from a thread attached to the VM, so the core's thread is
/// attached for good, as a daemon: it lives as long as the core does, and a
/// daemon thread keeps no VM from exiting.
extern "C" fn notify() {
    let Some(&Class {
        vm,
        class,
        on_notify,
        ..
    }) = CLASS.get()
    else {
        return;
    };
    let mut e: *mut JNIEnv = core::ptr::null_mut();
    // SAFETY: the VM GetJavaVM gave, which lives as long as the process; the
    // class `init` holds globally, and the ID of its `onNotify()V`.
    unsafe {
        let attach = (**vm).AttachCurrentThreadAsDaemon.or_bug(Errno::ENOSYS);
        if attach(vm, (&raw mut e).cast(), core::ptr::null_mut()) != JNI_OK {
            return;
        }
        let j = env(e);
        j.CallStaticVoidMethodA.or_bug(Errno::ENOSYS)(e, class, on_notify, core::ptr::null());
        // An exception Kotlin threw has no Java caller here to reach, so it is
        // cleared.
        if j.ExceptionCheck.or_bug(Errno::ENOSYS)(e) != 0 {
            j.ExceptionClear.or_bug(Errno::ENOSYS)(e);
        }
    }
}

/// A Java `byte[]`'s bytes, copied out.
///
/// # Safety
/// `e` is this thread's environment, and `a` a live `byte[]`.
pub unsafe fn bytes(e: *mut JNIEnv, a: jni_sys::jbyteArray) -> Vec<u8> {
    // SAFETY: as the caller says; the region is the array's whole length.
    unsafe {
        let j = env(e);
        let n = j.GetArrayLength.or_bug(Errno::ENOSYS)(e, a);
        let mut out = vec![0u8; n as usize];
        j.GetByteArrayRegion.or_bug(Errno::ENOSYS)(e, a, 0, n, out.as_mut_ptr().cast());
        out
    }
}

/// `bytes` as a new Java `byte[]`.
///
/// # Safety
/// `e` is this thread's environment.
pub unsafe fn byte_array(e: *mut JNIEnv, bytes: &[u8]) -> jni_sys::jbyteArray {
    // SAFETY: as the caller says; the region is the new array's whole length.
    unsafe {
        let j = env(e);
        let n = bytes.len() as jni_sys::jint;
        let out = j.NewByteArray.or_bug(Errno::ENOSYS)(e, n);
        if !out.is_null() {
            j.SetByteArrayRegion.or_bug(Errno::ENOSYS)(e, out, 0, n, bytes.as_ptr().cast());
        }
        out
    }
}

/// This is `Core.mem`: the memory, as a direct buffer over the whole of it.
///
/// # Safety
/// `e` is the environment JNI called with.
pub unsafe fn mem(e: *mut JNIEnv, core: Core) -> jobject {
    let (at, bytes) = core.mem();
    // SAFETY: the environment JNI called this with.
    let make = unsafe { env(e) }.NewDirectByteBuffer.or_bug(Errno::ENOSYS);
    // SAFETY: the memory lives as long as the process, and is `bytes` long.
    unsafe { make(e, at.as_ptr().cast(), bytes as i64) }
}
