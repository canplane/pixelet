// xpute-runtime-native/jni.rs

//! JNI's bindings as an Android app's native side reaches the runtime, its
//! host and what they implement, re-exported (`sys`) so that a device beside
//! the guest names them through this crate; and a Kotlin class whose static
//! methods a thread of the runtime or a host calls (`Class`, `call`,
//! `Return`), a string the C ABI gives among their values (`string`). The
//! runtime itself reaches the app through them: the C ABI's calls under the
//! names JNI looks for, as the `external` functions of the Kotlin class
//! `xpute.Runtime` (`jni_abi!`), and what the app hears as calls of its
//! static `onNotify()`, `onStopped(Stop)` and `onTrap(String)`, from the
//! threads that tell it, a thread the runtime made attached to the VM for
//! good as a daemon; the stop and the trap kept for a UI that starts after
//! them it reads by `stop(): Stop?` and `trap(): String?` (listener.rs
//! `stop`, `trap`). A stop crosses as an `xpute.Stop` both ways
//! (`stop_object`).

use core::ffi::CStr;
use std::path::PathBuf;
use std::sync::OnceLock;

use xpute_kit::status::bug::OrBug;
use xpute_kit::status::errno::Errno;
use xpute_link::mem;

pub use jni_sys as sys;

use sys::{JNIEnv, jbyteArray, jint, jintArray, jlong, jobject, jstring, jvalue};

use crate::listener::{self, notify_on, stopped_on, trap_on};
use crate::remote;

/// This is the table of JNI's functions that the environment of a call holds.
///
/// # Safety
/// `env` is the environment JNI called with.
pub unsafe fn env<'e>(env: *mut JNIEnv) -> &'e sys::JNINativeInterface_ {
    // SAFETY: an environment points at its function table for the call.
    unsafe { &**env }
}

/// A Java `String`, a local reference, of the `len` bytes of UTF-8 at `at`, as
/// a string crosses the C ABI. JNI takes its strings NUL-terminated, so a NUL
/// in it is a space, and an invalid sequence is U+FFFD.
///
/// # Safety
/// `e` is this thread's environment, and `at` `len` readable bytes.
pub unsafe fn string(e: *mut JNIEnv, at: *const u8, len: usize) -> jstring {
    // SAFETY: as the caller holds.
    let s =
        String::from_utf8_lossy(unsafe { core::slice::from_raw_parts(at, len) }).replace('\0', " ");
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
    e: *mut JNIEnv,
    this: jobject,
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
        let class = j.NewGlobalRef.or_bug(Errno::ENOSYS)(
            e,
            j.GetObjectClass.or_bug(Errno::ENOSYS)(e, this),
        );
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
    unsafe fn call(
        e: *mut JNIEnv,
        class: sys::jclass,
        m: sys::jmethodID,
        args: *const jvalue,
    ) -> Self;
}

impl Return for () {
    unsafe fn call(e: *mut JNIEnv, class: sys::jclass, m: sys::jmethodID, args: *const jvalue) {
        // SAFETY: as the caller holds.
        unsafe { env(e).CallStaticVoidMethodA.or_bug(Errno::ENOSYS)(e, class, m, args) }
    }
}

