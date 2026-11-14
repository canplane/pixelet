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
use xpute_native_runtime::jni::env;
use xpute_native_runtime::jni::sys::{
    jbyteArray, jclass, jint, jintArray, jmethodID, jobject, jstring, jvalue, JNIEnv, JavaVM,
    JNI_OK,
};

use crate::native::{notify_on, stopped_on, trap_on};

/// The VM, a global reference to the class `xpute.Runtime`, and the IDs of
/// its static methods the app hears by. The class is taken on a thread Java
/// made, because a thread the runtime made cannot find the app's classes by
/// name.
struct Class {
    vm: *mut JavaVM,
    class: jclass,
    on_notify: jmethodID,
    on_stopped: jmethodID,
    on_trap: jmethodID,
}

// SAFETY: JNI's VM is one for the process and may be used from any thread; a
// global reference and a method ID stay valid in every thread until let go.
unsafe impl Send for Class {}
// SAFETY: as for `Send`: nothing in it is changed once it is made.
unsafe impl Sync for Class {}

static CLASS: OnceLock<Class> = OnceLock::new();

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
    // SAFETY: the environment JNI called this with.
    let j = unsafe { env(e) };
    let mut vm: *mut JavaVM = core::ptr::null_mut();
    // SAFETY: JNI's own calls on the environment it called with; the class
    // is held globally, so it outlives this call.
    unsafe {
        if j.GetJavaVM.or_bug(Errno::ENOSYS)(e, &raw mut vm) != JNI_OK {
            return;
        }
        let class = j.NewGlobalRef.or_bug(Errno::ENOSYS)(e, j.GetObjectClass.or_bug(Errno::ENOSYS)(e, this));
        let method = |name: &core::ffi::CStr, sig: &core::ffi::CStr| {
            j.GetStaticMethodID.or_bug(Errno::ENOSYS)(e, class, name.as_ptr(), sig.as_ptr())
        };
        let on_notify = method(c"onNotify", c"()V");
        let on_stopped = method(c"onStopped", c"(ILjava/lang/String;IJJ)V");
        let on_trap = method(c"onTrap", c"()V");
        if on_notify.is_null() || on_stopped.is_null() || on_trap.is_null() {
            j.ExceptionClear.or_bug(Errno::ENOSYS)(e);
            return;
        }
        let _ = CLASS.set(Class {
            vm,
            class,
            on_notify,
            on_stopped,
            on_trap,
        });
    }
    notify_on(Some(notify));
    stopped_on(Some(stopped));
    trap_on(Some(trap));
}

/// Calls the static method `m` with `args` on this thread, attached to the
/// VM for good as a daemon. An exception Kotlin threw has no Java caller here
/// to reach, so it is cleared.
fn call(m: impl FnOnce(&Class) -> jmethodID, args: impl FnOnce(*mut JNIEnv) -> Vec<jvalue>) {
    let Some(c) = CLASS.get() else { return };
    let mut e: *mut JNIEnv = core::ptr::null_mut();
    // SAFETY: the VM GetJavaVM gave, which lives as long as the process; the
    // class `init` holds globally, and the ID of one of its static methods.
    unsafe {
        let attach = (**c.vm).AttachCurrentThreadAsDaemon.or_bug(Errno::ENOSYS);
        if attach(c.vm, (&raw mut e).cast(), core::ptr::null_mut()) != JNI_OK {
            return;
        }
        let j = env(e);
        let args = args(e);
        j.CallStaticVoidMethodA.or_bug(Errno::ENOSYS)(e, c.class, m(c), args.as_ptr());
        if j.ExceptionCheck.or_bug(Errno::ENOSYS)(e) != 0 {
            j.ExceptionClear.or_bug(Errno::ENOSYS)(e);
        }
    }
}

extern "C" fn notify() {
    call(|c| c.on_notify, |_| Vec::new());
}

extern "C" fn trap() {
    call(|c| c.on_trap, |_| Vec::new());
}

extern "C" fn stopped(errnum: i32, file: *const u8, len: usize, line: u32, n1: u64, n2: u64) {
    // SAFETY: the file's UTF-8, the stop's for the call.
    let file = String::from_utf8_lossy(unsafe { core::slice::from_raw_parts(file, len) }).into_owned();
    let file = std::ffi::CString::new(file.replace('\0', " ")).unwrap_or_default();
    call(
        |c| c.on_stopped,
        |e| {
            // SAFETY: this thread's environment, and a NUL-terminated string.
            let s = unsafe { env(e).NewStringUTF.or_bug(Errno::ENOSYS)(e, file.as_ptr()) };
            vec![
                jvalue { i: errnum },
                jvalue { l: s },
                jvalue { i: line as jint },
                jvalue { j: n1 as i64 },
                jvalue { j: n2 as i64 },
            ]
        },
    );
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
