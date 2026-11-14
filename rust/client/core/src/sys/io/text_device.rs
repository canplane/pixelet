// pxl-core/sys/io/text_device.rs

//! A device's text calls (text.json TEXT_LAYOUT, TEXT_RASTER), answered by the
//! system's font engine (Text.swift, Text.kt) on the core's own thread, so an
//! entry that needs a text waits for no other thread. On a page the runtime
//! answers the same calls (ts/client/src/service/core/text.ts), and the two
//! break lines alike.

use std::sync::OnceLock;

use xpute_link::frame::Frame;
use xpute_link::mem;
use xpute_kit::status::errno::Errno;
use xpute_kit::status::error::MarshalError;
use xpute_native_host::abi::Outcome;
use xpute_native_host::native;

use crate::sys::io::text_answer_spec::{LAYOUT_ANSWER, LINE};
use crate::sys::io::text_runtime_spec::{self as text, TextLayout, TextRaster};
use crate::spec::manifest::MANIFEST;
use pxl_spec::color::Rgba;

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

/// The system's own whitespace, as a page's `\s` and `trim` read it.
fn is_space(c: char) -> bool {
    matches!(
        c,
        '\u{9}'..='\u{d}'
            | ' '
            | '\u{a0}'
            | '\u{1680}'
            | '\u{2000}'..='\u{200a}'
            | '\u{2028}'
            | '\u{2029}'
            | '\u{202f}'
            | '\u{205f}'
            | '\u{3000}'
            | '\u{feff}'
    )
}

/// Closing marks and, in Japanese, the small kana and the marks that lean on
/// the character before them: no line starts with one.
fn no_line_start(c: char) -> bool {
    "、。，．・：；？！）］｝〕〉》」』】”’…‥ーぁぃぅぇぉっゃゅょゎァィゥェォッャュョヮゝゞヽヾ々,.:;!?)]}"
        .contains(c)
}

/// Opening marks: no line ends with one.
fn no_line_end(c: char) -> bool {
    "（［｛〔〈《「『【“‘([{".contains(c)
}

/// Moves the break at `at` back past any mark that keeps with its neighbor,
/// but never past the line's first character.
fn allowed_break(chars: &[(usize, char)], line: usize, mut at: usize) -> usize {
    while at - 1 > line
        && (no_line_start(chars[at].1) || no_line_end(chars[at - 1].1))
        && chars[at - 1].1 <= '\u{ffff}'
    {
        at -= 1;
    }
    at
}

/// Each line's end, a byte offset. Words fill each line greedily. A word too
/// wide for a line by itself, as Korean, Japanese or Chinese typed without
/// spaces can be, breaks between characters. A space stays at the head of the
/// next line, for the core to trim.
fn line_ends(text: &str, max_width: f64, width: &mut impl FnMut(&str) -> f64) -> Vec<usize> {
    let chars: Vec<(usize, char)> = text.char_indices().collect();
    let byte = |i: usize| chars.get(i).map_or(text.len(), |c| c.0);
    let mut words = Vec::new();
    let mut i = 0;
    while i < chars.len() {
        if is_space(chars[i].1) {
            i += 1;
            continue;
        }
        let start = i;
        while i < chars.len() && !is_space(chars[i].1) {
            i += 1;
        }
        words.push((start, i));
    }
    if max_width <= 0.0 || words.is_empty() {
        return vec![text.len()];
    }
    let mut measure = |s: usize, e: usize| width(&text[byte(s)..byte(e)]);
    let mut spans = Vec::new();
    let (mut start, mut end) = words[0];
    for &(ws, we) in &words[1..] {
        if measure(start, we) <= max_width {
            end = we;
        } else {
            spans.push((start, end));
            (start, end) = (ws, we);
        }
    }
    spans.push((start, end));
    let mut ends = Vec::new();
    for (s, e) in spans {
        if measure(s, e) > max_width {
            let mut chunk = s;
            for i in s..e {
                if i > chunk && measure(chunk, i + 1) > max_width {
                    chunk = allowed_break(&chars, chunk, i);
                    ends.push(byte(chunk));
                }
            }
        }
        ends.push(byte(e));
    }
    *ends.last_mut().expect("a text with a word has a line") = text.len();
    ends
}