impl Return for f64 {
    unsafe fn call(
        e: *mut JNIEnv,
        class: sys::jclass,
        m: sys::jmethodID,
        args: *const jvalue,
    ) -> f64 {
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
    args: impl FnOnce(*mut JNIEnv) -> Vec<jvalue>,
) -> R {
    let mut e: *mut JNIEnv = core::ptr::null_mut();
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

/// The class `xpute.Stop`, held globally, and its constructor, which `init`
/// takes on a thread Java made, as a thread the runtime made cannot find the
/// app's classes by name.
struct StopClass {
    class: sys::jclass,
    ctor: sys::jmethodID,
}

// SAFETY: a global reference and a method ID stay valid in every thread until
// let go, and nothing in it is changed once it is made.
unsafe impl Send for StopClass {}
// SAFETY: as for `Send`.
unsafe impl Sync for StopClass {}

static STOP: OnceLock<StopClass> = OnceLock::new();

/// The class `xpute.Stop` and its constructor, or none where JNI has no such
/// class or constructor, the exception cleared.
///
/// # Safety
/// `e` is what JNI called with, on a thread Java made.
unsafe fn stop_class(e: *mut JNIEnv) -> Option<StopClass> {
    // SAFETY: JNI's own calls on the environment it called with; the class
    // is held globally, so it outlives this call.
    unsafe {
        let j = env(e);
        let local = j.FindClass.or_bug(Errno::ENOSYS)(e, c"xpute/Stop".as_ptr());
        if local.is_null() {
            j.ExceptionClear.or_bug(Errno::ENOSYS)(e);
            return None;
        }
        let ctor = j.GetMethodID.or_bug(Errno::ENOSYS)(
            e,
            local,
            c"<init>".as_ptr(),
            c"(ILjava/lang/String;IJJ)V".as_ptr(),
        );
        if ctor.is_null() {
            j.ExceptionClear.or_bug(Errno::ENOSYS)(e);
            j.DeleteLocalRef.or_bug(Errno::ENOSYS)(e, local);
            return None;
        }
        let class = j.NewGlobalRef.or_bug(Errno::ENOSYS)(e, local);
        j.DeleteLocalRef.or_bug(Errno::ENOSYS)(e, local);
        Some(StopClass { class, ctor })
    }
}

/// Takes the classes `xpute.Runtime`, from `this`, and `xpute.Stop`, once,
/// and makes the static methods of `xpute.Runtime` hear a batch waiting, a
/// stop and a trap.
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
    methods[ON_STOPPED] = (c"onStopped", c"(Lxpute/Stop;)V");
    methods[ON_TRAP] = (c"onTrap", c"(Ljava/lang/String;)V");
    // SAFETY: what JNI called with, on a thread Java made.
    let Some(k) = (unsafe { stop_class(e) }) else {
        return;
    };
    // SAFETY: as above.
    let Some(c) = (unsafe { class(e, this, methods) }) else {
        // SAFETY: the global reference `stop_class` made, let go once.
        unsafe { env(e).DeleteGlobalRef.or_bug(Errno::ENOSYS)(e, k.class) };
        return;
    };
    let _ = STOP.set(k);
    let _ = CLASS.set(c);
    notify_on(Some(notify));
    stopped_on(Some(stopped));
    trap_on(Some(trapped));
}

extern "C" fn notify() {
    let Some(c) = CLASS.get() else { return };
    // SAFETY: `onNotify()V`, at its index.
    unsafe { call(c, ON_NOTIFY, |_| Vec::new()) }
}

extern "C" fn trapped(err: *const u8, len: usize) {
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

/// A stop as an `xpute.Stop`, a local reference, from what the C ABI gives
/// (listener.rs `Stopped`): its errno, its file's UTF-8 and length, its line
/// and its two numbers, `line`, `n1` and `n2` crossing as the bits of Java's
/// signed `int` and `long`; null before `init` took the class.
///
/// # Safety
/// `e` is this thread's environment, and `file` `len` readable bytes.
unsafe fn stop_object(
    e: *mut JNIEnv,
    errnum: i32,
    file: *const u8,
    len: usize,
    line: u32,
    n1: u64,
    n2: u64,
) -> jobject {
    let Some(k) = STOP.get() else {
        return core::ptr::null_mut();
    };
    // SAFETY: as the caller holds; the constructor of `xpute.Stop` with its
    // five values in order.
    unsafe {
        let vals = [
            jvalue { i: errnum },
            jvalue {
                l: string(e, file, len),
            },
            jvalue { i: line as jint },
            jvalue { j: n1 as i64 },
            jvalue { j: n2 as i64 },
        ];
        env(e).NewObjectA.or_bug(Errno::ENOSYS)(e, k.class, k.ctor, vals.as_ptr())
    }
}

extern "C" fn stopped(errnum: i32, file: *const u8, len: usize, line: u32, n1: u64, n2: u64) {
    let Some(c) = CLASS.get() else { return };
    let args = |e| {
        // SAFETY: this thread's environment, and the file's UTF-8, the
        // stop's for the call.
        let s = unsafe { stop_object(e, errnum, file, len, line, n1, n2) };
        vec![jvalue { l: s }]
    };
    // SAFETY: `onStopped(Lxpute/Stop;)V`, at its index, and its one value.
    unsafe { call(c, ON_STOPPED, args) }
}

/// The guest's last stop, kept for a UI that starts after it, as an
/// `xpute.Stop`, or null before the guest stopped (listener.rs `stop`).
///
/// # Safety
/// `e` is what JNI called with.
pub unsafe fn stop(e: *mut JNIEnv) -> jobject {
    let Some(s) = listener::stop() else {
        return core::ptr::null_mut();
    };
    // SAFETY: the environment JNI called with, and the stop's file.
    unsafe {
        stop_object(
            e,
            s.errnum,
            s.file.as_ptr(),
            s.file.len(),
            s.line,
            s.n1,
            s.n2,
        )
    }
}

/// What trapped the guest, kept for a UI that starts after it, or null
/// before it trapped (listener.rs `trap`).
///
/// # Safety
/// `e` is what JNI called with.
pub unsafe fn trap(e: *mut JNIEnv) -> jstring {
    let Some(err) = listener::trap() else {
        return core::ptr::null_mut();
    };
    // SAFETY: the environment JNI called with, and the trap's UTF-8.
    unsafe { string(e, err.as_ptr(), err.len()) }
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

/// The text of a Java `String`.
///
/// # Safety
/// `e` is what JNI called with, and `s` a live string.
pub unsafe fn text(e: *mut JNIEnv, s: jstring) -> String {
    // SAFETY: JNI's string, its UTF released after the copy.
    unsafe {
        let j = env(e);
        let at = j.GetStringUTFChars.or_bug(Errno::ENOSYS)(e, s, core::ptr::null_mut());
        let text = CStr::from_ptr(at).to_string_lossy().into_owned();
        j.ReleaseStringUTFChars.or_bug(Errno::ENOSYS)(e, s, at);
        text
    }
}

/// A directory a Java `String` names.
///
/// # Safety
/// `e` is what JNI called with, and `s` a live string.
pub unsafe fn dir(e: *mut JNIEnv, s: jstring) -> PathBuf {
    // SAFETY: the caller's.
    PathBuf::from(unsafe { text(e, s) })
}

/// The batch the UI of the handle `ui` has not taken, as a Java `byte[]`.
///
/// # Safety
/// `e` is what JNI called with.
pub unsafe fn take(e: *mut JNIEnv, ui: u32) -> jbyteArray {
    let b = remote::take(ui);
    // SAFETY: JNI's own calls; the array is the caller's local reference.
    unsafe {
        let j = env(e);
        let out = j.NewByteArray.or_bug(Errno::ENOSYS)(e, b.len() as jint);
        j.SetByteArrayRegion.or_bug(Errno::ENOSYS)(e, out, 0, b.len() as jint, b.as_ptr().cast());
        out
    }
}

/// The `len` bytes the guest lent at `at`, its address, as a direct
/// `ByteBuffer` over them, with no copy, the UI's until the lend's answer.
///
/// # Safety
/// `e` is what JNI called with, and the bytes are lent to the UI.
pub unsafe fn buf(e: *mut JNIEnv, at: jlong, len: jint) -> jobject {
    // SAFETY: the caller's.
    let b = unsafe { mem::bytes(at as u64, len as u32) };
    // SAFETY: JNI's own call over those bytes.
    unsafe {
        env(e).NewDirectByteBuffer.or_bug(Errno::ENOSYS)(e, b.as_mut_ptr().cast(), i64::from(len))
    }
}

/// Exports the runtime to JNI for the Kotlin class
/// `xpute.Runtime`, which `native_abi!` expands on Android: `init()`,
/// `start(named: IntArray, prefixes: ByteArray, cacheDir: String): Int`,
/// which answers the UI's handle, `post(ui: Int, tag: Int, h: Int, m: Int,
/// pkt: ByteArray?)`, `answer(ui: Int, tag: Int, result: Int, data:
/// ByteArray?)`, `take(ui: Int): ByteArray`,
/// `bytes(at: Long, len: Int): ByteBuffer`, `life(shown: Boolean)`, `stop(): Stop?`,
/// `trap(): String?` and `log(level: Int, context: String, message: String)`.
#[macro_export]
macro_rules! jni_abi {
    ($start:path) => {
        /// # Safety
        /// Only JNI calls this, on a thread Java made.
        #[unsafe(no_mangle)]
        unsafe extern "system" fn Java_xpute_Runtime_init(
            env: *mut $crate::jni::sys::JNIEnv,
            this: $crate::jni::sys::jobject,
        ) {
            // SAFETY: the environment and the `xpute.Runtime` JNI called with.
            unsafe { $crate::jni::init(env, this) }
        }

        /// # Safety
        /// Only JNI calls this, with live arrays and strings.
        #[unsafe(no_mangle)]
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
        #[unsafe(no_mangle)]
        unsafe extern "system" fn Java_xpute_Runtime_post(
            env: *mut $crate::jni::sys::JNIEnv,
            _this: $crate::jni::sys::jobject,
            ui: $crate::jni::sys::jint,
            tag: $crate::jni::sys::jint,
            h: $crate::jni::sys::jint,
            m: $crate::jni::sys::jint,
            pkt: $crate::jni::sys::jbyteArray,
        ) {
            // SAFETY: what JNI called with.
            let pkt = unsafe { $crate::jni::bytes(env, pkt) };
            $crate::post(ui as u32, tag as u32, h as u32, m as u32, pkt);
        }

        /// # Safety
        /// Only JNI calls this, `data` null or a live byte array.
        #[unsafe(no_mangle)]
        unsafe extern "system" fn Java_xpute_Runtime_answer(
            env: *mut $crate::jni::sys::JNIEnv,
            _this: $crate::jni::sys::jobject,
            ui: $crate::jni::sys::jint,
            tag: $crate::jni::sys::jint,
            result: $crate::jni::sys::jint,
            data: $crate::jni::sys::jbyteArray,
        ) {
            // SAFETY: what JNI called with.
            let data = unsafe { $crate::jni::bytes(env, data) };
            $crate::answer(ui as u32, tag as u32, result, data);
        }

        /// # Safety
        /// Only JNI calls this.
        #[unsafe(no_mangle)]
        unsafe extern "system" fn Java_xpute_Runtime_take(
            env: *mut $crate::jni::sys::JNIEnv,
            _this: $crate::jni::sys::jobject,
            ui: $crate::jni::sys::jint,
        ) -> $crate::jni::sys::jbyteArray {
            // SAFETY: the environment JNI called with.
            unsafe { $crate::jni::take(env, ui as u32) }
        }

        /// # Safety
        /// Only JNI calls this.
        #[unsafe(no_mangle)]
        unsafe extern "system" fn Java_xpute_Runtime_bytes(
            env: *mut $crate::jni::sys::JNIEnv,
            _this: $crate::jni::sys::jobject,
            at: $crate::jni::sys::jlong,
            len: $crate::jni::sys::jint,
        ) -> $crate::jni::sys::jobject {
            // SAFETY: the environment JNI called with, over a buffer the guest
            // lent the UI.
            unsafe { $crate::jni::buf(env, at, len) }
        }

        /// # Safety
        /// Only JNI calls this.
        #[unsafe(no_mangle)]
        unsafe extern "system" fn Java_xpute_Runtime_stop(
            env: *mut $crate::jni::sys::JNIEnv,
            _this: $crate::jni::sys::jobject,
        ) -> $crate::jni::sys::jobject {
            // SAFETY: the environment JNI called with.
            unsafe { $crate::jni::stop(env) }
        }

        /// # Safety
        /// Only JNI calls this.
        #[unsafe(no_mangle)]
        unsafe extern "system" fn Java_xpute_Runtime_trap(
            env: *mut $crate::jni::sys::JNIEnv,
            _this: $crate::jni::sys::jobject,
        ) -> $crate::jni::sys::jstring {
            // SAFETY: the environment JNI called with.
            unsafe { $crate::jni::trap(env) }
        }

        #[unsafe(no_mangle)]
        extern "system" fn Java_xpute_Runtime_life(
            _env: *mut $crate::jni::sys::JNIEnv,
            _this: $crate::jni::sys::jobject,
            shown: $crate::jni::sys::jboolean,
        ) {
            $crate::life(shown != 0);
        }

        /// # Safety
        /// Only JNI calls this, with live strings.
        #[unsafe(no_mangle)]
        unsafe extern "system" fn Java_xpute_Runtime_log(
            env: *mut $crate::jni::sys::JNIEnv,
            _this: $crate::jni::sys::jobject,
            level: $crate::jni::sys::jint,
            context: $crate::jni::sys::jstring,
            msg: $crate::jni::sys::jstring,
        ) {
            // SAFETY: what JNI called with.
            let (context, msg) =
                unsafe { ($crate::jni::text(env, context), $crate::jni::text(env, msg)) };
            $crate::log(level as u32, &context, &msg);
        }
    };
}
