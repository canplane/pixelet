// pxl-core/sys/unpack.rs

//! Decodes the zstd bodies a device's fetch receives. A device asks the
//! server for zstd, the encoding the bake stores objects in, and its
//! transport (ts/client/src/service/platform/origin.native.ts) decodes each body here
//! before anything reads it. A page has no such decoder, because the browser
//! decodes zstd itself.
//!
//! Any frame RFC 8878 allows decodes, whether a stored object's or one the
//! edge compressed on the fly. Such a frame need not state its decoded size,
//! so the library allocates the decoded bytes and the caller frees them.
//!
//! The decoder runs on the native module's queue, never on the JavaScript
//! thread or the core's. It touches only the caller's bytes and its own, so
//! it needs no handoff to the core (xpute-native-host native `Core::run`). An
//! Android app calls it through Kotlin's `PxlNative`
//! (`Java_me_plei_pixelet_platform_PxlNative_unpack`), and an iPhone app
//! through the C ABI (include/pxl_native.h).

#[cfg(target_os = "android")]
use xpute_kit::status::bug::OrBug;
#[cfg(target_os = "android")]
use xpute_kit::status::errno::Errno;
#[cfg(target_os = "android")]
use xpute_native_host::jni;
use zstd::stream;

/// Decodes every frame of `body`, skipping skippable frames. Returns None
/// when a frame does not decode or the body ends inside a frame.
pub(crate) fn decode(body: &[u8]) -> Option<Vec<u8>> {
    // libzstd reads an empty body as a frame cut short, but a body with no
    // frames decodes to nothing.
    if body.is_empty() {
        return Some(Vec::new());
    }
    stream::decode_all(body).ok()
}

/// Decodes the zstd body of `len` bytes at `src`. On success it returns 0,
/// writes the decoded bytes' address to `*out` and their length to
/// `*out_len`, and the library owns those bytes until `pxl_unpacked_free`.
/// When the body does not decode it returns -1 and writes nothing.
///
/// # Safety
/// `src` must point at `len` readable bytes, and `out` and `out_len` at
/// writable words.
#[no_mangle]
unsafe extern "C" fn pxl_unpack(
    src: *const u8,
    len: usize,
    out: *mut *mut u8,
    out_len: *mut usize,
) -> i32 {
    // SAFETY: the caller's bytes.
    let Some(raw) = decode(unsafe { core::slice::from_raw_parts(src, len) }) else {
        return -1;
    };
    let raw = Box::into_raw(raw.into_boxed_slice());
    // SAFETY: the caller's words.
    unsafe { (*out, *out_len) = (raw.cast::<u8>(), raw.len()) };
    0
}

/// Frees the bytes `pxl_unpack` handed out.
///
/// # Safety
/// `out` and `len` must be what one `pxl_unpack` call wrote, and those bytes
/// are freed only once.
#[no_mangle]
unsafe extern "C" fn pxl_unpacked_free(out: *mut u8, len: usize) {
    // SAFETY: the boxed slice `pxl_unpack` made, as it made it.
    drop(unsafe { Box::from_raw(core::ptr::slice_from_raw_parts_mut(out, len)) });
}

/// Runs `f` over the bytes of `packed`, a Java byte array, then releases the
/// array without copying anything back to it.
///
/// # Safety
/// `e` must be the environment JNI called with, and `packed` a live byte
/// array.
#[cfg(target_os = "android")]
unsafe fn with_bytes<R>(
    e: *mut jni::sys::JNIEnv,
    packed: jni::sys::jbyteArray,
    f: impl FnOnce(*const u8, usize) -> R,
) -> R {
    // SAFETY: the environment JNI called this with.
    let j = unsafe { jni::env(e) };
    let (length, elements, release) = (
        j.GetArrayLength.or_bug(Errno::ENOSYS),
        j.GetByteArrayElements.or_bug(Errno::ENOSYS),
        j.ReleaseByteArrayElements.or_bug(Errno::ENOSYS),
    );
    // SAFETY: the caller's array, read until it is released below.
    let (len, at) = unsafe {
        (
            length(e, packed),
            elements(e, packed, core::ptr::null_mut()),
        )
    };
    let r = f(at.cast_const().cast(), usize::try_from(len).unwrap_or(0));
    // SAFETY: the elements just taken, nothing written to them.
    unsafe { release(e, packed, at, jni::sys::JNI_ABORT) };
    r
}

/// Kotlin's `PxlNative.unpack`. It decodes a zstd body and copies the decoded
/// bytes once into a direct buffer that Java owns. It returns null when the
/// body does not decode, when it is too large for a Java buffer, or when Java
/// cannot allocate the buffer.
///
/// # Safety
/// Called by JNI.
#[cfg(target_os = "android")]
#[no_mangle]
unsafe extern "system" fn Java_me_plei_pixelet_platform_PxlNative_unpack(
    e: *mut jni::sys::JNIEnv,
    _class: jni::sys::jclass,
    packed: jni::sys::jbyteArray,
) -> jni::sys::jobject {
    use jni::sys::{jint, jvalue};
    // SAFETY: JNI's array, its own length.
    let Some(raw) = (unsafe {
        with_bytes(e, packed, |at, len| {
            decode(core::slice::from_raw_parts(at, len))
        })
    }) else {
        return core::ptr::null_mut();
    };
    let Ok(size) = jint::try_from(raw.len()) else {
        return core::ptr::null_mut();
    };
    // SAFETY: the environment JNI called this with.
    let j = unsafe { jni::env(e) };
    let (find, method, call, address) = (
        j.FindClass.or_bug(Errno::ENOSYS),
        j.GetStaticMethodID.or_bug(Errno::ENOSYS),
        j.CallStaticObjectMethodA.or_bug(Errno::ENOSYS),
        j.GetDirectBufferAddress.or_bug(Errno::ENOSYS),
    );
    // SAFETY: JNI's own class and method, named as the JVM names them; a
    // failed lookup or allocation leaves its exception for Kotlin, and null.
    unsafe {
        let class = find(e, c"java/nio/ByteBuffer".as_ptr());
        let allocate = if class.is_null() {
            core::ptr::null_mut()
        } else {
            method(
                e,
                class,
                c"allocateDirect".as_ptr(),
                c"(I)Ljava/nio/ByteBuffer;".as_ptr(),
            )
        };
        if allocate.is_null() {
            return core::ptr::null_mut();
        }
        let out = call(e, class, allocate, &jvalue { i: size });
        let at = if out.is_null() {
            core::ptr::null_mut()
        } else {
            address(e, out).cast::<u8>()
        };
        if at.is_null() {
            return core::ptr::null_mut();
        }
        core::ptr::copy_nonoverlapping(raw.as_ptr(), at, raw.len());
        out
    }
}
