// xpute-core/abi/stop.rs
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
pub const URI: [&str; 2] = ["xpute:core", "stop"];

/// `stopped`
pub const STOPPED: Method = 0;

pub trait Stop {
    fn stopped(&self, errnum: i32, file: String, line: u32, n1: u64, n2: u64) -> Result<(), Errno>;
}

struct StopObject(Rc<dyn Stop>);

impl Object for StopObject {
    fn call(&self, m: Method, tag: Tag, args: Option<BranchCursor<'_>>) -> Reply {
        let run = || -> Result<Reply, MarshalError> {
            Ok(match m {
                STOPPED => {
                    let b = args.ok_or(MarshalError::new(Errno::EBADMSG))?;
                    let errnum = b.at(0).and_then(|c| c.get_number().map(|v| v as i32))?;
                    let file = b.at(1).and_then(|c| c.get_str_ref().map(str::to_owned))?;
                    let line = b.at(2).and_then(|c| c.get_number().map(|v| v as u32))?;
                    let n1 = b.at(3).and_then(|c| c.get_u64())?;
                    let n2 = b.at(4).and_then(|c| c.get_u64())?;
                    match self.0.stopped(errnum, file, line, n1, n2) {
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
pub fn mount(imp: Rc<dyn Stop>) {
    let o: Rc<dyn Object> = Rc::new(StopObject(imp));
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

pub fn stopped(errnum: i32, file: &str, line: u32, n1: u64, n2: u64) -> Result<(), Errno> {
    let (r, data) = link::call(
        handle(),
        STOPPED,
        Some((5, &|w: &mut PacketWriter| {
            w.i32(Some(errnum as i32));
            w.str(Some(&*file));
            w.u32(Some(line as u32));
            w.u64(Some(n1));
            w.u64(Some(n2));
        })),
    );
    link::answer(r).map(|_| ())
}
