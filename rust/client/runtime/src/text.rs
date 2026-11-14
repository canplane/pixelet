// pxl-runtime/text.rs

//! A device's `text` (spec/client/core/wit/text.wit), served on the host's
//! side of the link and answered by the system's font engine (Text.swift,
//! Text.kt) on the core's own thread before the call returns, so an entry that
//! needs a text waits for no other thread. On a page the runtime answers the
//! same calls (ts/client/src/service/core/text.ts); the core breaks the lines
//! itself (pxl-core sys/io/text.rs).

use std::rc::Rc;
use std::sync::OnceLock;

use pxl_spec::color::Rgba;
use xpute_kit::status::errno::Errno;
use xpute_link::abi::types::Lent;
use xpute_link::mem;

use crate::abi::text;

/// The system's text: how wide a line of UTF-8 runs at a CSS weight and a size,
/// and a line drawn from `x` into `width` by `height` pixels of RGBA, top row
/// first, alpha not premultiplied.
#[derive(Clone, Copy)]
pub struct SystemText {
    pub width: unsafe extern "C" fn(weight: f64, px: f64, text: *const u8, len: usize) -> f64,
    pub raster: unsafe extern "C" fn(
        weight: f64,
        px: f64,
        rgba: Rgba,
        text: *const u8,
        len: usize,
        x: f64,
        width: u32,
        height: u32,
        out: *mut u8,
    ),
}

static SYSTEM: OnceLock<SystemText> = OnceLock::new();

/// Says what answers the core's text from now on. A process's text never
/// changes, so a second call changes nothing.
pub fn text_on(system: SystemText) {
    let _ = SYSTEM.set(system);
}

/// The system's text, or ENOSYS before the platform says what it is.
fn system() -> Result<&'static SystemText, Errno> {
    SYSTEM.get().ok_or(Errno::ENOSYS)
}

/// The text a call lends, which the core holds until the call returns.
fn text_of(span: &Lent) -> Result<&'static str, Errno> {
    // SAFETY: the text the core lent for the call, until it returns.
    core::str::from_utf8(unsafe { mem::read(span.at, span.len) }).map_err(|_| Errno::EBADMSG)
}

/// The system's text, as the core's `text` asks it.
struct Device;

impl text::Host for Device {
    fn width(&self, weight: u32, px: f64, text: Lent) -> Result<f64, Errno> {
        let system = system()?;
        let text = text_of(&text)?;
        // SAFETY: the bytes of `text`, which outlive the call.
        Ok(unsafe { (system.width)(f64::from(weight), px, text.as_ptr(), text.len()) })
    }

    /// Draws the line into `image`, `width` by `height` pixels.
    fn raster(
        &self,
        weight: u32,
        px: f64,
        color: Rgba,
        image: Lent,
        width: u32,
        height: u32,
        x: f64,
        text: Lent,
    ) -> Result<u32, Errno> {
        let system = system()?;
        let text = text_of(&text)?;
        let len = width
            .checked_mul(height)
            .and_then(|n| n.checked_mul(4))
            .filter(|&n| n <= image.len)
            .ok_or(Errno::EINVAL)?;
        // SAFETY: the pixels the core lent for this call, which nothing else
        // touches until it returns.
        let out = unsafe { mem::bytes(image.at, len) };
        // SAFETY: the text's bytes and `out`, both held for the call.
        unsafe {
            (system.raster)(
                f64::from(weight),
                px,
                color,
                text.as_ptr(),
                text.len(),
                x,
                width,
                height,
                out.as_mut_ptr(),
            );
        }
        Ok(0)
    }
}

/// Mounts the text on the runtime's link, on the core's thread before it
/// boots (lib.rs `start`).
pub(crate) fn serve() {
    text::mount(Rc::new(Device));
}

/// Says what an iPhone app's Swift answers the core's text with.
#[cfg(not(target_os = "android"))]
#[no_mangle]
extern "C" fn pxl_text_on(
    width: unsafe extern "C" fn(f64, f64, *const u8, usize) -> f64,
    raster: unsafe extern "C" fn(f64, f64, u32, *const u8, usize, f64, u32, u32, *mut u8),
) {
    text_on(SystemText { width, raster });
}

/// An Android app's text, the static methods of Kotlin's `PxlNative`.
#[cfg(target_os = "android")]
mod android {
    use std::sync::OnceLock;

    use xpute_kit::status::bug::OrBug;
    use xpute_kit::status::errno::Errno;
    use xpute_native_runtime::jni::sys::{
        jbyteArray, jclass, jmethodID, jobject, jvalue, JNIEnv, JavaVM, JNI_OK, JNI_VERSION_1_6,
    };
    use xpute_native_runtime::jni::{self, sys::jint};

    use super::{text_on, Rgba, SystemText};

    struct Class {
        vm: *mut JavaVM,
        class: jclass,
        width: jmethodID,
        raster: jmethodID,
    }

    // SAFETY: JNI's VM is one for the process and may be used from any thread;
    // a global reference and a method ID stay valid in every thread.
    unsafe impl Send for Class {}
    // SAFETY: as for `Send`: nothing in it is changed once it is made.
    unsafe impl Sync for Class {}

    static CLASS: OnceLock<Class> = OnceLock::new();

