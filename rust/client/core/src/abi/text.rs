// pxl-core/abi/text.rs
//
// GENERATED from spec/client/core/wit (xpute-link-gen wit.rs)

/// The interface's handle, named by `pxl:core/text` (frame.json5 `NAMED`).
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
    let (text_at_, text_len_) = _rt::mem::span(&text[..]);
    let (r, data) = _rt::link::call(
        HANDLE,
        WIDTH,
        Some((32, &|w_: &mut _rt::Lower| {
            w_.u32(0, weight);
            w_.f64(8, px);
            _rt::Lent {
                at: text_at_,
                len: text_len_,
            }
            .store(w_, 16);
        })),
    );
    _rt::answer(r).and_then(|_| {
        let r_ = _rt::Lift::new(data.ok_or(_rt::Errno::EBADMSG)?);
        r_.f64(0)
    })
}

#[expect(
    clippy::too_many_arguments,
    reason = "the WIT function's parameters, each its own"
)]
pub fn raster<T0: Copy + Unpin + 'static, T1: Copy + Unpin + 'static>(
    weight: u32,
    px: f64,
    color: u32,
    image: &mut [T0],
    width: u32,
    height: u32,
    x: f64,
    text: &mut [T1],
) -> Result<(), _rt::Errno> {
    let (image_at_, image_len_) = _rt::mem::span(&image[..]);
    let (text_at_, text_len_) = _rt::mem::span(&text[..]);
    let (r, _) = _rt::link::call(
        HANDLE,
        RASTER,
        Some((72, &|w_: &mut _rt::Lower| {
            w_.u32(0, weight);
            w_.f64(8, px);
            w_.u32(16, color);
            _rt::Lent {
                at: image_at_,
                len: image_len_,
            }
            .store(w_, 24);
            w_.u32(40, width);
            w_.u32(44, height);
            w_.f64(48, x);
            _rt::Lent {
                at: text_at_,
                len: text_len_,
            }
            .store(w_, 56);
        })),
    );
    _rt::answer(r)
}

mod _rt {
    pub(super) use xpute_guest::link;
    pub(super) use xpute_kit::status::errno::Errno;
    pub(super) use xpute_link::abi::types::Lent;
    pub(super) use xpute_link::{answer, mem, Handle, Lift, Lower, Method};
}
