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

use std::rc::Rc;
use xpute_guest::link;
use xpute_guest::wait;
use xpute_kit::status::errno::Errno;
use xpute_kit::status::error::MarshalError;
use xpute_kit::wire::xtp::{
    ArrayElem, BranchCursor, Cursor, PacketWriter, ScalarType, SequenceType, TreeReader,
};
use xpute_link::abi::types::Lent;
use xpute_link::mem;
use xpute_link::{Handle, Method, Object, Reply, Return, Tag};

/// The interface's handle, named by `pxl:core/text` (frame.json `NAMED`).
pub const HANDLE: Handle = 0x61d944;

/// `width`
pub const WIDTH: Method = 0;
/// `raster`
pub const RASTER: Method = 1;

pub fn width<T0: Copy + Unpin + 'static>(
    weight: u32,
    px: f64,
    text: &mut [T0],
) -> Result<f64, Errno> {
    let text_at_ = mem::off(text.as_ptr().cast());
    let text_len_ = size_of_val(&text[..]) as u32;
    let (r, data) = link::call(
        HANDLE,
        WIDTH,
        Some((3, &|w_: &mut PacketWriter| {
            w_.u32(Some(weight as u32));
            w_.f64(Some(px as f64));
            Lent {
                at: text_at_,
                len: text_len_,
            }
            .write(w_);
        })),
    );
    xpute_link::answer(r).and_then(|_| {
        let p = data.as_deref().ok_or(Errno::EBADMSG)?;
        TreeReader::new(p)
            .and_then(|t| t.read_branch())
            .and_then(|b| b.at(0))
            .and_then(|c| c.get_number().map(|v| v as f64))
            .map_err(|e| e.errnum)
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
) -> Result<u32, Errno> {
    let image_at_ = mem::off(image.as_ptr().cast());
    let image_len_ = size_of_val(&image[..]) as u32;
    let text_at_ = mem::off(text.as_ptr().cast());
    let text_len_ = size_of_val(&text[..]) as u32;
    let (r, data) = link::call(
        HANDLE,
        RASTER,
        Some((8, &|w_: &mut PacketWriter| {
            w_.u32(Some(weight as u32));
            w_.f64(Some(px as f64));
            w_.u32(Some(color as u32));
            Lent {
                at: image_at_,
                len: image_len_,
            }
            .write(w_);
            w_.u32(Some(width as u32));
            w_.u32(Some(height as u32));
            w_.f64(Some(x as f64));
            Lent {
                at: text_at_,
                len: text_len_,
            }
            .write(w_);
        })),
    );
    xpute_link::answer(r).map(|v| v as u32)
}