    /// The core's thread as JNI knows it, attached for good as a daemon.
    ///
    /// # Safety
    /// `vm` is the process's VM.
    unsafe fn env_here(vm: *mut JavaVM) -> *mut JNIEnv {
        let mut e: *mut JNIEnv = core::ptr::null_mut();
        // SAFETY: the VM, which lives as long as the process.
        unsafe {
            let get = (**vm).GetEnv.or_bug(Errno::ENOSYS);
            if get(vm, (&raw mut e).cast(), JNI_VERSION_1_6) != JNI_OK {
                let attach = (**vm).AttachCurrentThreadAsDaemon.or_bug(Errno::ENOSYS);
                attach(vm, (&raw mut e).cast(), core::ptr::null_mut());
            }
        }
        e
    }

    /// The UTF-8 of a text as a Java `byte[]`, a local reference.
    ///
    /// # Safety
    /// `e` is this thread's environment, and `text` `len` readable bytes.
    unsafe fn bytes(e: *mut JNIEnv, text: *const u8, len: usize) -> jbyteArray {
        // SAFETY: as the caller says.
        unsafe {
            let j = jni::env(e);
            let out = j.NewByteArray.or_bug(Errno::ENOSYS)(e, len as jint);
            j.SetByteArrayRegion.or_bug(Errno::ENOSYS)(e, out, 0, len as jint, text.cast());
            out
        }
    }

    unsafe extern "C" fn width(weight: f64, px: f64, text: *const u8, len: usize) -> f64 {
        let Some(c) = CLASS.get() else { return 0.0 };
        // SAFETY: the class holds the VM and the method; a local reference is
        // let go here, since this thread never returns to Java.
        unsafe {
            let e = env_here(c.vm);
            let j = jni::env(e);
            let t = bytes(e, text, len);
            let args = [jvalue { l: t }, jvalue { d: weight }, jvalue { d: px }];
            let w =
                j.CallStaticDoubleMethodA.or_bug(Errno::ENOSYS)(e, c.class, c.width, args.as_ptr());
            j.ExceptionClear.or_bug(Errno::ENOSYS)(e);
            j.DeleteLocalRef.or_bug(Errno::ENOSYS)(e, t);
            w
        }
    }

    unsafe extern "C" fn raster(
        weight: f64,
        px: f64,
        rgba: Rgba,
        text: *const u8,
        len: usize,
        x: f64,
        width: u32,
        height: u32,
        out: *mut u8,
    ) {
        let Some(c) = CLASS.get() else { return };
        // SAFETY: as in `width`; `out` is `width` by `height` pixels of four
        // bytes, the caller's for the call.
        unsafe {
            let e = env_here(c.vm);
            let j = jni::env(e);
            let t = bytes(e, text, len);
            let buf = j.NewDirectByteBuffer.or_bug(Errno::ENOSYS)(
                e,
                out.cast(),
                i64::from(width) * i64::from(height) * 4,
            );
            let args = [
                jvalue { l: t },
                jvalue { d: weight },
                jvalue { d: px },
                jvalue { j: i64::from(rgba) },
                jvalue { d: x },
                jvalue { i: width as jint },
                jvalue { i: height as jint },
                jvalue { l: buf },
            ];
            j.CallStaticVoidMethodA.or_bug(Errno::ENOSYS)(e, c.class, c.raster, args.as_ptr());
            j.ExceptionClear.or_bug(Errno::ENOSYS)(e);
            j.DeleteLocalRef.or_bug(Errno::ENOSYS)(e, t);
            j.DeleteLocalRef.or_bug(Errno::ENOSYS)(e, buf);
        }
    }

    /// Kotlin's `PxlNative.textOn`: takes the class on a thread Java made, which
    /// can find it, and makes it answer the core's text.
    ///
    /// # Safety
    /// Only JNI calls this, with the environment and object of its call.
    #[no_mangle]
    unsafe extern "system" fn Java_me_plei_pixelet_platform_PxlNative_textOn(
        e: *mut JNIEnv,
        this: jobject,
    ) {
        if CLASS.get().is_some() {
            return;
        }
        // SAFETY: JNI's own calls on the environment it called with; the class
        // is held globally, so it outlives this call.
        unsafe {
            let j = jni::env(e);
            let mut vm: *mut JavaVM = core::ptr::null_mut();
            if j.GetJavaVM.or_bug(Errno::ENOSYS)(e, &raw mut vm) != JNI_OK {
                return;
            }
            let class = j.NewGlobalRef.or_bug(Errno::ENOSYS)(
                e,
                j.GetObjectClass.or_bug(Errno::ENOSYS)(e, this),
            );
            let method = |name: &core::ffi::CStr, sig: &core::ffi::CStr| {
                j.GetStaticMethodID.or_bug(Errno::ENOSYS)(e, class, name.as_ptr(), sig.as_ptr())
            };
            let w = method(c"width", c"([BDD)D");
            if w.is_null() {
                j.ExceptionClear.or_bug(Errno::ENOSYS)(e);
                return;
            }
            let r = method(c"raster", c"([BDDJDIILjava/nio/ByteBuffer;)V");
            if r.is_null() {
                j.ExceptionClear.or_bug(Errno::ENOSYS)(e);
                return;
            }
            let _ = CLASS.set(Class {
                vm,
                class,
                width: w,
                raster: r,
            });
        }
        text_on(SystemText { width, raster });
    }
}
