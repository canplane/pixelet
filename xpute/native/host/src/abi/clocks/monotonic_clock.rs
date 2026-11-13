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
use crate::wait;
use std::rc::Rc;
use xpute_kit::status::errno::Errno;
use xpute_kit::status::error::MarshalError;
use xpute_kit::wire::xtp::{
    ArrayElem, BranchCursor, Cursor, PacketWriter, ScalarType, SequenceType, TreeReader,
};
use xpute_link::{Handle, Method, Object, Reply, Return, Tag};

/// The interface's handle, named by `xpute:clocks/monotonic-clock` (frame.json `NAMED`).
pub const HANDLE: Handle = 0x73f703;

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
    fn call(&self, m_: Method, tag_: Tag, args_: Option<BranchCursor<'_>>) -> Reply {
        let run = || -> Result<Reply, MarshalError> {
            Ok(match m_ {
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
                    let b_ = args_.ok_or(MarshalError::new(Errno::EBADMSG))?;
                    let how_long = b_.at(0).and_then(|c| c.get_u64())?;
                    self.0.wait_for(
                        how_long,
                        Return::new(
                            tag_,
                            link::on_cancel,
                            Box::new(move |got| {
                                let reply = match got {
                                    Ok(v) => Reply::from(0),
                                    Err(e) => Reply::from(e as i32),
                                };
                                link::task_return(
                                    tag_,
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
