// xpute-gpu/host/native/jni.rs

//! The GPU's host as an Android app reaches it: the C ABI's calls under the
//! names JNI looks for, for the Kotlin class `xpute.Gpu`, whose `external`
//! functions they are (`native_abi!`). A view's `Surface` crosses as the
//! native window wgpu draws through (`target`), held from its bind until its
//! unbind.

use std::collections::HashMap;
use std::ptr::NonNull;
use std::sync::{Mutex, OnceLock};

use xpute_host::jni::env;
use xpute_host::jni::ndk::{ANativeWindow, ANativeWindow_fromSurface, ANativeWindow_release};
use xpute_host::jni::sys::{jboolean, jclass, jint, jmethodID, jobject, JNIEnv, JavaVM, JNI_OK};
use xpute_kit::status::bug::OrBug;
use xpute_kit::status::errno::Errno;

use super::wake_on;

/// The native window each bound surface draws through, by the guest's
/// handle: acquired when a view's surface is bound and released once it is
/// not, since the guest's surface over it is gone by then.
static WINDOWS: Mutex<Option<HashMap<u32, usize>>> = Mutex::new(None);

/// The VM, `xpute.Gpu`'s class held past the call, and its static `wake`:
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
// SAFETY: as for `Send`: nothing in it is changed once it is made.
unsafe impl Sync for Wake {}

static WAKE: OnceLock<Wake> = OnceLock::new();

/// `Gpu.open`: the device opened (`open`, the program's `xpute_gpu_open`),
/// `wake` what a completion calls: whether it opened. `this` is `xpute.Gpu`,
/// a Kotlin object, whose calls are its instance's.
///
/// # Safety
/// `e` and `this` are what JNI called with.
pub unsafe fn open(e: *mut JNIEnv, this: jobject, open: fn() -> i32) -> jboolean {
    // SAFETY: the environment JNI called this with.
    let j = unsafe { env(e) };
    let mut vm: *mut JavaVM = core::ptr::null_mut();
    // SAFETY: JNI's own calls on the environment it called with; the class
    // is held globally, so it outlives this call.
    unsafe {
        if j.GetJavaVM.or_bug(Errno::ENOSYS)(e, &raw mut vm) == JNI_OK {
            let class = j.NewGlobalRef.or_bug(Errno::ENOSYS)(
                e,
                j.GetObjectClass.or_bug(Errno::ENOSYS)(e, this),
            );
            let method = j.GetStaticMethodID.or_bug(Errno::ENOSYS)(
                e,
                class,
                c"wake".as_ptr(),
                c"()V".as_ptr(),
            );
            if !method.is_null() {
                let _ = WAKE.set(Wake { vm, class, method });
            }
        }
    }
    wake_on(wake);
    jboolean::from(open() == 0)
}

/// `Gpu.wake`, from whichever thread learned of a completion (`wake_on`).
/// JNI may be called only from a thread attached to the VM, so the caller is
/// attached, for good and as a daemon: the host's threads live as long as it
/// does, and a daemon keeps no VM from exiting.
extern "C" fn wake() {
    let Some(&Wake { vm, class, method }) = WAKE.get() else {
        return;
    };
    let mut e: *mut JNIEnv = core::ptr::null_mut();
    // SAFETY: the VM GetJavaVM gave, which lives as long as the process; the
    // class and method JNI gave while the device opened, the class held.
    unsafe {
        let attach = (**vm).AttachCurrentThreadAsDaemon.or_bug(Errno::ENOSYS);
        if attach(vm, (&raw mut e).cast(), core::ptr::null_mut()) != JNI_OK {
            return;
        }
        let j = env(e);
        j.CallStaticVoidMethodA.or_bug(Errno::ENOSYS)(e, class, method, core::ptr::null());
        // What Kotlin threw has no Java caller here to reach.
        if j.ExceptionCheck.or_bug(Errno::ENOSYS)(e) != 0 {
            j.ExceptionClear.or_bug(Errno::ENOSYS)(e);
        }
    }
}

/// `Gpu.surfaceBind`: the guest's surface `handle` bound to a view's
/// `surface` through the native window it is (`bind`, the program's
/// `xpute_gpu_surface_bind`): 0, `NOT_OPEN`, `NO_SURFACE` or `SLOT_TAKEN`.
///
/// # Safety
/// `e` is what JNI called with, and `surface` a live `android.view.Surface`,
/// which its view keeps until it has unbound it.
pub unsafe fn bind(
    e: *mut JNIEnv,
    handle: u32,
    surface: jobject,
    bind: unsafe fn(u32, *mut core::ffi::c_void) -> i32,
) -> jint {
    // SAFETY: the environment and the surface the call was given; a window
    // made this way is ours until released.
    let Some(window) = NonNull::new(unsafe { ANativeWindow_fromSurface(e, surface) }) else {
        return super::NO_SURFACE;
    };
    let at = window.as_ptr().expose_provenance();
    // SAFETY: the window is held until the surface over it is unbound.
    let bound = unsafe { bind(handle, window.as_ptr().cast()) };
    let let_go = if bound == 0 {
        WINDOWS
            .lock()
            .or_bug(Errno::ENOTRECOVERABLE)
            .get_or_insert_with(HashMap::default)
            .insert(handle, at)
    } else {
        Some(at)
    };
    if let Some(old) = let_go {
        // SAFETY: a window this module acquired, whose surface is gone.
        unsafe {
            ANativeWindow_release(core::ptr::with_exposed_provenance_mut::<ANativeWindow>(old))
        };
    }
    bound
}

/// `Gpu.surfaceUnbind`: `handle`'s surface let go (`unbind`, the program's
/// `xpute_gpu_surface_unbind`), and the window it drew through.
pub fn unbind(handle: u32, unbind: fn(u32)) {
    unbind(handle);
    let window = WINDOWS
        .lock()
        .or_bug(Errno::ENOTRECOVERABLE)
        .as_mut()
        .and_then(|w| w.remove(&handle));
    if let Some(at) = window {
        // SAFETY: a window this module acquired, whose surface is gone.
        unsafe {
            ANativeWindow_release(core::ptr::with_exposed_provenance_mut::<ANativeWindow>(at))
        };
    }
}
