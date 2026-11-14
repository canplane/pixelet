// xpute-native-runtime/jni.rs

//! JNI's and the NDK's bindings as an Android app's native side reaches the
//! runtime, its host and what they serve, re-exported (`sys`, `ndk`) so that a device
//! beside the guest names them through this crate; and a Kotlin class whose
//! static methods a thread of the runtime or a host calls (`Class`, `call`,
//! `Return`), a string the C ABI gives among their values (`string`). The
//! runtime itself reaches the app through them: the C ABI's calls under the
//! names JNI looks for, as the `external` functions of the Kotlin class
//! `xpute.Runtime` (`jni_abi!`), and what the app hears as calls of its
//! static `onNotify()`, `onStopped(…)` and `onTrap(…)`, from the threads that
//! tell it, a thread the runtime made attached to the VM for good as a
//! daemon, and a stop and a trap again from the Java thread of a start after
//! them (native.rs `start`).

use core::ffi::CStr;
use std::path::PathBuf;
use std::sync::OnceLock;

use xpute_kit::status::bug::OrBug;
use xpute_kit::status::errno::Errno;

pub use jni_sys as sys;
pub use ndk_sys as ndk;

use sys::{jbyteArray, jint, jintArray, jobject, jstring, jvalue, JNIEnv};

use crate::native::{notify_on, stopped_on, trap_on};

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

/// The class `xpute.Runtime` and the static methods the app hears by, at
/// `ON_NOTIFY`, `ON_STOPPED` and `ON_TRAP`.
static CLASS: OnceLock<Class<3>> = OnceLock::new();
const ON_NOTIFY: usize = 0;
const ON_STOPPED: usize = 1;
const ON_TRAP: usize = 2;

/// Takes the class `xpute.Runtime` from `this`, once, and makes its static
/// methods hear a batch waiting, a stop and a trap.
///
/// # Safety
/// `e` and `this` are what JNI called with, on a thread Java made, `this` an
/// `xpute.Runtime`.
pub unsafe fn init(e: *mut JNIEnv, this: jobject) {
    if CLASS.get().is_some() {
        return;
    }
    let mut methods = [(c"", c""); 3];
    methods[ON_NOTIFY] = (c"onNotify", c"()V");
    methods[ON_STOPPED] = (c"onStopped", c"(ILjava/lang/String;IJJ)V");
    methods[ON_TRAP] = (c"onTrap", c"(Ljava/lang/String;)V");
    // SAFETY: what JNI called with, on a thread Java made.
    let Some(c) = (unsafe { class(e, this, methods) }) else {
        return;
    };
    let _ = CLASS.set(c);
    notify_on(Some(notify));
    stopped_on(Some(stopped));
    trap_on(Some(trap));
}

extern "C" fn notify() {
    let Some(c) = CLASS.get() else { return };
    // SAFETY: `onNotify()V`, at its index.
    unsafe { call(c, ON_NOTIFY, |_| Vec::new()) }
}

extern "C" fn trap(err: *const u8, len: usize) {
    let Some(c) = CLASS.get() else { return };
    let args = |e| {
        // SAFETY: this thread's environment, and the trap's UTF-8, the
        // trap's for the call.
        let s = unsafe { string(e, err, len) };
        vec![jvalue { l: s }]
    };
    // SAFETY: `onTrap(Ljava/lang/String;)V`, at its index, and its one value.
    unsafe { call(c, ON_TRAP, args) }
}

extern "C" fn stopped(errnum: i32, file: *const u8, len: usize, line: u32, n1: u64, n2: u64) {
    let Some(c) = CLASS.get() else { return };
    let args = |e| {
        // SAFETY: this thread's environment, and the file's UTF-8, the stop's
        // for the call.
        let s = unsafe { string(e, file, len) };
        vec![
            jvalue { i: errnum },
            jvalue { l: s },
            jvalue { i: line as jint },
            jvalue { j: n1 as i64 },
            jvalue { j: n2 as i64 },
        ]
    };
    // SAFETY: `onStopped(ILjava/lang/String;IJJ)V`, at its index, and its
    // five values in order.
    unsafe { call(c, ON_STOPPED, args) }
}

