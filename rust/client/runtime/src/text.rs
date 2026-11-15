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
pub(crate) struct SystemText {
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
pub(crate) fn text_on(system: SystemText) {
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
    ) -> Result<(), Errno> {
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
        Ok(())
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
    use xpute_native_runtime::jni::sys::{jbyteArray, jint, jobject, jvalue, JNIEnv};
    use xpute_native_runtime::jni::{call, class, env, Class};

    use super::{text_on, Rgba, SystemText};

    /// The class `PxlNative` and its static methods at `WIDTH` and `RASTER`.
    static CLASS: OnceLock<Class<2>> = OnceLock::new();
    const WIDTH: usize = 0;
    const RASTER: usize = 1;

    /// The UTF-8 of a text as a Java `byte[]`, a local reference.
    ///
    /// # Safety
    /// `e` is this thread's environment, and `text` `len` readable bytes.
    unsafe fn bytes(e: *mut JNIEnv, text: *const u8, len: usize) -> jbyteArray {
        // SAFETY: as the caller says.
        unsafe {
            let j = env(e);
            let out = j.NewByteArray.or_bug(Errno::ENOSYS)(e, len as jint);
            j.SetByteArrayRegion.or_bug(Errno::ENOSYS)(e, out, 0, len as jint, text.cast());
            out
        }
    }

    unsafe extern "C" fn width(weight: f64, px: f64, text: *const u8, len: usize) -> f64 {
        let Some(c) = CLASS.get() else { return 0.0 };
        let args = |e| {
            // SAFETY: this thread's environment, and the text's bytes, the
            // caller's for the call.
            let t = unsafe { bytes(e, text, len) };
            vec![jvalue { l: t }, jvalue { d: weight }, jvalue { d: px }]
        };
        // SAFETY: `width([BDD)D`, at its index, and its three values in order.
        unsafe { call(c, WIDTH, args) }
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
        let args = |e| {
            // SAFETY: as in `width`; `out` is `width` by `height` pixels of
            // four bytes, the caller's for the call.
            let (t, buf) = unsafe {
                let buf = env(e).NewDirectByteBuffer.or_bug(Errno::ENOSYS)(
                    e,
                    out.cast(),
                    i64::from(width) * i64::from(height) * 4,
                );
                (bytes(e, text, len), buf)
            };
            vec![
                jvalue { l: t },
                jvalue { d: weight },
                jvalue { d: px },
                jvalue { j: i64::from(rgba) },
                jvalue { d: x },
                jvalue { i: width as jint },
                jvalue { i: height as jint },
                jvalue { l: buf },
            ]
        };
        // SAFETY: `raster([BDDJDIILjava/nio/ByteBuffer;)V`, at its index, and
        // its eight values in order.
        unsafe { call(c, RASTER, args) }
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
        let mut methods = [(c"", c""); 2];
        methods[WIDTH] = (c"width", c"([BDD)D");
        methods[RASTER] = (c"raster", c"([BDDJDIILjava/nio/ByteBuffer;)V");
        // SAFETY: what JNI called with, on a thread Java made.
        let Some(c) = (unsafe { class(e, this, methods) }) else {
            return;
        };
        let _ = CLASS.set(c);
        text_on(SystemText { width, raster });
    }
}
