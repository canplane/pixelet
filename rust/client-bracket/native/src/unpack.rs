// pxl-native/unpack.rs

//! A body the app's fetch hands over still zstd, decoded before anything
//! reads it: the device asks for zstd, as the bake stores its objects, and
//! undoes it in its transport (ts/app/native/src/origin.ts), as a
//! browser undoes it in its own. Any frame RFC 8878 allows decodes, a
//! stored object's or one the edge made on the fly, which need not say its
//! size: so the decoded bytes are the library's, handed out and let go by
//! the caller. Called from the module's queue, never the JavaScript thread
//! or the kernel's: it touches only the caller's bytes and its own, so it
//! takes no handoff (lib.rs `on_kernel`).

use ruzstd::decoding::errors::{FrameDecoderError, ReadFrameHeaderError};
use ruzstd::decoding::{BlockDecodingStrategy, FrameDecoder};

/// The blocks decoded before what they made is drained: a bound on the
/// window the decoder holds beside the output.
const DRAIN_BYTES: usize = 1 << 20;

/// Every frame of `body` decoded, its skippable frames passed over; none
/// where one does not decode.
pub fn decode(mut body: &[u8]) -> Option<Vec<u8>> {
    let mut frames = FrameDecoder::new();
    let mut raw = Vec::new();
    while !body.is_empty() {
        match frames.init(&mut body) {
            Ok(()) => {}
            Err(FrameDecoderError::ReadFrameHeaderError(ReadFrameHeaderError::SkipFrame {
                length,
                ..
            })) => {
                body = body.get(length as usize..)?;
                continue;
            }
            Err(_) => return None,
        }
        while !frames.is_finished() {
            frames
                .decode_blocks(&mut body, BlockDecodingStrategy::UptoBytes(DRAIN_BYTES))
                .ok()?;
            frames.collect_to_writer(&mut raw).ok()?;
        }
        frames.collect_to_writer(&mut raw).ok()?;
    }
    Some(raw)
}

/// Decodes the zstd body of `len` bytes at `src`: 0, with the bytes at
/// `*out`, `*out_len` long, which are the library's until
/// `pxl_unpacked_free`; -1 where it does not decode, nothing handed out.
///
/// # Safety
/// `src` points at `len` readable bytes, `out` and `out_len` at writable words.
#[no_mangle]
pub unsafe extern "C" fn pxl_unpack(
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
pub unsafe extern "C" fn pxl_unpacked_free(out: *mut u8, len: usize) {
    // SAFETY: the boxed slice `pxl_unpack` made, as it made it.
    drop(unsafe { Box::from_raw(core::ptr::slice_from_raw_parts_mut(out, len)) });
}