/// The bytes of a Java `byte[]`, or none for null.
///
/// # Safety
/// `e` is what JNI called with, and `a` null or a live byte array.
pub unsafe fn bytes(e: *mut JNIEnv, a: jbyteArray) -> Option<Vec<u8>> {
    if a.is_null() {
        return None;
    }
    // SAFETY: JNI's array, its own length.
    unsafe {
        let j = env(e);
        let len = j.GetArrayLength.or_bug(Errno::ENOSYS)(e, a);
        let mut v = vec![0u8; usize::try_from(len).unwrap_or(0)];
        j.GetByteArrayRegion.or_bug(Errno::ENOSYS)(e, a, 0, len, v.as_mut_ptr().cast());
        Some(v)
    }
}

/// The handles of a Java `int[]`.
///
/// # Safety
/// `e` is what JNI called with, and `a` a live int array.
pub unsafe fn handles(e: *mut JNIEnv, a: jintArray) -> Vec<u32> {
    // SAFETY: JNI's array, its own length.
    unsafe {
        let j = env(e);
        let len = j.GetArrayLength.or_bug(Errno::ENOSYS)(e, a);
        let mut v = vec![0i32; usize::try_from(len).unwrap_or(0)];
        j.GetIntArrayRegion.or_bug(Errno::ENOSYS)(e, a, 0, len, v.as_mut_ptr());
        v.into_iter().map(|h| h as u32).collect()
    }
}

/// A directory a Java `String` names.
///
/// # Safety
/// `e` is what JNI called with, and `s` a live string.
pub unsafe fn dir(e: *mut JNIEnv, s: jstring) -> PathBuf {
    // SAFETY: JNI's string, its UTF released after the copy.
    unsafe {
        let j = env(e);
        let at = j.GetStringUTFChars.or_bug(Errno::ENOSYS)(e, s, core::ptr::null_mut());
        let dir = PathBuf::from(core::ffi::CStr::from_ptr(at).to_string_lossy().into_owned());
        j.ReleaseStringUTFChars.or_bug(Errno::ENOSYS)(e, s, at);
        dir
    }
}

/// The batch the UI of `generation` has not taken, as a Java `byte[]`.
///
/// # Safety
/// `e` is what JNI called with.
pub unsafe fn take(e: *mut JNIEnv, generation: u32) -> jbyteArray {
    let b = crate::remote::take(generation);
    // SAFETY: JNI's own calls; the array is the caller's local reference.
    unsafe {
        let j = env(e);
        let out = j.NewByteArray.or_bug(Errno::ENOSYS)(e, b.len() as jint);
        j.SetByteArrayRegion.or_bug(Errno::ENOSYS)(e, out, 0, b.len() as jint, b.as_ptr().cast());
        out
    }
}

/// The guest's memory as a direct `ByteBuffer`, with no copy.
///
/// # Safety
/// `e` is what JNI called with.
pub unsafe fn mem(e: *mut JNIEnv) -> jobject {
    let (at, len) = crate::native::mem();
    // SAFETY: the region, the guest's for the process's life.
    unsafe { env(e).NewDirectByteBuffer.or_bug(Errno::ENOSYS)(e, at.cast(), i64::from(len)) }
}

