// xpute-gpu-native-host/abi/gpu.rs
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
use xpute_link::abi::types::Lent;
use xpute_link::{Handle, Method, Object, Reply, Return, Tag};
use xpute_native_host::link;
use xpute_native_host::wait;

/// The interface's handle, named by `xpute:gpu/gpu` (frame.json `NAMED`).
pub const HANDLE: Handle = 0x79d455;

/// `run`
pub const RUN: Method = 0;
/// `[method]device.lost`
pub const DEVICE_LOST: Method = 0;
/// `[method]device.ready`
pub const DEVICE_READY: Method = 1;
/// `[method]device.settled`
pub const DEVICE_SETTLED: Method = 2;

pub trait Gpu {
    fn run(&self, records: Lent, task_return: Return<()>);
}

struct GpuObject(Rc<dyn Gpu>);

impl Object for GpuObject {
    fn call(&self, m_: Method, tag_: Tag, args_: Option<BranchCursor<'_>>) -> Reply {
        let run = || -> Result<Reply, MarshalError> {
            Ok(match m_ {
                RUN => {
                    let b_ = args_.ok_or(MarshalError::new(Errno::EBADMSG))?;
                    let records = b_.at(0).and_then(|c| Lent::read(c))?;
                    self.0.run(
                        records,
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
pub fn mount(imp: Rc<dyn Gpu>) {
    link::mount_named(HANDLE, Rc::new(GpuObject(imp)));
}

pub trait Device {
    fn lost(&self, block: Lent, task_return: Return<()>);
    fn ready(&self, task_return: Return<u32>);
    fn settled(&self, task_return: Return<()>);
}

struct DeviceObject(Rc<dyn Device>);

impl Object for DeviceObject {
    fn call(&self, m_: Method, tag_: Tag, args_: Option<BranchCursor<'_>>) -> Reply {
        let run = || -> Result<Reply, MarshalError> {
            Ok(match m_ {
                DEVICE_LOST => {
                    let b_ = args_.ok_or(MarshalError::new(Errno::EBADMSG))?;
                    let block = b_.at(0).and_then(|c| Lent::read(c))?;
                    self.0.lost(
                        block,
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
                DEVICE_READY => {
                    self.0.ready(Return::new(
                        tag_,
                        link::on_cancel,
                        Box::new(move |got| {
                            let reply = match got {
                                Ok(v) => Reply::from(v as i32),
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
                    ));
                    Reply::from(Errno::EINPROGRESS as i32)
                }
                DEVICE_SETTLED => {
                    self.0.settled(Return::new(
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
                    ));
                    Reply::from(Errno::EINPROGRESS as i32)
                }
                _ => Reply::from(Errno::ENOSYS as i32),
            })
        };
        run().unwrap_or_else(|e| Reply::from(e.errnum as i32))
    }
}

/// The object over `imp`, for an opener to answer.
pub fn device(imp: Rc<dyn Device>) -> Rc<dyn Object> {
    Rc::new(DeviceObject(imp))
}
