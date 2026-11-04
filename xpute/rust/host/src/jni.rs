// xpute-host/jni.rs

//! A native guest as an Android app reaches it: the C ABI's calls under the
//! names JNI looks for, for the Kotlin class `xpute.Guest`, whose `external`
//! functions they are (`native_guest!`). Only JNI's own values cross: a
//! number, and the memory as a direct buffer over the whole of it, which the
//! app hands its JavaScript as it is. JNI's and the NDK's bindings are
//! re-exported (`sys`, `ndk`), so a program's expansion and xpute-gpu's
//! name them through this crate.

use std::cell::Cell;
use std::sync::Once;

pub use jni_sys as sys;
pub use ndk_sys as ndk;

use sys::{jdouble, jint, jobject, jvalue, JNIEnv, JNINativeInterface_};
use xpute_kit::status::bug::OrBug;
use xpute_kit::status::errno::Errno;

use crate::native::{answer_with, Guest};

/// JNI's functions, as the environment a call is given holds them.
///
/// # Safety
/// `env` is the environment JNI called with.
pub unsafe fn env<'e>(env: *mut JNIEnv) -> &'e JNINativeInterface_ {
    // SAFETY: an environment points at its function table for the call.
    unsafe { &**env }
}

thread_local! {
    /// The environment and `xpute.Guest` of the call waiting in `run`, which
    /// answers the turn's hostcalls (`hostcall`).
    #[allow(clippy::missing_const_for_thread_local, reason = "already const; the lint misreads Android's expansion")]
    static INTR: Cell<Option<(*mut JNIEnv, jobject)>> = const { Cell::new(None) };
}

/// `Guest.run`: a turn (`turn`, the program's `xpute_run`), its hostcalls
/// answered on this thread by `Guest.hostcall`. `this` is `xpute.Guest`.
///
/// # Safety
/// `env` and `this` are what JNI called with.
pub unsafe fn run(env: *mut JNIEnv, this: jobject, turn: impl FnOnce() -> jdouble) -> jdouble {
    static ANSWER: Once = Once::new();
    ANSWER.call_once(|| answer_with(Some(hostcall)));
    INTR.with(|c| c.set(Some((env, this))));
    let wake = turn();
    INTR.with(|c| c.set(None));
    wake
}

/// `Guest.hostcall`, the static `hostcall(I)I` of `xpute.Guest`, on the
/// thread waiting in `run`, through the environment it was called with
/// (`answer_with`): ENOSYS outside one, EIO where Kotlin threw.
extern "C" fn hostcall(at: u32) -> i32 {
    let Some((e, this)) = INTR.with(Cell::get) else {
        return Errno::ENOSYS as i32;
    };
    // SAFETY: the environment and object JNI called `run` with, which wait
    // in that call on this thread; JNI's own calls on them.
    unsafe {
        let j = env(e);
        let class = j.GetObjectClass.or_bug(Errno::ENOSYS)(e, this);
        let method = j.GetStaticMethodID.or_bug(Errno::ENOSYS)(
            e,
            class,
            c"hostcall".as_ptr(),
            c"(I)I".as_ptr(),
        );
        if method.is_null() {
            j.ExceptionClear.or_bug(Errno::ENOSYS)(e);
            return Errno::ENOSYS as i32;
        }
        let res = j.CallStaticIntMethodA.or_bug(Errno::ENOSYS)(
            e,
            class,
            method,
            &jvalue { i: at as jint },
        );
        if j.ExceptionCheck.or_bug(Errno::ENOSYS)(e) != 0 {
            j.ExceptionClear.or_bug(Errno::ENOSYS)(e);
            return Errno::EIO as i32;
        }
        res
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
