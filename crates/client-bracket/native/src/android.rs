// pxl-native/android.rs

//! The kernel as an Android app reaches it. Kotlin's `PxlGuest`
//! (apps/native/modules/platform/android/src/main/java/me/plei/pixelet/platform/PxlGuest.kt)
//! declares these as `external` functions, and each is the C ABI's call an
//! iPhone's Swift makes (lib.rs, surface.rs, unpack.rs) under the name JNI
//! looks for. Only JNI's own values cross: a number, an array, the memory as
//! a direct buffer over the memory — which Kotlin hands JavaScript as it is,
//! as Swift hands it the memory — and a view's `Surface`, made the native
//! window wgpu draws through.

use fnv::FnvHashMap;
use std::ptr::NonNull;
use std::sync::{Mutex, OnceLock};

use jni_sys::{jboolean, jbyteArray, jclass, jdouble, jint, jmethodID, jobject, jvalue, JNIEnv, JNINativeInterface_, JavaVM, JNI_ABORT, JNI_OK};
use ndk_sys::{ANativeWindow, ANativeWindow_fromSurface, ANativeWindow_release};
use wgpu::rwh::{AndroidDisplayHandle, AndroidNdkWindowHandle, RawDisplayHandle, RawWindowHandle};
use xpute_kit::status::bug::OrBug;
use xpute_kit::status::errno::Errno;

/// The native window each bound surface draws through, by the kernel's
/// handle: acquired when a view's surface is bound and released once it is
/// not, since the kernel's surface over it is gone by then.
static WINDOWS: Mutex<Option<FnvHashMap<u32, usize>>> = Mutex::new(None);

/// JNI's functions, as the environment a call is given holds them.
///
/// # Safety
/// `env` is the environment JNI called with.
unsafe fn jni<'e>(env: *mut JNIEnv) -> &'e JNINativeInterface_ {
    // SAFETY: an environment points at its function table for the call.
    unsafe { &**env }
}

/// A turn of `quota_ms` (lib.rs `interrupt`, exported as pxl_interrupt).
#[no_mangle]
pub extern "system" fn Java_me_plei_pixelet_platform_PxlGuest_interrupt(_env: *mut JNIEnv, _class: jclass, quota_ms: jdouble) -> jdouble {
    crate::interrupt(quota_ms)
}

/// The memory, as a direct buffer over the whole of it (lib.rs `memory`, exported as pxl_memory).
///
/// # Safety
/// Called by JNI.
#[no_mangle]
pub unsafe extern "system" fn Java_me_plei_pixelet_platform_PxlGuest_memory(env: *mut JNIEnv, _class: jclass) -> jobject {
    let mut bytes = 0u32;
    // SAFETY: our own word.
    let at = unsafe { crate::memory(&raw mut bytes) };
    let make = unsafe { jni(env) }.NewDirectByteBuffer.or_bug(Errno::ENOSYS);
    // SAFETY: the memory lives as long as the process, and is `bytes` long.
    unsafe { make(env, at.cast(), i64::from(bytes)) }
}

/// `f` over the bytes of `packed`, a Java byte array, let go unwritten after.
///
/// # Safety
/// `env` is the environment JNI called with, `packed` a live byte array.
unsafe fn with_bytes<R>(env: *mut JNIEnv, packed: jbyteArray, f: impl FnOnce(*const u8, usize) -> R) -> R {
    let jni = unsafe { jni(env) };
    let (length, elements, release) = (
        jni.GetArrayLength.or_bug(Errno::ENOSYS),
        jni.GetByteArrayElements.or_bug(Errno::ENOSYS),
        jni.ReleaseByteArrayElements.or_bug(Errno::ENOSYS),
    );
    // SAFETY: the caller's array, read until it is released below.
    let (len, at) = unsafe { (length(env, packed), elements(env, packed, core::ptr::null_mut())) };
    let r = f(at.cast_const().cast(), usize::try_from(len).unwrap_or(0));
    // SAFETY: the elements just taken, nothing written to them.
    unsafe { release(env, packed, at, JNI_ABORT) };
    r
}

