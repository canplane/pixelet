// pxl-core/abi/text.rs
//
// GENERATED from spec/client/core/wit (xpute-gen spec/wit.rs)

#![allow(
    unused_imports,
    unused_variables,
    dead_code,
    clippy::all,
    reason = "generated from WIT"
)]

use std::cell::Cell;
use std::rc::Rc;
use xpute_core::link::{self, Handle, Method, Object, Reply, Return, Tag};
use xpute_core::wait::wait;
use xpute_kit::status::bug::OrBug;
use xpute_kit::status::errno::Errno;
use xpute_kit::status::error::MarshalError;
use xpute_kit::wire::xtp::{BranchCursor, Cursor, PacketWriter, TreeReader};

/// The singleton's URI.
pub const URI: [&str; 2] = ["pxl:core", "text"];

/// `layout`
pub const LAYOUT: Method = 0;
/// `raster`
pub const RASTER: Method = 1;

pub trait Text {
    fn layout(
        &self,
        weight: u32,
        px: f64,
        max_width: f64,
        at: u32,
        lines: u32,
        text_at: u32,
        text_len: u32,
    ) -> Result<u32, Errno>;
    fn raster(
        &self,
        weight: u32,
        px: f64,
        color: u32,
        at: u32,
        width: u32,
        height: u32,
        x: f64,
        text_at: u32,
        text_len: u32,
    ) -> Result<u32, Errno>;
}

struct TextObject(Rc<dyn Text>);

impl Object for TextObject {
    fn call(&self, m: Method, tag: Tag, args: Option<BranchCursor<'_>>) -> Reply {
        let run = || -> Result<Reply, MarshalError> {
            Ok(match m {
                LAYOUT => {
                    let b = args.ok_or(MarshalError::new(Errno::EBADMSG))?;
                    let weight = b.at(0).and_then(|c| c.get_number().map(|v| v as u32))?;
                    let px = b.at(1).and_then(|c| c.get_number().map(|v| v as f64))?;
                    let max_width = b.at(2).and_then(|c| c.get_number().map(|v| v as f64))?;
                    let at = b.at(3).and_then(|c| c.get_number().map(|v| v as u32))?;
                    let lines = b.at(4).and_then(|c| c.get_number().map(|v| v as u32))?;
                    let text_at = b.at(5).and_then(|c| c.get_number().map(|v| v as u32))?;
                    let text_len = b.at(6).and_then(|c| c.get_number().map(|v| v as u32))?;
                    match self
                        .0
                        .layout(weight, px, max_width, at, lines, text_at, text_len)
                    {
                        Ok(v) => Reply::from(v as i32),
                        Err(e) => Reply::from(e as i32),
                    }
                }
                RASTER => {
                    let b = args.ok_or(MarshalError::new(Errno::EBADMSG))?;
                    let weight = b.at(0).and_then(|c| c.get_number().map(|v| v as u32))?;
                    let px = b.at(1).and_then(|c| c.get_number().map(|v| v as f64))?;
                    let color = b.at(2).and_then(|c| c.get_number().map(|v| v as u32))?;
                    let at = b.at(3).and_then(|c| c.get_number().map(|v| v as u32))?;
                    let width = b.at(4).and_then(|c| c.get_number().map(|v| v as u32))?;
                    let height = b.at(5).and_then(|c| c.get_number().map(|v| v as u32))?;
                    let x = b.at(6).and_then(|c| c.get_number().map(|v| v as f64))?;
                    let text_at = b.at(7).and_then(|c| c.get_number().map(|v| v as u32))?;
                    let text_len = b.at(8).and_then(|c| c.get_number().map(|v| v as u32))?;
                    match self
                        .0
                        .raster(weight, px, color, at, width, height, x, text_at, text_len)
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

/// Mounts `imp` at the URI, one object for every open.
pub fn mount(imp: Rc<dyn Text>) {
    let o: Rc<dyn Object> = Rc::new(TextObject(imp));
    link::mount(link::uri(&URI), move |_, _| Ok(o.clone()));
}

thread_local! {
    static HANDLE: Cell<Option<Handle>> = const { Cell::new(None) };
}

/// Opens the singleton on the other side, once, at boot.
pub fn open() -> Result<(), Errno> {
    HANDLE.set(Some(link::open(&link::uri(&URI))?));
    Ok(())
}

/// The singleton's handle, which `open` opened.
pub fn handle() -> Handle {
    HANDLE.get().or_bug(Errno::EBADF)
}

pub fn layout(
    weight: u32,
    px: f64,
    max_width: f64,
    at: u32,
    lines: u32,
    text_at: u32,
    text_len: u32,
) -> Result<u32, Errno> {
    let (r, data) = link::call(
        handle(),
        LAYOUT,
        Some((7, &|w: &mut PacketWriter| {
            w.u32(Some(weight as u32));
            w.f64(Some(px as f64));
            w.f64(Some(max_width as f64));
            w.u32(Some(at as u32));
            w.u32(Some(lines as u32));
            w.u32(Some(text_at as u32));
            w.u32(Some(text_len as u32));
        })),
    );
    link::answer(r).map(|v| v as u32)
}

pub fn raster(
    weight: u32,
    px: f64,
    color: u32,
    at: u32,
    width: u32,
    height: u32,
    x: f64,
    text_at: u32,
    text_len: u32,
) -> Result<u32, Errno> {
    let (r, data) = link::call(
        handle(),
        RASTER,
        Some((9, &|w: &mut PacketWriter| {
            w.u32(Some(weight as u32));
            w.f64(Some(px as f64));
            w.u32(Some(color as u32));
            w.u32(Some(at as u32));
            w.u32(Some(width as u32));
            w.u32(Some(height as u32));
            w.f64(Some(x as f64));
            w.u32(Some(text_at as u32));
            w.u32(Some(text_len as u32));
        })),
    );
    link::answer(r).map(|v| v as u32)
}
