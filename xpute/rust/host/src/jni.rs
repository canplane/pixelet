// xpute-host/jni.rs

//! A native guest as an Android app reaches it: the C ABI's calls under the
//! names JNI looks for, for the Kotlin class `xpute.Guest`, whose `external`
//! functions they are (`native_guest!`). Only JNI's own values cross: a
//! number, an array of numbers, and the memory as a direct buffer over the
//! whole of it, which the app hands its JavaScript as it is. JNI's and the
//! NDK's bindings are re-exported (`sys`, `ndk`), so a program's expansion
//! and xpute-gpu's name them through this crate.

use std::sync::OnceLock;

pub use jni_sys as sys;
pub use ndk_sys as ndk;

use sys::{
    jclass, jint, jmethodID, jobject, jvalue, JNIEnv, JNINativeInterface_, JavaVM, JNI_OK,
    JNI_VERSION_1_6,
};
use xpute_kit::status::bug::OrBug;
use xpute_kit::status::errno::Errno;

use crate::native::{answer_with, ack as ack_of, notify_on, Guest};

/// JNI's functions, as the environment a call is given holds them.
///
/// # Safety
/// `env` is the environment JNI called with.
pub unsafe fn env<'e>(env: *mut JNIEnv) -> &'e JNINativeInterface_ {
    // SAFETY: an environment points at its function table for the call.
    unsafe { &**env }
}

/// The VM, `xpute.Guest`'s class held past the call, and its static
/// `hostcall(I)I` and `onNotify()V`. Taken on a thread Java made, since a
/// thread the host made finds no class of the app's by name.
struct Class {
    vm: *mut JavaVM,
    class: jclass,
    hostcall: jmethodID,
    on_notify: jmethodID,
}

// SAFETY: JNI's VM is one for the process and may be used from any thread; a
// global reference and a method ID stay valid in every thread until let go.
unsafe impl Send for Class {}
// SAFETY: as for `Send`: nothing in it is changed once it is made.
unsafe impl Sync for Class {}

static CLASS: OnceLock<Class> = OnceLock::new();

/// `xpute.Guest`'s class taken from `this`, once, and its methods made what
/// answers the guest's hostcalls (`hostcall`) and is told the guest's news
/// (`notify`).
///
/// # Safety
/// `e` and `this` are what JNI called with, `this` an `xpute.Guest`.
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
        let hostcall = method(c"hostcall", c"(I)I");
        let on_notify = method(c"onNotify", c"()V");
        if hostcall.is_null() || on_notify.is_null() {
            j.ExceptionClear.or_bug(Errno::ENOSYS)(e);
            return;
        }
        let _ = CLASS.set(Class {
            vm,
            class,
            hostcall,
            on_notify,
        });
    }
    answer_with(Some(hostcall));
    notify_on(Some(notify));
}

/// `Guest.hostcall`, on the thread that answers it, one Java made and so
/// attached (`answer_with`): ENOSYS on a thread JNI does not know, EIO where
/// Kotlin threw.
extern "C" fn hostcall(at: u32) -> i32 {
    let Some(&Class {
        vm,
        class,
        hostcall,
        ..
    }) = CLASS.get()
    else {
        return Errno::ENOSYS as i32;
    };
    let mut e: *mut JNIEnv = core::ptr::null_mut();
    // SAFETY: the VM GetJavaVM gave, which lives as long as the process; the
    // class and method JNI gave in `hold`, the class held.
    unsafe {
        let get = (**vm).GetEnv.or_bug(Errno::ENOSYS);
        if get(vm, (&raw mut e).cast(), JNI_VERSION_1_6) != JNI_OK {
            return Errno::ENOSYS as i32;
        }
        let j = env(e);
        let res = j.CallStaticIntMethodA.or_bug(Errno::ENOSYS)(
            e,
            class,
            hostcall,
            &jvalue { i: at as jint },
        );
        if j.ExceptionCheck.or_bug(Errno::ENOSYS)(e) != 0 {
            j.ExceptionClear.or_bug(Errno::ENOSYS)(e);
            return Errno::EIO as i32;
        }
        res
    }
}

/// `Guest.onNotify`, from the guest's thread (`notify_on`). JNI may be called
/// only from a thread attached to the VM, so the thread is attached, for good
/// and as a daemon: it lives as long as the guest does, and a daemon keeps no
/// VM from exiting.
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
    // class and method JNI gave in `hold`, the class held.
    unsafe {
        let attach = (**vm).AttachCurrentThreadAsDaemon.or_bug(Errno::ENOSYS);
        if attach(vm, (&raw mut e).cast(), core::ptr::null_mut()) != JNI_OK {
            return;
        }
        let j = env(e);
        j.CallStaticVoidMethodA.or_bug(Errno::ENOSYS)(e, class, on_notify, core::ptr::null());
        // What Kotlin threw has no Java caller here to reach.
        if j.ExceptionCheck.or_bug(Errno::ENOSYS)(e) != 0 {
            j.ExceptionClear.or_bug(Errno::ENOSYS)(e);
        }
    }
}

/// `Guest.ack`: the wake and the longest turn since it was last asked
/// (`native::ack`), as a `double[]` of two.
///
/// # Safety
/// `e` is the environment JNI called with.
pub unsafe fn ack(e: *mut JNIEnv) -> jobject {
    let (wake, turn_ms) = ack_of();
    // SAFETY: the environment JNI called this with; an array of two made
    // here and filled with two.
    unsafe {
        let j = env(e);
        let out = j.NewDoubleArray.or_bug(Errno::ENOSYS)(e, 2);
        if !out.is_null() {
            j.SetDoubleArrayRegion.or_bug(Errno::ENOSYS)(e, out, 0, 2, [wake, turn_ms].as_ptr());
        }
        out
    }
}

/// `Guest.mem`: the memory, as a direct buffer over the whole of it.
///
/// # Safety
/// `e` is the environment JNI called with.
pub unsafe fn mem(e: *mut JNIEnv, guest: Guest) -> jobject {
    let (at, bytes) = guest.mem();
    // SAFETY: the environment JNI called this with.
    let make = unsafe { env(e) }.NewDirectByteBuffer.or_bug(Errno::ENOSYS);
    // SAFETY: the memory lives as long as the process, and is `bytes` long.
    unsafe { make(e, at.as_ptr().cast(), bytes as i64) }
}
