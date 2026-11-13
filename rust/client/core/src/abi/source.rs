// pxl-core/abi/source.rs
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
use xpute_link::{Handle, Method, Object, Reply, Return, Tag};

/// The interface's handle, named by `pxl:core/source` (frame.json `NAMED`).
pub const HANDLE: Handle = 0x6a88bf;

/// The interface's qualified name: its package and its name.
pub const NAME: [&str; 2] = ["pxl:core", "source"];

/// `declare`
pub const DECLARE: Method = 0;

pub trait Source {
    fn declare(&self, min_zoom: u32, format: Vec<u32>, shard: Vec<String>) -> Result<(), Errno>;
}

struct SourceObject(Rc<dyn Source>);

impl Object for SourceObject {
    fn call(&self, m: Method, tag: Tag, args: Option<BranchCursor<'_>>) -> Reply {
        let run = || -> Result<Reply, MarshalError> {
            Ok(match m {
                DECLARE => {
                    let b = args.ok_or(MarshalError::new(Errno::EBADMSG))?;
                    let min_zoom = b.at(0).and_then(|c| c.get_number().map(|v| v as u32))?;
                    let format = b
                        .at(1)
                        .and_then(|c| c.get_array::<u32>().map(<[u32]>::to_vec))?;
                    let shard = b.at(2).and_then(|c| {
                        c.as_branch().and_then(|b| {
                            (0..b.len)
                                .map(|i| b.at(i).and_then(|c| c.get_str_ref().map(str::to_owned)))
                                .collect::<Result<Vec<_>, MarshalError>>()
                        })
                    })?;
                    match self.0.declare(min_zoom, format, shard) {
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

/// Serves `imp` under the interface's handle.
pub fn mount(imp: Rc<dyn Source>) {
    link::mount_named(HANDLE, Rc::new(SourceObject(imp)));
}

pub fn declare(min_zoom: u32, format: &[u32], shard: &[String]) -> Result<(), Errno> {
    let (r, data) = link::call(
        HANDLE,
        DECLARE,
        Some((3, &|w: &mut PacketWriter| {
            w.u32(Some(min_zoom as u32));
            w.array::<u32>(Some(&*format));
            w.branch(shard.len() as u32, |w| {
                for x in shard.iter() {
                    w.str(Some(&**x));
                }
            });
        })),
    );
    xpute_link::answer(r).map(|_| ())
}