/// The text a call names in the core's memory.
fn text_of(at: u32, len: u32) -> Result<&'static str, Errno> {
    if len == 0 {
        return Ok("");
    }
    // SAFETY: bytes the core laid in its memory for the call, which it holds
    // until the call returns.
    let bytes = unsafe { core::slice::from_raw_parts(mem::at::<u8>(at as usize), len as usize) };
    core::str::from_utf8(bytes).map_err(|_| Errno::EBADMSG)
}

/// Writes the layout into the answer the call names (text_answer.json
/// `LAYOUT_ANSWER`, `LINE`).
fn layout(system: &SystemText, c: &TextLayout) -> Result<(), Errno> {
    let text = text_of(c.text_at, c.text_len)?;
    let head = LAYOUT_ANSWER::WORDS;
    let words = head + LINE::WORDS * c.lines as usize;
    // SAFETY: the block the core made for this answer, of `words` words, which
    // no one else touches until the call returns.
    let at = unsafe { core::slice::from_raw_parts_mut(mem::at::<u32>(c.at as usize), words) };
    let mut width = |s: &str| {
        // SAFETY: the bytes of `s`, which outlive the call.
        unsafe { (system.width)(f64::from(c.weight), c.px, s.as_ptr(), s.len()) }
    };
    let mut ends = line_ends(text, c.max_width, &mut width);
    ends.truncate(c.lines as usize);
    at[LAYOUT_ANSWER::cnt] = ends.len() as u32;
    let mut start = 0;
    for (i, &end) in ends.iter().enumerate() {
        let w = width(text[start..end].trim_matches(is_space)) as f32;
        let l = head + LINE::WORDS * i;
        at[l + LINE::end] = end as u32;
        at[l + LINE::width] = w.to_bits();
        start = end;
    }
    Ok(())
}

/// Draws the line into the pixels the call names.
fn raster(system: &SystemText, c: &TextRaster) -> Result<(), Errno> {
    let text = text_of(c.text_at, c.text_len)?;
    // SAFETY: the text's bytes and the pixels the core made for this answer,
    // both held until the call returns.
    unsafe {
        (system.raster)(
            f64::from(c.weight),
            c.px,
            c.color,
            text.as_ptr(),
            text.len(),
            c.x,
            c.width,
            c.height,
            mem::at::<u8>(c.at as usize),
        );
    }
    Ok(())
}

/// The system's text, as the text protocol's runtime end takes it.
struct Device(&'static SystemText);

impl text::Imports for Device {
    fn text_layout(&mut self, e: TextLayout) -> Outcome {
        layout(self.0, &e).map_err(MarshalError::new)?;
        Ok(Some(0))
    }

    fn text_raster(&mut self, e: TextRaster) -> Outcome {
        raster(self.0, &e).map_err(MarshalError::new)?;
        Ok(Some(0))
    }
}

/// Answers a text call here, before it returns, as the contract asks of
/// whatever answers a frame's major; a command the text protocol does not
/// carry is answered ENOSYS.
fn answer(at: u32) -> i32 {
    let Some(system) = SYSTEM.get() else {
        return Errno::ENOSYS as i32;
    };
    let frame = Frame::at(mem::core(), at);
    let Ok(pkt) = frame.pkt(mem::core(), at, MANIFEST.frame_bytes()) else {
        return Errno::EBADMSG as i32;
    };
    match text::serve(&mut Device(system), frame.tag, frame.cmd, &pkt, &[]) {
        Ok(val) => val.unwrap_or(0),
        Err(e) => e.errnum as i32,
    }
}

/// Has the native runtime answer the text protocol's calls here, on the
/// core's thread, on each of its majors.
pub(crate) fn install() {
    for major in text::MAJORS {
        native::handle(major, answer);
    }
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
    use xpute_native_host::jni::sys::{
        jbyteArray, jclass, jmethodID, jobject, jvalue, JNIEnv, JavaVM, JNI_OK, JNI_VERSION_1_6,
    };
    use xpute_native_host::jni::{self, sys::jint};

    use super::{text_on, SystemText};

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

#[cfg(test)]
#[path = "text_device.test.rs"]
mod test;
