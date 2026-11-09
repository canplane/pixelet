// pxl-core/sys/file_body.rs

//! A file a device fetched off the JavaScript thread (FileFetch.swift,
//! FileFetch.kt). Its body, zstd or not, is decoded on the caller's thread and
//! handed to the runtime's thread, so no JavaScript touches the bytes.

#[cfg(target_os = "android")]
use xpute_runtime::jni::{
    self,
    sys::{jboolean, jbyteArray, jint, jobject, JNIEnv},
};
use xpute_runtime::native;

use crate::sys::unpack;

/// Says whether the body was good. One that was not goes unanswered here, for
/// the caller to report as a failed fetch.
fn file_body(fd: u32, body: &[u8], zstd: bool) -> bool {
    let raw = if zstd {
        unpack::decode(body)
    } else {
        Some(body.to_vec())
    };
    let Some(raw) = raw else {
        return false;
    };
    native::fs_opened(fd, raw.len() as i32, 0, Some(raw));
    true
}

/// # Safety
/// `body` points at `len` readable bytes.
#[no_mangle]
unsafe extern "C" fn pxl_file_body(fd: u32, body: *const u8, len: usize, zstd: bool) -> bool {
    let body = if len == 0 {
        &[][..]
    } else {
        // SAFETY: the caller's bytes.
        unsafe { core::slice::from_raw_parts(body, len) }
    };
    file_body(fd, body, zstd)
}

/// # Safety
/// Only JNI calls this, with the environment and array of its call.
#[cfg(target_os = "android")]
#[no_mangle]
unsafe extern "system" fn Java_me_plei_pixelet_platform_PxlNative_fileBody(
    e: *mut JNIEnv,
    _this: jobject,
    fd: jint,
    body: jbyteArray,
    zstd: jboolean,
) -> jboolean {
    // SAFETY: the environment and array JNI called with.
    let body = unsafe { jni::bytes(e, body) };
    jboolean::from(file_body(fd as u32, &body, zstd != 0))
}
