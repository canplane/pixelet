// xpute-native-host/abi/clocks/monotonic_clock.rs
//
// GENERATED from wit/clocks (xpute-link-gen wit.rs)

#![allow(
    unused_imports,
    unused_variables,
    dead_code,
    clippy::all,
    reason = "generated from WIT"
)]

use crate::link;
use crate::wait::wait;
use core::future::Future;
use std::rc::Rc;
use xpute_kit::status::errno::Errno;
use xpute_kit::status::error::MarshalError;
use xpute_kit::wire::xtp::{
    ArrayElem, BranchCursor, Cursor, PacketWriter, ScalarType, SequenceType, TreeReader,
};
use xpute_link::{Handle, Method, Object, Reply, Return, Tag};

/// The interface's handle, named by `xpute:clocks/monotonic-clock` (frame.json `NAMED`).
pub const HANDLE: Handle = 0x73f703;

/// The interface's qualified name: its package and its name.
pub const NAME: [&str; 2] = ["xpute:clocks", "monotonic-clock"];

/// `now`
pub const NOW: Method = 0;
/// `wait-for`
pub const WAIT_FOR: Method = 1;

pub type Mark = u64;

pub trait MonotonicClock {
    fn now(&self) -> Result<u64, Errno>;
    fn wait_for(&self, how_long: u64, task_return: Return<()>);
}

struct MonotonicClockObject(Rc<dyn MonotonicClock>);

impl Object for MonotonicClockObject {
    fn call(&self, m: Method, tag: Tag, args: Option<BranchCursor<'_>>) -> Reply {
        let run = || -> Result<Reply, MarshalError> {
            Ok(match m {
                NOW => match self.0.now() {
                    Ok(v) => Reply {
                        result: 0,
                        data: Some((
                            1,
                            Box::new(move |w: &mut PacketWriter| {
                                w.u64(Some(v));
                            }),
                        )),
                    },
                    Err(e) => Reply::from(e as i32),
                },
                WAIT_FOR => {
                    let b = args.ok_or(MarshalError::new(Errno::EBADMSG))?;
                    let how_long = b.at(0).and_then(|c| c.get_u64())?;
                    self.0.wait_for(
                        how_long,
                        Return::new(
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
                        ),
                    );
                    Reply::from(Errno::EINPROGRESS as i32)
                }
                _ => Reply::from(Errno::ENOSYS as i32),
            })
        };
        run().unwrap_or_else(|e| Reply::from(e.errnum as i32))
    }
}

/// Serves `imp` under the interface's handle.
pub fn mount(imp: Rc<dyn MonotonicClock>) {
    link::mount_named(HANDLE, Rc::new(MonotonicClockObject(imp)));
}

pub fn now() -> Result<u64, Errno> {
    let (r, data) = link::call(HANDLE, NOW, None);
    xpute_link::answer(r).and_then(|_| {
        let p = data.as_deref().ok_or(Errno::EBADMSG)?;
        TreeReader::new(p)
            .and_then(|t| t.read_branch())
            .and_then(|b| b.at(0))
            .and_then(|c| c.get_u64())
            .map_err(|e| e.errnum)
    })
}

pub fn wait_for(how_long: u64) -> impl Future<Output = Result<(), Errno>> {
    let sent = wait(
        HANDLE,
        WAIT_FOR,
        Some((1, &|w: &mut PacketWriter| {
            w.u64(Some(how_long));
        })),
        (),
    );
    async move {
        let (r, data, lent) = sent.await;
        xpute_link::answer(r).map(|_| ())
    }
}
