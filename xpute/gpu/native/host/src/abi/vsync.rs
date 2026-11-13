// xpute-gpu-native-host/abi/vsync.rs
//
// GENERATED from gpu/wit (xpute-link-gen wit.rs)

#![allow(
    unused_imports,
    unused_variables,
    dead_code,
    clippy::all,
    reason = "generated from WIT"
)]

use core::future::Future;
use std::rc::Rc;
use xpute_kit::status::errno::Errno;
use xpute_kit::status::error::MarshalError;
use xpute_kit::wire::xtp::{
    ArrayElem, BranchCursor, Cursor, PacketWriter, ScalarType, SequenceType, TreeReader,
};
use xpute_link::{Handle, Method, Object, Reply, Return, Tag};
use xpute_native_host::link;
use xpute_native_host::wait::wait;

/// The interface's handle, named by `xpute:gpu/vsync` (frame.json `NAMED`).
pub const HANDLE: Handle = 0x42eda6;

/// The interface's qualified name: its package and its name.
pub const NAME: [&str; 2] = ["xpute:gpu", "vsync"];

/// `next`
pub const NEXT: Method = 0;

pub trait Vsync {
    fn next(&self, task_return: Return<()>);
}

struct VsyncObject(Rc<dyn Vsync>);

impl Object for VsyncObject {
    fn call(&self, m: Method, tag: Tag, args: Option<BranchCursor<'_>>) -> Reply {
        let run = || -> Result<Reply, MarshalError> {
            Ok(match m {
                NEXT => {
                    self.0.next(Return::new(
                        tag,
                        link::on_cancel,
                        Box::new(move |got| {
                            let reply = match got {
                                Ok(v) => Reply::from(0),
                                Err(e) => Reply::from(e as i32),
                            };
                            link::task_return(
                                tag,
                                reply.result,
                                reply
                                    .data
                                    .as_ref()
                                    .map(|(n, w)| (*n, &**w as &dyn Fn(&mut PacketWriter))),
                            )
                        }),
                    ));
                    Reply::from(Errno::EINPROGRESS as i32)
                }
                _ => Reply::from(Errno::ENOSYS as i32),
            })
        };
        run().unwrap_or_else(|e| Reply::from(e.errnum as i32))
    }
}

/// Serves `imp` under the interface's handle.
pub fn mount(imp: Rc<dyn Vsync>) {
    link::mount_named(HANDLE, Rc::new(VsyncObject(imp)));
}

pub fn next() -> impl Future<Output = Result<(), Errno>> {
    let sent = wait(HANDLE, NEXT, None, ());
    async move {
        let (r, data, lent) = sent.await;
        xpute_link::answer(r).map(|_| ())
    }
}
