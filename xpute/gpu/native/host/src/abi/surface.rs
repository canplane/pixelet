// xpute-gpu-native-host/abi/surface.rs
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
use xpute_kit::status::errno::Errno;
use xpute_kit::status::error::MarshalError;
use xpute_kit::wire::xtp::{
    ArrayElem, BranchCursor, Cursor, PacketWriter, ScalarType, SequenceType, TreeReader,
};
use xpute_link::{Handle, Method, Object, Reply, Return, Tag};
use xpute_native_host::link;
use xpute_native_host::wait;

/// `[method]surface.resize`
pub const SURFACE_RESIZE: Method = 0;

pub fn surface_resize(surface_: Handle, width: u32, height: u32, dpr: f32) -> Result<(), Errno> {
    let (r, data) = link::call(
        surface_,
        SURFACE_RESIZE,
        Some((3, &|w_: &mut PacketWriter| {
            w_.u32(Some(width as u32));
            w_.u32(Some(height as u32));
            w_.f64(Some(dpr as f64));
        })),
    );
    xpute_link::answer(r).map(|_| ())
}
