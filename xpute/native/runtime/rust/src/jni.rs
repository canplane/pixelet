// xpute-native-runtime/jni.rs

//! JNI's and the NDK's bindings as an Android app's native side reaches the
//! runtime, its host and what they serve, re-exported (`sys`, `ndk`) so that a device
//! beside the guest names them through this crate.

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
