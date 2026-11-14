// xpute-native-host/jni.rs

//! The runtime and its host as an Android app reaches them: the C ABI's calls
//! under the names JNI looks for, as the `external` functions of the Kotlin
//! class `xpute.Runtime` (`jni_abi!`), and what the app hears as calls of its
//! static `onNotify()`, `onStopped(…)` and `onTrap()`, from the threads
//! that tell it, each attached to the VM for good as a daemon.

use std::path::PathBuf;
use std::sync::OnceLock;

use xpute_kit::status::bug::OrBug;
use xpute_kit::status::errno::Errno;
use xpute_native_runtime::jni::sys::{jbyteArray, jint, jintArray, jobject, jstring, jvalue, JNIEnv};
use xpute_native_runtime::jni::{call, class, env, Class};

use crate::native::{notify_on, stopped_on, trap_on};

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
    methods[ON_TRAP] = (c"onTrap", c"()V");
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
    unsafe { call(c, ON_NOTIFY, |_| Vec::new()) };
}

extern "C" fn trap() {
    let Some(c) = CLASS.get() else { return };
    // SAFETY: `onTrap()V`, at its index.
    unsafe { call(c, ON_TRAP, |_| Vec::new()) };
}

extern "C" fn stopped(errnum: i32, file: *const u8, len: usize, line: u32, n1: u64, n2: u64) {
    let Some(c) = CLASS.get() else { return };
    // SAFETY: the file's UTF-8, the stop's for the call.
    let file = String::from_utf8_lossy(unsafe { core::slice::from_raw_parts(file, len) }).into_owned();
    let file = std::ffi::CString::new(file.replace('\0', " ")).unwrap_or_default();
    let args = |e| {
        // SAFETY: this thread's environment, and a NUL-terminated string.
        let s = unsafe { env(e).NewStringUTF.or_bug(Errno::ENOSYS)(e, file.as_ptr()) };
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
    unsafe { call(c, ON_STOPPED, args) };
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

/// The batch the UI has not taken, as a Java `byte[]`.
///
/// # Safety
/// `e` is what JNI called with.
pub unsafe fn take(e: *mut JNIEnv) -> jbyteArray {
    let b = xpute_native_runtime::remote::take();
    // SAFETY: JNI's own calls; the array is the caller's local reference.
    unsafe {
        let j = env(e);
        let out = j.NewByteArray.or_bug(Errno::ENOSYS)(e, b.len() as jint);
        j.SetByteArrayRegion.or_bug(Errno::ENOSYS)(e, out, 0, b.len() as jint, b.as_ptr().cast());
        out
    }
}

/// The core's memory as a direct `ByteBuffer`, with no copy.
///
/// # Safety
/// `e` is what JNI called with.
pub unsafe fn mem(e: *mut JNIEnv) -> jobject {
    let (at, len) = crate::native::mem();
    // SAFETY: the region, the guest's for the process's life.
    unsafe { env(e).NewDirectByteBuffer.or_bug(Errno::ENOSYS)(e, at.cast(), i64::from(len)) }
}

/// Exports the runtime and its host to JNI for the Kotlin class
/// `xpute.Runtime`, which `native_abi!` expands on Android: `init()`,
/// `start(named: IntArray, prefixes: ByteArray, filesDir: String, cacheDir:
/// String)`, `post(tag: Int, h: Int, m: Int, pkt: ByteArray?)`, `answer(tag:
/// Int, result: Int, data: ByteArray?)`, `take(): ByteArray`, `mem():
/// ByteBuffer` and `life(shown: Boolean)`.
#[macro_export]
macro_rules! jni_abi {
    ($start:path) => {
        /// # Safety
        /// Only JNI calls this, on a thread Java made.
        #[no_mangle]
        unsafe extern "system" fn Java_xpute_Runtime_init(
            env: *mut ::xpute_native_runtime::jni::sys::JNIEnv,
            this: ::xpute_native_runtime::jni::sys::jobject,
        ) {
            // SAFETY: the environment and the `xpute.Runtime` JNI called with.
            unsafe { $crate::jni::init(env, this) }
        }

        /// # Safety
        /// Only JNI calls this, with live arrays and strings.
        #[no_mangle]
        unsafe extern "system" fn Java_xpute_Runtime_start(
            env: *mut ::xpute_native_runtime::jni::sys::JNIEnv,
            _this: ::xpute_native_runtime::jni::sys::jobject,
            named: ::xpute_native_runtime::jni::sys::jintArray,
            prefixes: ::xpute_native_runtime::jni::sys::jbyteArray,
            files_dir: ::xpute_native_runtime::jni::sys::jstring,
            cache_dir: ::xpute_native_runtime::jni::sys::jstring,
        ) {
            // SAFETY: what JNI called with.
            unsafe {
                let named = $crate::jni::handles(env, named);
                let prefixes = $crate::jni::bytes(env, prefixes).unwrap_or_default();
                let (files_dir, cache_dir) = ($crate::jni::dir(env, files_dir), $crate::jni::dir(env, cache_dir));
                $crate::native::start($start, named, &prefixes, files_dir, cache_dir);
            }
        }

        /// # Safety
        /// Only JNI calls this, `pkt` null or a live byte array.
        #[no_mangle]
        unsafe extern "system" fn Java_xpute_Runtime_post(
            env: *mut ::xpute_native_runtime::jni::sys::JNIEnv,
            _this: ::xpute_native_runtime::jni::sys::jobject,
            tag: ::xpute_native_runtime::jni::sys::jint,
            h: ::xpute_native_runtime::jni::sys::jint,
            m: ::xpute_native_runtime::jni::sys::jint,
            pkt: ::xpute_native_runtime::jni::sys::jbyteArray,
        ) {
            // SAFETY: what JNI called with.
            let pkt = unsafe { $crate::jni::bytes(env, pkt) };
            ::xpute_native_runtime::post(tag as u32, h as u32, m as u32, pkt);
        }

        /// # Safety
        /// Only JNI calls this, `data` null or a live byte array.
        #[no_mangle]
        unsafe extern "system" fn Java_xpute_Runtime_answer(
            env: *mut ::xpute_native_runtime::jni::sys::JNIEnv,
            _this: ::xpute_native_runtime::jni::sys::jobject,
            tag: ::xpute_native_runtime::jni::sys::jint,
            result: ::xpute_native_runtime::jni::sys::jint,
            data: ::xpute_native_runtime::jni::sys::jbyteArray,
        ) {
            // SAFETY: what JNI called with.
            let data = unsafe { $crate::jni::bytes(env, data) };
            ::xpute_native_runtime::answer(tag as u32, result, data);
        }

        /// # Safety
        /// Only JNI calls this.
        #[no_mangle]
        unsafe extern "system" fn Java_xpute_Runtime_take(
            env: *mut ::xpute_native_runtime::jni::sys::JNIEnv,
            _this: ::xpute_native_runtime::jni::sys::jobject,
        ) -> ::xpute_native_runtime::jni::sys::jbyteArray {
            // SAFETY: the environment JNI called with.
            unsafe { $crate::jni::take(env) }
        }

        /// # Safety
        /// Only JNI calls this.
        #[no_mangle]
        unsafe extern "system" fn Java_xpute_Runtime_mem(
            env: *mut ::xpute_native_runtime::jni::sys::JNIEnv,
            _this: ::xpute_native_runtime::jni::sys::jobject,
        ) -> ::xpute_native_runtime::jni::sys::jobject {
            // SAFETY: the environment JNI called with.
            unsafe { $crate::jni::mem(env) }
        }

        #[no_mangle]
        extern "system" fn Java_xpute_Runtime_life(
            _env: *mut ::xpute_native_runtime::jni::sys::JNIEnv,
            _this: ::xpute_native_runtime::jni::sys::jobject,
            shown: ::xpute_native_runtime::jni::sys::jboolean,
        ) {
            ::xpute_native_runtime::life(shown != 0);
        }
    };
}
