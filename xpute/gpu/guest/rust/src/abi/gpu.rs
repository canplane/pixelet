// xpute-gpu-guest/abi/gpu.rs
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
use xpute_guest::link;
use xpute_guest::wait::wait;
use xpute_kit::status::errno::Errno;
use xpute_kit::status::error::MarshalError;
use xpute_kit::wire::xtp::{
    ArrayElem, BranchCursor, Cursor, PacketWriter, ScalarType, SequenceType, TreeReader,
};
use xpute_link::abi::types::Lent;
use xpute_link::Lend;
use xpute_link::{Handle, Method, Object, Reply, Return, Tag};

/// The interface's handle, named by `xpute:gpu/gpu` (frame.json `NAMED`).
pub const HANDLE: Handle = 0x79d455;

/// The interface's qualified name: its package and its name.
pub const NAME: [&str; 2] = ["xpute:gpu", "gpu"];

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
    fn call(&self, m: Method, tag: Tag, args: Option<BranchCursor<'_>>) -> Reply {
        let run = || -> Result<Reply, MarshalError> {
            Ok(match m {
                RUN => {
                    let b = args.ok_or(MarshalError::new(Errno::EBADMSG))?;
                    let records = b.at(0).and_then(|c| Lent::read(c))?;
                    self.0.run(
                        records,
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
pub fn mount(imp: Rc<dyn Gpu>) {
    link::mount_named(HANDLE, Rc::new(GpuObject(imp)));
}

pub fn run<L: Lend>(records: L) -> impl Future<Output = (L, Result<(), Errno>)> {
    let mut records = records;
    let (records_at, records_len) = Lend::span(&mut records);
    let sent = wait(
        HANDLE,
        RUN,
        Some((1, &|w: &mut PacketWriter| {
            Lent {
                at: records_at,
                len: records_len,
            }
            .write(w);
        })),
        records,
    );
    async move {
        let (r, data, lent) = sent.await;
        (lent, xpute_link::answer(r).map(|_| ()))
    }
}

pub trait Device {
    fn lost(&self, block: Lent, task_return: Return<()>);
    fn ready(&self, task_return: Return<u32>);
    fn settled(&self, task_return: Return<()>);
}

struct DeviceObject(Rc<dyn Device>);

impl Object for DeviceObject {
    fn call(&self, m: Method, tag: Tag, args: Option<BranchCursor<'_>>) -> Reply {
        let run = || -> Result<Reply, MarshalError> {
            Ok(match m {
                DEVICE_LOST => {
                    let b = args.ok_or(MarshalError::new(Errno::EBADMSG))?;
                    let block = b.at(0).and_then(|c| Lent::read(c))?;
                    self.0.lost(
                        block,
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
                DEVICE_READY => {
                    self.0.ready(Return::new(
                        tag,
                        link::on_cancel,
                        Box::new(move |got| {
                            let reply = match got {
                                Ok(v) => Reply::from(v as i32),
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
                DEVICE_SETTLED => {
                    self.0.settled(Return::new(
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

/// The object over `imp`, for an opener to answer.
pub fn device(imp: Rc<dyn Device>) -> Rc<dyn Object> {
    Rc::new(DeviceObject(imp))
}

pub fn device_lost<L: Lend>(
    device: Handle,
    block: L,
) -> impl Future<Output = (L, Result<(), Errno>)> {
    let mut block = block;
    let (block_at, block_len) = Lend::span(&mut block);
    let sent = wait(
        device,
        DEVICE_LOST,
        Some((1, &|w: &mut PacketWriter| {
            Lent {
                at: block_at,
                len: block_len,
            }
            .write(w);
        })),
        block,
    );
    async move {
        let (r, data, lent) = sent.await;
        (lent, xpute_link::answer(r).map(|_| ()))
    }
}

pub fn device_ready(device: Handle) -> impl Future<Output = Result<u32, Errno>> {
    let sent = wait(device, DEVICE_READY, None, ());
    async move {
        let (r, data, lent) = sent.await;
        xpute_link::answer(r).map(|v| v as u32)
    }
}

pub fn device_settled(device: Handle) -> impl Future<Output = Result<(), Errno>> {
    let sent = wait(device, DEVICE_SETTLED, None, ());
    async move {
        let (r, data, lent) = sent.await;
        xpute_link::answer(r).map(|_| ())
    }
}
