// xpute-gpu-guest/abi/surface.rs
//
// GENERATED from gpu/wit (xpute-link-gen wit.rs)

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
use xpute_link::{Handle, Method, Object, Reply, Return, Tag};

/// The interface's qualified name: its package and its name.
pub const NAME: [&str; 2] = ["xpute:gpu", "surface"];

/// `[method]surface.resize`
pub const SURFACE_RESIZE: Method = 0;

pub trait Surface {
    fn resize(&self, width: u32, height: u32, dpr: f32) -> Result<(), Errno>;
}

struct SurfaceObject(Rc<dyn Surface>);

impl Object for SurfaceObject {
    fn call(&self, m: Method, tag: Tag, args: Option<BranchCursor<'_>>) -> Reply {
        let run = || -> Result<Reply, MarshalError> {
            Ok(match m {
                SURFACE_RESIZE => {
                    let b = args.ok_or(MarshalError::new(Errno::EBADMSG))?;
                    let width = b.at(0).and_then(|c| c.get_number().map(|v| v as u32))?;
                    let height = b.at(1).and_then(|c| c.get_number().map(|v| v as u32))?;
                    let dpr = b.at(2).and_then(|c| c.get_number().map(|v| v as f32))?;
                    match self.0.resize(width, height, dpr) {
                        Ok(v) => Reply::from(0),
                        Err(e) => Reply::from(e as i32),
                    }
                }
                _ => Reply::from(Errno::ENOSYS as i32),
            })
        };
        run().unwrap_or_else(|e| Reply::from(e.errnum as i32))
    }
}

/// The object over `imp`, for an opener to answer.
pub fn surface(imp: Rc<dyn Surface>) -> Rc<dyn Object> {
    Rc::new(SurfaceObject(imp))
}

pub fn surface_resize(surface: Handle, width: u32, height: u32, dpr: f32) -> Result<(), Errno> {
    let (r, data) = link::call(
        surface,
        SURFACE_RESIZE,
        Some((3, &|w: &mut PacketWriter| {
            w.u32(Some(width as u32));
            w.u32(Some(height as u32));
            w.f64(Some(dpr as f64));
        })),
    );
    xpute_link::answer(r).map(|_| ())
}
