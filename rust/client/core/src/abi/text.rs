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
use xpute_guest::wait::wait;
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

/// The interface's qualified name: its package and its name.
pub const NAME: [&str; 2] = ["pxl:core", "text"];

/// `width`
pub const WIDTH: Method = 0;
/// `raster`
pub const RASTER: Method = 1;

pub trait Text {
    fn width(&self, weight: u32, px: f64, text: Lent) -> Result<f64, Errno>;
    fn raster(
        &self,
        weight: u32,
        px: f64,
        color: u32,
        image: Lent,
        width: u32,
        height: u32,
        x: f64,
        text: Lent,
    ) -> Result<u32, Errno>;
}

struct TextObject(Rc<dyn Text>);

impl Object for TextObject {
    fn call(&self, m: Method, tag: Tag, args: Option<BranchCursor<'_>>) -> Reply {
        let run = || -> Result<Reply, MarshalError> {
            Ok(match m {
                WIDTH => {
                    let b = args.ok_or(MarshalError::new(Errno::EBADMSG))?;
                    let weight = b.at(0).and_then(|c| c.get_number().map(|v| v as u32))?;
                    let px = b.at(1).and_then(|c| c.get_number().map(|v| v as f64))?;
                    let text = b.at(2).and_then(|c| Lent::read(c))?;
                    match self.0.width(weight, px, text) {
                        Ok(v) => Reply {
                            result: 0,
                            data: Some((
                                1,
                                Box::new(move |w: &mut PacketWriter| {
                                    w.f64(Some(v as f64));
                                }),
                            )),
                        },
                        Err(e) => Reply::from(e as i32),
                    }
                }
                RASTER => {
                    let b = args.ok_or(MarshalError::new(Errno::EBADMSG))?;
                    let weight = b.at(0).and_then(|c| c.get_number().map(|v| v as u32))?;
                    let px = b.at(1).and_then(|c| c.get_number().map(|v| v as f64))?;
                    let color = b.at(2).and_then(|c| c.get_number().map(|v| v as u32))?;
                    let image = b.at(3).and_then(|c| Lent::read(c))?;
                    let width = b.at(4).and_then(|c| c.get_number().map(|v| v as u32))?;
                    let height = b.at(5).and_then(|c| c.get_number().map(|v| v as u32))?;
                    let x = b.at(6).and_then(|c| c.get_number().map(|v| v as f64))?;
                    let text = b.at(7).and_then(|c| Lent::read(c))?;
                    match self
                        .0
                        .raster(weight, px, color, image, width, height, x, text)
                    {
                        Ok(v) => Reply::from(v as i32),
                        Err(e) => Reply::from(e as i32),
                    }
                }
                _ => Reply::from(Errno::ENOSYS as i32),
            })
        };
        run().unwrap_or_else(|e| Reply::from(e.errnum as i32))
    }
}

/// Serves `imp` under the interface's handle.
pub fn mount(imp: Rc<dyn Text>) {
    link::mount_named(HANDLE, Rc::new(TextObject(imp)));
}

pub fn width<T0: Copy + Unpin + 'static>(
    weight: u32,
    px: f64,
    text: &mut [T0],
) -> Result<f64, Errno> {
    let text_at = mem::off(text.as_ptr().cast());
    let text_len = size_of_val(&text[..]) as u32;
    let (r, data) = link::call(
        HANDLE,
        WIDTH,
        Some((3, &|w: &mut PacketWriter| {
            w.u32(Some(weight as u32));
            w.f64(Some(px as f64));
            Lent {
                at: text_at,
                len: text_len,
            }
            .write(w);
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
    let image_at = mem::off(image.as_ptr().cast());
    let image_len = size_of_val(&image[..]) as u32;
    let text_at = mem::off(text.as_ptr().cast());
    let text_len = size_of_val(&text[..]) as u32;
    let (r, data) = link::call(
        HANDLE,
        RASTER,
        Some((8, &|w: &mut PacketWriter| {
            w.u32(Some(weight as u32));
            w.f64(Some(px as f64));
            w.u32(Some(color as u32));
            Lent {
                at: image_at,
                len: image_len,
            }
            .write(w);
            w.u32(Some(width as u32));
            w.u32(Some(height as u32));
            w.f64(Some(x as f64));
            Lent {
                at: text_at,
                len: text_len,
            }
            .write(w);
        })),
    );
    xpute_link::answer(r).map(|v| v as u32)
}
