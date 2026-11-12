// xpute-core/abi/clock.rs
//
// GENERATED from wit (xpute-gen spec/wit.rs)

#![allow(
    unused_imports,
    unused_variables,
    dead_code,
    clippy::all,
    reason = "generated from WIT"
)]

use crate::link::{self, Handle, Method, Object, Reply, Return, Tag};
use crate::wait::wait;
use std::cell::Cell;
use std::rc::Rc;
use xpute_kit::status::bug::OrBug;
use xpute_kit::status::errno::Errno;
use xpute_kit::status::error::MarshalError;
use xpute_kit::wire::xtp::{BranchCursor, Cursor, PacketWriter, TreeReader};

/// The singleton's URI.
pub const URI: [&str; 2] = ["xpute:core", "clock"];

/// `now`
pub const NOW: Method = 0;

pub trait Clock {
    fn now(&self, at: u32) -> Result<(), Errno>;
}

struct ClockObject(Rc<dyn Clock>);

impl Object for ClockObject {
    fn call(&self, m: Method, tag: Tag, args: Option<BranchCursor<'_>>) -> Reply {
        let run = || -> Result<Reply, MarshalError> {
            Ok(match m {
                NOW => {
                    let b = args.ok_or(MarshalError::new(Errno::EBADMSG))?;
                    let at = b.at(0).and_then(|c| c.get_number().map(|v| v as u32))?;
                    match self.0.now(at) {
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
pub fn mount(imp: Rc<dyn Clock>) {
    let o: Rc<dyn Object> = Rc::new(ClockObject(imp));
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

pub fn now(at: u32) -> Result<(), Errno> {
    let (r, data) = link::call(
        handle(),
        NOW,
        Some((1, &|w: &mut PacketWriter| {
            w.u32(Some(at as u32));
        })),
    );
    link::answer(r).map(|_| ())
}
