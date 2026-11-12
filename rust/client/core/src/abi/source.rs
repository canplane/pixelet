// pxl-core/abi/source.rs
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
pub const URI: [&str; 2] = ["pxl:core", "source"];

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
                    let format = b.at(1).and_then(|c| {
                        c.as_branch().and_then(|b| {
                            (0..b.len)
                                .map(|i| b.at(i).and_then(|c| c.get_number().map(|v| v as u32)))
                                .collect::<Result<Vec<_>, MarshalError>>()
                        })
                    })?;
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

/// Mounts `imp` at the URI, one object for every open.
pub fn mount(imp: Rc<dyn Source>) {
    let o: Rc<dyn Object> = Rc::new(SourceObject(imp));
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

pub fn declare(min_zoom: u32, format: &[u32], shard: &[String]) -> Result<(), Errno> {
    let (r, data) = link::call(
        handle(),
        DECLARE,
        Some((3, &|w: &mut PacketWriter| {
            w.u32(Some(min_zoom as u32));
            w.branch(format.len() as u32, |w| {
                for x in format.iter() {
                    w.u32(Some((*x) as u32));
                }
            });
            w.branch(shard.len() as u32, |w| {
                for x in shard.iter() {
                    w.str(Some(&*(*x)));
                }
            });
        })),
    );
    link::answer(r).map(|_| ())
}
