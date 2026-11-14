// xpute-native-runtime/jni.rs

//! JNI's and the NDK's bindings as an Android app's native side reaches the
//! runtime, its host and what they serve, re-exported (`sys`, `ndk`) so that a device
//! beside the guest names them through this crate; and a Kotlin class whose
//! static methods a thread of the runtime or a host calls (`Class`, `call`,
//! `Return`), a string the C ABI gives among their values (`string`).

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

/// A Java `String`, a local reference, of the `len` bytes of UTF-8 at `at`, as
/// a string crosses the C ABI. JNI takes its strings NUL-terminated, so a NUL
/// in it is a space, and an invalid sequence is U+FFFD.
///
/// # Safety
/// `e` is this thread's environment, and `at` `len` readable bytes.
pub unsafe fn string(e: *mut sys::JNIEnv, at: *const u8, len: usize) -> sys::jstring {
    // SAFETY: as the caller holds.
    let s = String::from_utf8_lossy(unsafe { core::slice::from_raw_parts(at, len) }).replace('\0', " ");
    let s = std::ffi::CString::new(s).unwrap_or_default();
    // SAFETY: this thread's environment, and a NUL-terminated string.
    unsafe { env(e).NewStringUTF.or_bug(Errno::ENOSYS)(e, s.as_ptr()) }
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
/// exception cleared and the class let go.
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
                j.DeleteGlobalRef.or_bug(Errno::ENOSYS)(e, class);
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

/// What a static method returns, each type by the JNI function that calls a
/// method returning it.
pub trait Return: Default {
    /// Calls the static method `m` of `class` with `args`.
    ///
    /// # Safety
    /// As JNI's `CallStatic…MethodA`: `e` is this thread's environment, `m`
    /// a static method of `class` that returns this type, and `args` the
    /// values of its signature.
    unsafe fn call(e: *mut sys::JNIEnv, class: sys::jclass, m: sys::jmethodID, args: *const sys::jvalue) -> Self;
}

impl Return for () {
    unsafe fn call(e: *mut sys::JNIEnv, class: sys::jclass, m: sys::jmethodID, args: *const sys::jvalue) {
        // SAFETY: as the caller holds.
        unsafe { env(e).CallStaticVoidMethodA.or_bug(Errno::ENOSYS)(e, class, m, args) }
    }
}

impl Return for f64 {
    unsafe fn call(e: *mut sys::JNIEnv, class: sys::jclass, m: sys::jmethodID, args: *const sys::jvalue) -> f64 {
        // SAFETY: as the caller holds.
        unsafe { env(e).CallStaticDoubleMethodA.or_bug(Errno::ENOSYS)(e, class, m, args) }
    }
}

/// Calls the static method `m` of `c`, its index in the names `class` was
/// given, with `args` on this thread, attached to the VM for good as a
/// daemon where Java has not attached it, and gives what it returns. The
/// references `args` makes live in a local frame of the call's, since a
/// thread the runtime made never returns to Java to let them go. An exception
/// Kotlin threw is cleared, as such a thread has no Java caller to reach, and
/// the call, as one that cannot attach or make its frame, gives `R`'s
/// default.
///
/// # Safety
/// `args` gives the values of the signature of the method at `m`, in its
/// order, and the method returns `R`.
pub unsafe fn call<const N: usize, R: Return>(
    c: &Class<N>,
    m: usize,
    args: impl FnOnce(*mut sys::JNIEnv) -> Vec<sys::jvalue>,
) -> R {
    let mut e: *mut sys::JNIEnv = core::ptr::null_mut();
    // SAFETY: the VM GetJavaVM gave, which lives as long as the process; the
    // class `class` holds globally, the ID of one of its static methods, and
    // the values its signature takes, as the caller holds.
    unsafe {
        let attach = (**c.vm).AttachCurrentThreadAsDaemon.or_bug(Errno::ENOSYS);
        if attach(c.vm, (&raw mut e).cast(), core::ptr::null_mut()) != sys::JNI_OK {
            return R::default();
        }
        let j = env(e);
        if j.PushLocalFrame.or_bug(Errno::ENOSYS)(e, 4) != 0 {
            j.ExceptionClear.or_bug(Errno::ENOSYS)(e);
            return R::default();
        }
        let args = args(e);
        let mut r = R::call(e, c.class, c.methods[m], args.as_ptr());
        if j.ExceptionCheck.or_bug(Errno::ENOSYS)(e) != 0 {
            j.ExceptionClear.or_bug(Errno::ENOSYS)(e);
            r = R::default();
        }
        j.PopLocalFrame.or_bug(Errno::ENOSYS)(e, core::ptr::null_mut());
        r
    }
}