/// Exports the runtime to JNI for the Kotlin class
/// `xpute.Runtime`, which `native_abi!` expands on Android: `init()`,
/// `start(named: IntArray, prefixes: ByteArray, cacheDir: String): Int`,
/// which answers the UI's generation, `post(generation: Int,
/// tag: Int, h: Int, m: Int, pkt: ByteArray?)`, `answer(generation: Int, tag:
/// Int, result: Int, data: ByteArray?)`, `take(generation: Int): ByteArray`,
/// `mem(): ByteBuffer` and `life(shown: Boolean)`.
#[macro_export]
macro_rules! jni_abi {
    ($start:path) => {
        /// # Safety
        /// Only JNI calls this, on a thread Java made.
        #[no_mangle]
        unsafe extern "system" fn Java_xpute_Runtime_init(
            env: *mut $crate::jni::sys::JNIEnv,
            this: $crate::jni::sys::jobject,
        ) {
            // SAFETY: the environment and the `xpute.Runtime` JNI called with.
            unsafe { $crate::jni::init(env, this) }
        }

        /// # Safety
        /// Only JNI calls this, with live arrays and strings.
        #[no_mangle]
        unsafe extern "system" fn Java_xpute_Runtime_start(
            env: *mut $crate::jni::sys::JNIEnv,
            _this: $crate::jni::sys::jobject,
            named: $crate::jni::sys::jintArray,
            prefixes: $crate::jni::sys::jbyteArray,
            cache_dir: $crate::jni::sys::jstring,
        ) -> $crate::jni::sys::jint {
            // SAFETY: what JNI called with.
            unsafe {
                let named = $crate::jni::handles(env, named);
                let prefixes = $crate::jni::bytes(env, prefixes).unwrap_or_default();
                let cache_dir = $crate::jni::dir(env, cache_dir);
                $crate::native::start($start, named, &prefixes, cache_dir) as $crate::jni::sys::jint
            }
        }

        /// # Safety
        /// Only JNI calls this, `pkt` null or a live byte array.
        #[no_mangle]
        unsafe extern "system" fn Java_xpute_Runtime_post(
            env: *mut $crate::jni::sys::JNIEnv,
            _this: $crate::jni::sys::jobject,
            generation: $crate::jni::sys::jint,
            tag: $crate::jni::sys::jint,
            h: $crate::jni::sys::jint,
            m: $crate::jni::sys::jint,
            pkt: $crate::jni::sys::jbyteArray,
        ) {
            // SAFETY: what JNI called with.
            let pkt = unsafe { $crate::jni::bytes(env, pkt) };
            $crate::post(generation as u32, tag as u32, h as u32, m as u32, pkt);
        }

        /// # Safety
        /// Only JNI calls this, `data` null or a live byte array.
        #[no_mangle]
        unsafe extern "system" fn Java_xpute_Runtime_answer(
            env: *mut $crate::jni::sys::JNIEnv,
            _this: $crate::jni::sys::jobject,
            generation: $crate::jni::sys::jint,
            tag: $crate::jni::sys::jint,
            result: $crate::jni::sys::jint,
            data: $crate::jni::sys::jbyteArray,
        ) {
            // SAFETY: what JNI called with.
            let data = unsafe { $crate::jni::bytes(env, data) };
            $crate::answer(generation as u32, tag as u32, result, data);
        }

        /// # Safety
        /// Only JNI calls this.
        #[no_mangle]
        unsafe extern "system" fn Java_xpute_Runtime_take(
            env: *mut $crate::jni::sys::JNIEnv,
            _this: $crate::jni::sys::jobject,
            generation: $crate::jni::sys::jint,
        ) -> $crate::jni::sys::jbyteArray {
            // SAFETY: the environment JNI called with.
            unsafe { $crate::jni::take(env, generation as u32) }
        }

        /// # Safety
        /// Only JNI calls this.
        #[no_mangle]
        unsafe extern "system" fn Java_xpute_Runtime_mem(
            env: *mut $crate::jni::sys::JNIEnv,
            _this: $crate::jni::sys::jobject,
        ) -> $crate::jni::sys::jobject {
            // SAFETY: the environment JNI called with.
            unsafe { $crate::jni::mem(env) }
        }

        #[no_mangle]
        extern "system" fn Java_xpute_Runtime_life(
            _env: *mut $crate::jni::sys::JNIEnv,
            _this: $crate::jni::sys::jobject,
            shown: $crate::jni::sys::jboolean,
        ) {
            $crate::life(shown != 0);
        }
    };
}
