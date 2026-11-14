// xpute-native-runtime/jni.rs

//! JNI's and the NDK's bindings as an Android app's native side reaches the
//! runtime, its host and what they serve, re-exported (`sys`, `ndk`) so that a device
//! beside the guest names them through this crate; and a Kotlin class whose
//! static methods a thread of the runtime or a host calls (`Class`, `call`).

use core::ffi::CStr;

use xpute_kit::status::bug::OrBug;
use xpute_kit::status::errno::Errno;

pub use jni_sys as sys;
pub use ndk_sys as ndk;

/// This is the table of JNI's functions that the environment of a call holds.
///
/// # Safety
/// `env` is the environment JNI called with.
pub unsafe fn env<'e>(env: *mut sys::JNIEnv) -> &'e sys::JNINativeInterface_ {
    // SAFETY: an environment points at its function table for the call.
    unsafe { &**env }
}

/// The VM, a global reference to a Kotlin class, and the IDs of `N` of its
/// static methods, in the order `class` was given their names. The class is
/// taken on a thread Java made, because a thread the runtime or a host made
/// cannot find the app's classes by name.
pub struct Class<const N: usize> {
    vm: *mut sys::JavaVM,
    class: sys::jclass,
    methods: [sys::jmethodID; N],
}

// SAFETY: JNI's VM is one for the process and may be used from any thread; a
// global reference and a method ID stay valid in every thread until let go.
unsafe impl<const N: usize> Send for Class<N> {}
// SAFETY: as for `Send`: nothing in it is changed once it is made.
unsafe impl<const N: usize> Sync for Class<N> {}

/// The class of `this` and its static methods `methods`, each a name and a
/// signature, or none where JNI has no VM or the class no such method, the
/// exception cleared.
///
/// # Safety
/// `e` and `this` are what JNI called with, on a thread Java made.
pub unsafe fn class<const N: usize>(
    e: *mut sys::JNIEnv,
    this: sys::jobject,
    methods: [(&CStr, &CStr); N],
) -> Option<Class<N>> {
    // SAFETY: the environment JNI called this with.
    let j = unsafe { env(e) };
    let mut vm: *mut sys::JavaVM = core::ptr::null_mut();
    // SAFETY: JNI's own calls on the environment it called with; the class
    // is held globally, so it outlives this call.
    unsafe {
        if j.GetJavaVM.or_bug(Errno::ENOSYS)(e, &raw mut vm) != sys::JNI_OK {
            return None;
        }
        let class = j.NewGlobalRef.or_bug(Errno::ENOSYS)(e, j.GetObjectClass.or_bug(Errno::ENOSYS)(e, this));
        let mut ids = [core::ptr::null_mut(); N];
        for (id, (name, sig)) in ids.iter_mut().zip(methods) {
            *id = j.GetStaticMethodID.or_bug(Errno::ENOSYS)(e, class, name.as_ptr(), sig.as_ptr());
            if id.is_null() {
                j.ExceptionClear.or_bug(Errno::ENOSYS)(e);
                return None;
            }
        }
        Some(Class {
            vm,
            class,
            methods: ids,
        })
    }
}

/// Calls the static method `m` of `c`, its index in the names `class` was
/// given, with `args` on this thread, attached to the VM for good as a
/// daemon. An exception Kotlin threw has no Java caller here to reach, so it
/// is cleared.
///
/// # Safety
/// `args` gives the values of the signature of the method at `m`, in its
/// order.
pub unsafe fn call<const N: usize>(c: &Class<N>, m: usize, args: impl FnOnce(*mut sys::JNIEnv) -> Vec<sys::jvalue>) {
    let mut e: *mut sys::JNIEnv = core::ptr::null_mut();
    // SAFETY: the VM GetJavaVM gave, which lives as long as the process; the
    // class `class` holds globally, the ID of one of its static methods, and
    // the values its signature takes, as the caller holds.
    unsafe {
        let attach = (**c.vm).AttachCurrentThreadAsDaemon.or_bug(Errno::ENOSYS);
        if attach(c.vm, (&raw mut e).cast(), core::ptr::null_mut()) != sys::JNI_OK {
            return;
        }
        let j = env(e);
        let args = args(e);
        j.CallStaticVoidMethodA.or_bug(Errno::ENOSYS)(e, c.class, c.methods[m], args.as_ptr());
        if j.ExceptionCheck.or_bug(Errno::ENOSYS)(e) != 0 {
            j.ExceptionClear.or_bug(Errno::ENOSYS)(e);
        }
    }
}
