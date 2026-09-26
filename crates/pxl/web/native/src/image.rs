// pxl-native/image.rs

//! The images the shell's loads hand the kernel, decoded where the device is
//! (pxl_render::gpu::api's device backend), as the browser decodes them for
//! the page. The app calls this off the kernel's thread, so a large image
//! costs no turn.

/// Decodes the file of `bytes` bytes at `file` and holds it for the renderer:
/// the handle the kernel names it by, or 0 for a file that does not decode.
///
/// # Safety
/// `file` points at `bytes` bytes the caller owns for the call.
#[no_mangle]
pub unsafe extern "C" fn pxl_image_hold(file: *const u8, bytes: u32) -> u32 {
    // SAFETY: the caller's bytes.
    let file = unsafe { core::slice::from_raw_parts(file, bytes as usize) };
    pxl_render::gpu::api::hold_image(file).unwrap_or(0)
}