/// A zstd body decoded into a direct buffer Java owns, the decoded bytes
/// copied once into it; null where it does not decode (unpack.rs `decode`).
///
/// # Safety
/// Called by JNI.
#[no_mangle]
pub unsafe extern "system" fn Java_me_plei_pixelet_platform_PxlGuest_unpack(env: *mut JNIEnv, _class: jclass, packed: jbyteArray) -> jobject {
    // SAFETY: JNI's array, its own length.
    let Some(raw) = (unsafe { with_bytes(env, packed, |at, len| crate::unpack::decode(core::slice::from_raw_parts(at, len))) }) else {
        return core::ptr::null_mut();
    };
    let Ok(size) = jint::try_from(raw.len()) else { return core::ptr::null_mut() };
    let jni = unsafe { jni(env) };
    let (find, method, call, address) = (
        jni.FindClass.or_bug(Errno::ENOSYS),
        jni.GetStaticMethodID.or_bug(Errno::ENOSYS),
        jni.CallStaticObjectMethodA.or_bug(Errno::ENOSYS),
        jni.GetDirectBufferAddress.or_bug(Errno::ENOSYS),
    );
    // SAFETY: JNI's own class and method, named as the JVM names them; a
    // failed lookup or allocation leaves its exception for Kotlin, and null.
    unsafe {
        let class = find(env, c"java/nio/ByteBuffer".as_ptr());
        let allocate = if class.is_null() {
            core::ptr::null_mut()
        } else {
            method(env, class, c"allocateDirect".as_ptr(), c"(I)Ljava/nio/ByteBuffer;".as_ptr())
        };
        if allocate.is_null() {
            return core::ptr::null_mut();
        }
        let out = call(env, class, allocate, &jvalue { i: size });
        let at = if out.is_null() { core::ptr::null_mut() } else { address(env, out).cast::<u8>() };
        if at.is_null() {
            return core::ptr::null_mut();
        }
        core::ptr::copy_nonoverlapping(raw.as_ptr(), at, raw.len());
        out
    }
}

/// The VM, `PxlGuest`'s class held past the call, and its static `wake`:
/// what a completion calls to ask a turn (`wake`). Taken as the device opens,
/// on a thread Java made, since a thread the host made finds no class of the
/// app's by name.
struct Wake {
    vm: *mut JavaVM,
    class: jclass,
    method: jmethodID,
}

// SAFETY: JNI's VM is one for the process and may be used from any thread; a
// global reference and a method ID stay valid in every thread until let go.
unsafe impl Send for Wake {}
unsafe impl Sync for Wake {}

static WAKE: OnceLock<Wake> = OnceLock::new();

/// Opens the device: whether it opened (surface.rs `pxl_gpu_open`). `this` is
/// `PxlGuest`, a Kotlin object, whose calls are its instance's.
///
/// # Safety
/// Called by JNI.
#[no_mangle]
pub unsafe extern "system" fn Java_me_plei_pixelet_platform_PxlGuest_gpuOpen(env: *mut JNIEnv, this: jobject) -> jboolean {
    let j = unsafe { jni(env) };
    let mut vm: *mut JavaVM = core::ptr::null_mut();
    // SAFETY: JNI's own calls on the environment it called with; the class
    // is held globally, so it outlives this call.
    unsafe {
        if j.GetJavaVM.or_bug(Errno::ENOSYS)(env, &raw mut vm) == JNI_OK {
            let class = j.NewGlobalRef.or_bug(Errno::ENOSYS)(env, j.GetObjectClass.or_bug(Errno::ENOSYS)(env, this));
            let method = j.GetStaticMethodID.or_bug(Errno::ENOSYS)(env, class, c"wake".as_ptr(), c"()V".as_ptr());
            if !method.is_null() {
                let _ = WAKE.set(Wake { vm, class, method });
            }
        }
    }
    jboolean::from(crate::surface::pxl_gpu_open() == 0)
}

