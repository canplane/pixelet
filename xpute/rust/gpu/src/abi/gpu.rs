// xpute-gpu/abi/gpu.rs
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
pub const URI: [&str; 2] = ["xpute:gpu", "gpu"];

/// `run`
pub const RUN: Method = 0;
/// `[method]device.lost`
pub const DEVICE_LOST: Method = 0;
/// `[method]device.ready`
pub const DEVICE_READY: Method = 1;
/// `[method]device.settled`
pub const DEVICE_SETTLED: Method = 2;

pub trait Gpu {
    fn run(&self, records: xpute_core::abi::link::Lent, done: Done<()>);
}

struct GpuObject(Rc<dyn Gpu>);

impl Object for GpuObject {
    fn call(&self, m: Method, tag: Tag, args: Option<BranchCursor<'_>>) -> Reply {
        let run = || -> Result<Reply, MarshalError> {
            Ok(match m {
                RUN => {
                    let b = args.ok_or(MarshalError::new(Errno::EBADMSG))?;
                    let records = b.at(0).and_then(|c| xpute_core::abi::link::Lent::read(c))?;
                    self.0.run(
                        records,
                        Box::new(move |got| {
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
                        }),
                    );
                    Reply::from(Errno::EINPROGRESS as i32)
                }
                _ => Reply::from(Errno::ENOSYS as i32),
            })
        };
        run().unwrap_or_else(|e| Reply::from(e.errnum as i32))
    }
}

/// Mounts `imp` at the URI, one object for every open.
pub fn mount(imp: Rc<dyn Gpu>) {
    let o: Rc<dyn Object> = Rc::new(GpuObject(imp));
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

pub fn run<L: xpute_core::link::Lend>(
    records: L,
) -> impl core::future::Future<Output = (L, Result<(), Errno>)> {
    let mut records = records;
    let (lent_at, lent_len) = xpute_core::link::Lend::span(&mut records);
    let sent = wait(
        handle(),
        RUN,
        Some((
            1,
            Box::new(move |w: &mut PacketWriter| {
                xpute_core::abi::link::Lent {
                    at: lent_at,
                    len: lent_len,
                }
                .write(w);
            }),
        )),
        records,
    );
    async move {
        let (r, data, lent) = sent.await;
        (lent, link::answer(r).map(|_| ()))
    }
}

pub fn run_then<L: xpute_core::link::Lend>(
    records: L,
    then: impl FnOnce((L, Result<(), Errno>)) + 'static,
) {
    let mut records = records;
    let (lent_at, lent_len) = xpute_core::link::Lend::span(&mut records);
    let lent = records;
    link::send(
        handle(),
        RUN,
        Some((1, &|w: &mut PacketWriter| {
            xpute_core::abi::link::Lent {
                at: lent_at,
                len: lent_len,
            }
            .write(w);
        })),
        Some(Box::new(move |r, data| {
            then((lent, link::answer(r).map(|_| ())))
        })),
    );
}

pub trait Device {
    fn lost(&self, block: xpute_core::abi::link::Lent, done: Done<()>);
    fn ready(&self, done: Done<u32>);
    fn settled(&self, done: Done<()>);
}

struct DeviceObject(Rc<dyn Device>);

impl Object for DeviceObject {
    fn call(&self, m: Method, tag: Tag, args: Option<BranchCursor<'_>>) -> Reply {
        let run = || -> Result<Reply, MarshalError> {
            Ok(match m {
                DEVICE_LOST => {
                    let b = args.ok_or(MarshalError::new(Errno::EBADMSG))?;
                    let block = b.at(0).and_then(|c| xpute_core::abi::link::Lent::read(c))?;
                    self.0.lost(
                        block,
                        Box::new(move |got| {
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
                        }),
                    );
                    Reply::from(Errno::EINPROGRESS as i32)
                }
                DEVICE_READY => {
                    self.0.ready(Box::new(move |got| {
                        let reply = match got {
                            Ok(v) => Reply::from(v as i32),
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
                DEVICE_SETTLED => {
                    self.0.settled(Box::new(move |got| {
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

/// The object over `imp`, for an opener to answer.
pub fn device(imp: Rc<dyn Device>) -> Rc<dyn Object> {
    Rc::new(DeviceObject(imp))
}

pub fn device_lost<L: xpute_core::link::Lend>(
    device: Handle,
    block: L,
) -> impl core::future::Future<Output = (L, Result<(), Errno>)> {
    let mut block = block;
    let (lent_at, lent_len) = xpute_core::link::Lend::span(&mut block);
    let sent = wait(
        device,
        DEVICE_LOST,
        Some((
            1,
            Box::new(move |w: &mut PacketWriter| {
                xpute_core::abi::link::Lent {
                    at: lent_at,
                    len: lent_len,
                }
                .write(w);
            }),
        )),
        block,
    );
    async move {
        let (r, data, lent) = sent.await;
        (lent, link::answer(r).map(|_| ()))
    }
}

pub fn device_lost_then<L: xpute_core::link::Lend>(
    device: Handle,
    block: L,
    then: impl FnOnce((L, Result<(), Errno>)) + 'static,
) {
    let mut block = block;
    let (lent_at, lent_len) = xpute_core::link::Lend::span(&mut block);
    let lent = block;
    link::send(
        device,
        DEVICE_LOST,
        Some((1, &|w: &mut PacketWriter| {
            xpute_core::abi::link::Lent {
                at: lent_at,
                len: lent_len,
            }
            .write(w);
        })),
        Some(Box::new(move |r, data| {
            then((lent, link::answer(r).map(|_| ())))
        })),
    );
}

pub fn device_ready(device: Handle) -> impl core::future::Future<Output = Result<u32, Errno>> {
    let sent = wait(device, DEVICE_READY, None, ());
    async move {
        let (r, data, lent) = sent.await;
        link::answer(r).map(|v| v as u32)
    }
}

pub fn device_ready_then(device: Handle, then: impl FnOnce(Result<u32, Errno>) + 'static) {
    let lent = ();
    link::send(
        device,
        DEVICE_READY,
        None,
        Some(Box::new(move |r, data| {
            then(link::answer(r).map(|v| v as u32))
        })),
    );
}

pub fn device_settled(device: Handle) -> impl core::future::Future<Output = Result<(), Errno>> {
    let sent = wait(device, DEVICE_SETTLED, None, ());
    async move {
        let (r, data, lent) = sent.await;
        link::answer(r).map(|_| ())
    }
}

pub fn device_settled_then(device: Handle, then: impl FnOnce(Result<(), Errno>) + 'static) {
    let lent = ();
    link::send(
        device,
        DEVICE_SETTLED,
        None,
        Some(Box::new(move |r, data| then(link::answer(r).map(|_| ())))),
    );
}
