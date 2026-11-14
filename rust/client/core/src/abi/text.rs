// pxl-core/abi/text.rs
//
// GENERATED from spec/client/core/wit (xpute-link-gen wit.rs)

#![allow(
    unused_imports,
    unused_variables,
    dead_code,
    clippy::all,
    reason = "generated from WIT"
)]

/// The interface's handle, named by `pxl:core/text` (frame.json `NAMED`).
pub const HANDLE: _rt::Handle = 0x61d944;

/// `width`
pub const WIDTH: _rt::Method = 0;
/// `raster`
pub const RASTER: _rt::Method = 1;

pub fn width<T0: Copy + Unpin + 'static>(
    weight: u32,
    px: f64,
    text: &mut [T0],
) -> Result<f64, _rt::Errno> {
    let text_at_ = _rt::mem::off(text.as_ptr().cast());
    let text_len_ = size_of_val(&text[..]) as u32;
    let (r, data) = _rt::link::call(
        HANDLE,
        WIDTH,
        Some((24, &|w_: &mut _rt::Lower| {
            w_.u32(0, *&weight);
            w_.f64(8, *&px);
            _rt::Lent {
                at: text_at_,
                len: text_len_,
            }
            .store(w_, 16);
        })),
    );
    _rt::answer(r).and_then(|_| {
        let r_ = _rt::Lift::new(data.as_deref().ok_or(_rt::Errno::EBADMSG)?);
        r_.f64(0)
    })
}

pub fn raster<T0: Copy + Unpin + 'static, T1: Copy + Unpin + 'static>(
    weight: u32,
    px: f64,
    color: u32,
    image: &mut [T0],
    width: u32,
    height: u32,
    x: f64,
    text: &mut [T1],
) -> Result<u32, _rt::Errno> {
    let image_at_ = _rt::mem::off(image.as_ptr().cast());
    let image_len_ = size_of_val(&image[..]) as u32;
    let text_at_ = _rt::mem::off(text.as_ptr().cast());
    let text_len_ = size_of_val(&text[..]) as u32;
    let (r, data) = _rt::link::call(
        HANDLE,
        RASTER,
        Some((56, &|w_: &mut _rt::Lower| {
            w_.u32(0, *&weight);
            w_.f64(8, *&px);
            w_.u32(16, *&color);
            _rt::Lent {
                at: image_at_,
                len: image_len_,
            }
            .store(w_, 20);
            w_.u32(28, *&width);
            w_.u32(32, *&height);
            w_.f64(40, *&x);
            _rt::Lent {
                at: text_at_,
                len: text_len_,
            }
            .store(w_, 48);
        })),
    );
    _rt::answer(r).and_then(|_| {
        let r_ = _rt::Lift::new(data.as_deref().ok_or(_rt::Errno::EBADMSG)?);
        r_.u32(0)
    })
}

mod _rt {
    pub use xpute_guest::link;
    pub use xpute_kit::status::errno::Errno;
    pub use xpute_link::abi::types::Lent;
    pub use xpute_link::{answer, mem, Handle, Lift, Lower, Method};
}