/// `PxlGuest.wake`, from whichever thread learned of a completion (surface.rs
/// `wake`). JNI may be called only from a thread attached to the VM, so the
/// caller is attached, for good and as a daemon: the host's threads live as
/// long as it does, and a daemon keeps no VM from exiting. Kotlin sends the
/// app's event from there, which Expo carries to the JavaScript thread.
pub fn wake() {
    let Some(&Wake { vm, class, method }) = WAKE.get() else { return };
    let mut env: *mut JNIEnv = core::ptr::null_mut();
    // SAFETY: the VM GetJavaVM gave, which lives as long as the process; the
    // class and method JNI gave while the device opened, the class held.
    unsafe {
        let attach = (**vm).AttachCurrentThreadAsDaemon.or_bug(Errno::ENOSYS);
        if attach(vm, (&raw mut env).cast(), core::ptr::null_mut()) != JNI_OK {
            return;
        }
        let j = jni(env);
        j.CallStaticVoidMethodA.or_bug(Errno::ENOSYS)(env, class, method, core::ptr::null());
        // What Kotlin threw has no Java caller here to reach.
        if j.ExceptionCheck.or_bug(Errno::ENOSYS)(env) != 0 {
            j.ExceptionClear.or_bug(Errno::ENOSYS)(env);
        }
    }
}

/// Binds the kernel's surface `handle` to a view's `surface`, through the
/// native window it is: 0, -2 when no device is open, -3 when the system
/// would not make a surface of it (surface.rs `bind`).
///
/// # Safety
/// Called by JNI, with a live `android.view.Surface`, which its view keeps
/// until it has unbound it.
#[no_mangle]
pub unsafe extern "system" fn Java_me_plei_pixelet_platform_PxlGuest_surfaceBind(env: *mut JNIEnv, _class: jclass, handle: jint, surface: jobject) -> jint {
    let handle = handle as u32;
    // SAFETY: the environment and the surface the call was given; a window
    // made this way is ours until released.
    let Some(window) = NonNull::new(unsafe { ANativeWindow_fromSurface(env, surface) }) else {
        return -3;
    };
    let at = window.as_ptr().expose_provenance();
    let made = move || wgpu::SurfaceTargetUnsafe::RawHandle {
        raw_display_handle: Some(RawDisplayHandle::Android(AndroidDisplayHandle::new())),
        raw_window_handle: RawWindowHandle::AndroidNdk(AndroidNdkWindowHandle::new(NonNull::new(core::ptr::with_exposed_provenance_mut(at)).or_bug(Errno::EFAULT))),
    };
    // SAFETY: the window is held until the surface over it is unbound.
    let bound = unsafe { crate::surface::bind(handle, made) };
    let let_go = if bound == 0 {
        WINDOWS.lock().or_bug(Errno::ENOTRECOVERABLE).get_or_insert_with(FnvHashMap::default).insert(handle, at)
    } else {
        Some(at)
    };
    if let Some(old) = let_go {
        // SAFETY: a window this module acquired, whose surface is gone.
        unsafe { ANativeWindow_release(core::ptr::with_exposed_provenance_mut::<ANativeWindow>(old)) };
    }
    bound
}

/// Lets `handle`'s surface go, and the window it drew through
/// (surface.rs `pxl_surface_unbind`).
#[no_mangle]
pub extern "system" fn Java_me_plei_pixelet_platform_PxlGuest_surfaceUnbind(_env: *mut JNIEnv, _class: jclass, handle: jint) {
    let handle = handle as u32;
    crate::surface::pxl_surface_unbind(handle);
    let window = WINDOWS.lock().or_bug(Errno::ENOTRECOVERABLE).as_mut().and_then(|w| w.remove(&handle));
    if let Some(at) = window {
        // SAFETY: a window this module acquired, whose surface is gone.
        unsafe { ANativeWindow_release(core::ptr::with_exposed_provenance_mut::<ANativeWindow>(at)) };
    }
}

/// A line to the system's log, where an Android app's error stream goes
/// nowhere anyone reads.
pub fn say(line: &str) {
    let line = std::ffi::CString::new(line.replace('\0', " ")).unwrap_or_default();
    // SAFETY: two strings, ours for the call.
    unsafe { ndk_sys::__android_log_write(ndk_sys::android_LogPriority::ANDROID_LOG_ERROR.0 as i32, c"pxl".as_ptr(), line.as_ptr()) };
}
