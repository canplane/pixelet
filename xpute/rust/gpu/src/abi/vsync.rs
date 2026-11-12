// xpute-gpu/abi/vsync.rs
//
// GENERATED from wit/gpu (xpute-gen spec/wit.rs)

#![allow(
    unused_imports,
    unused_variables,
    dead_code,
    clippy::all,
    reason = "generated from WIT"
)]

use std::cell::Cell;
use std::rc::Rc;
use xpute_core::link::{self, Done, Handle, Method, Object, Reply, Tag};
use xpute_core::wait::wait;
use xpute_kit::status::bug::OrBug;
use xpute_kit::status::errno::Errno;
use xpute_kit::status::error::MarshalError;
use xpute_kit::wire::xtp::{BranchCursor, Cursor, PacketWriter, TreeReader};

/// The singleton's URI.
pub const URI: [&str; 2] = ["xpute:gpu", "vsync"];

/// `next`
pub const NEXT: Method = 0;

pub trait Vsync {
    fn next(&self, done: Done<()>);
}

struct VsyncObject(Rc<dyn Vsync>);

impl Object for VsyncObject {
    fn call(&self, m: Method, tag: Tag, args: Option<BranchCursor<'_>>) -> Reply {
        let run = || -> Result<Reply, MarshalError> {
            Ok(match m {
                NEXT => {
                    self.0.next(Box::new(move |got| {
                        let reply = match got {
                            Ok(v) => Reply::from(0),
                            Err(e) => Reply::from(e as i32),
                        };
                        link::complete(
                            tag,
                            reply.result,
                            reply
                                .data
                                .as_ref()
                                .map(|(n, w)| (*n, &**w as &dyn Fn(&mut PacketWriter))),
                        )
                    }));
                    Reply::from(Errno::EINPROGRESS as i32)
                }
                _ => Reply::from(Errno::ENOSYS as i32),
            })
        };
        run().unwrap_or_else(|e| Reply::from(e.errnum as i32))
    }
}

/// Mounts `imp` at the URI, one object for every open.
pub fn mount(imp: Rc<dyn Vsync>) {
    let o: Rc<dyn Object> = Rc::new(VsyncObject(imp));
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

pub fn next() -> impl core::future::Future<Output = Result<(), Errno>> {
    let sent = wait(handle(), NEXT, None, ());
    async move {
        let (r, data, lent) = sent.await;
        link::answer(r).map(|_| ())
    }
}

pub fn next_then(then: impl FnOnce(Result<(), Errno>) + 'static) {
    let lent = ();
    link::send(
        handle(),
        NEXT,
        None,
        Some(Box::new(move |r, data| then(link::answer(r).map(|_| ())))),
    );
}
