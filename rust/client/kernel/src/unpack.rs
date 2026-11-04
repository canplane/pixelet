// pxl-kernel/unpack.rs

//! A body the app's fetch hands over still zstd, decoded before anything
//! reads it: the device asks for zstd, as the bake stores its objects, and
//! undoes it in its transport (ts/client/src/services/origin.native.ts), as a
//! browser undoes it in its own. Any frame RFC 8878 allows decodes, a
//! stored object's or one the edge made on the fly, which need not say its
//! size: so the decoded bytes are the library's, handed out and let go by
//! the caller. Called from the module's queue, never the JavaScript thread
//! or the kernel's: it touches only the caller's bytes and its own, so it
//! takes no handoff (xpute-host native `Guest::run`). An Android app reaches
//! it through Kotlin's `PxlNative` (`Java_me_plei_pixelet_platform_PxlNative_unpack`),
//! an iPhone's Swift through the C ABI (include/pxl_native.h). A page has
//! none: its browser undoes zstd itself.

#[cfg(target_os = "android")]
use xpute_host::jni;
#[cfg(target_os = "android")]
use xpute_kit::status::bug::OrBug;
#[cfg(target_os = "android")]
use xpute_kit::status::errno::Errno;
use zstd::stream;

/// Every frame of `body` decoded, its skippable frames passed over; none
/// where one does not decode, or ends before its frame does.
pub(crate) fn decode(body: &[u8]) -> Option<Vec<u8>> {
    // libzstd reads no frame as one cut short; no frame is nothing.
    if body.is_empty() {
        return Some(Vec::new());
    }
    stream::decode_all(body).ok()
}

/// Decodes the zstd body of `len` bytes at `src`: 0, with the bytes at
/// `*out`, `*out_len` long, which are the library's until
/// `pxl_unpacked_free`; -1 where it does not decode, nothing handed out.
///
/// # Safety
/// `src` points at `len` readable bytes, `out` and `out_len` at writable words.
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

/// Lets go the bytes `pxl_unpack` handed out.
///
/// # Safety
/// `out` and `len` are what one `pxl_unpack` wrote, let go once.
#[no_mangle]
unsafe extern "C" fn pxl_unpacked_free(out: *mut u8, len: usize) {
    // SAFETY: the boxed slice `pxl_unpack` made, as it made it.
    drop(unsafe { Box::from_raw(core::ptr::slice_from_raw_parts_mut(out, len)) });
}

/// `f` over the bytes of `packed`, a Java byte array, let go unwritten after.
///
/// # Safety
/// `e` is the environment JNI called with, `packed` a live byte array.
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

/// Kotlin's `PxlNative.unpack`: a zstd body decoded into a direct buffer Java
/// owns, the decoded bytes copied once into it; null where it does not
/// decode (`decode`).
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
