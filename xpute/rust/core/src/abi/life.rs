// xpute-core/abi/life.rs
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
pub const URI: [&str; 2] = ["xpute:core", "life"];

/// `hidden`
pub const HIDDEN: Method = 0;
/// `shown`
pub const SHOWN: Method = 1;

pub trait Life {
    fn hidden(&self) -> Result<(), Errno>;
    fn shown(&self) -> Result<(), Errno>;
}

struct LifeObject(Rc<dyn Life>);

impl Object for LifeObject {
    fn call(&self, m: Method, tag: Tag, args: Option<BranchCursor<'_>>) -> Reply {
        let run = || -> Result<Reply, MarshalError> {
            Ok(match m {
                HIDDEN => match self.0.hidden() {
                    Ok(v) => Reply::from(0),
                    Err(e) => Reply::from(e as i32),
                },
                SHOWN => match self.0.shown() {
                    Ok(v) => Reply::from(0),
                    Err(e) => Reply::from(e as i32),
                },
                _ => Reply::from(Errno::ENOSYS as i32),
            })
        };
        run().unwrap_or_else(|e| Reply::from(e.errnum as i32))
    }
}

/// Mounts `imp` at the URI, one object for every open.
pub fn mount(imp: Rc<dyn Life>) {
    let o: Rc<dyn Object> = Rc::new(LifeObject(imp));
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

pub fn hidden() -> Result<(), Errno> {
    let (r, data) = link::call(handle(), HIDDEN, None);
    link::answer(r).map(|_| ())
}

pub fn shown() -> Result<(), Errno> {
    let (r, data) = link::call(handle(), SHOWN, None);
    link::answer(r).map(|_| ())
}
