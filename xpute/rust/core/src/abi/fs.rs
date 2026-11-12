// xpute-core/abi/fs.rs
//
// GENERATED from wit (xpute-gen spec/wit.rs)

#![allow(
    unused_imports,
    unused_variables,
    dead_code,
    clippy::all,
    reason = "generated from WIT"
)]

use crate::link::{self, Done, Handle, Method, Object, Reply, Tag};
use crate::wait::wait;
use std::cell::Cell;
use std::rc::Rc;
use xpute_kit::status::bug::OrBug;
use xpute_kit::status::errno::Errno;
use xpute_kit::status::error::MarshalError;
use xpute_kit::wire::xtp::{BranchCursor, Cursor, PacketWriter, TreeReader};

/// `[method]file.size`
pub const FILE_SIZE: Method = 0;
/// `[method]file.read`
pub const FILE_READ: Method = 1;
/// `[method]file.write`
pub const FILE_WRITE: Method = 2;
/// `[method]file.changed`
pub const FILE_CHANGED: Method = 3;
/// `[method]file.close`
pub const FILE_CLOSE: Method = 4;

pub trait File {
    fn size(&self, done: Done<u32>);
    fn read(&self, buf: super::link::Lent, done: Done<u32>);
    fn write(&self, buf: super::link::Lent, done: Done<()>);
    fn changed(&self, done: Done<()>);
    fn close(&self, done: Done<()>);
}

struct FileObject(Rc<dyn File>);

impl Object for FileObject {
    fn call(&self, m: Method, tag: Tag, args: Option<BranchCursor<'_>>) -> Reply {
        let run = || -> Result<Reply, MarshalError> {
            Ok(match m {
                FILE_SIZE => {
                    self.0.size(Box::new(move |got| {
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
                FILE_READ => {
                    let b = args.ok_or(MarshalError::new(Errno::EBADMSG))?;
                    let buf = b.at(0).and_then(|c| super::link::Lent::read(c))?;
                    self.0.read(
                        buf,
                        Box::new(move |got| {
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
                        }),
                    );
                    Reply::from(Errno::EINPROGRESS as i32)
                }
                FILE_WRITE => {
                    let b = args.ok_or(MarshalError::new(Errno::EBADMSG))?;
                    let buf = b.at(0).and_then(|c| super::link::Lent::read(c))?;
                    self.0.write(
                        buf,
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
                FILE_CHANGED => {
                    self.0.changed(Box::new(move |got| {
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
                FILE_CLOSE => {
                    self.0.close(Box::new(move |got| {
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
pub fn file(imp: Rc<dyn File>) -> Rc<dyn Object> {
    Rc::new(FileObject(imp))
}

pub fn file_size(file: Handle) -> impl core::future::Future<Output = Result<u32, Errno>> {
    let sent = wait(file, FILE_SIZE, None, ());
    async move {
        let (r, data, lent) = sent.await;
        link::answer(r).map(|v| v as u32)
    }
}

pub fn file_size_then(file: Handle, then: impl FnOnce(Result<u32, Errno>) + 'static) {
    let lent = ();
    link::send(
        file,
        FILE_SIZE,
        None,
        Some(Box::new(move |r, data| {
            then(link::answer(r).map(|v| v as u32))
        })),
    );
}

pub fn file_read<L: crate::link::Lend>(
    file: Handle,
    buf: L,
) -> impl core::future::Future<Output = (L, Result<u32, Errno>)> {
    let mut buf = buf;
    let (lent_at, lent_len) = crate::link::Lend::span(&mut buf);
    let sent = wait(
        file,
        FILE_READ,
        Some((
            1,
            Box::new(move |w: &mut PacketWriter| {
                super::link::Lent {
                    at: lent_at,
                    len: lent_len,
                }
                .write(w);
            }),
        )),
        buf,
    );
    async move {
        let (r, data, lent) = sent.await;
        (lent, link::answer(r).map(|v| v as u32))
    }
}

pub fn file_read_then<L: crate::link::Lend>(
    file: Handle,
    buf: L,
    then: impl FnOnce((L, Result<u32, Errno>)) + 'static,
) {
    let mut buf = buf;
    let (lent_at, lent_len) = crate::link::Lend::span(&mut buf);
    let lent = buf;
    link::send(
        file,
        FILE_READ,
        Some((1, &|w: &mut PacketWriter| {
            super::link::Lent {
                at: lent_at,
                len: lent_len,
            }
            .write(w);
        })),
        Some(Box::new(move |r, data| {
            then((lent, link::answer(r).map(|v| v as u32)))
        })),
    );
}

pub fn file_write<L: crate::link::Lend>(
    file: Handle,
    buf: L,
) -> impl core::future::Future<Output = (L, Result<(), Errno>)> {
    let mut buf = buf;
    let (lent_at, lent_len) = crate::link::Lend::span(&mut buf);
    let sent = wait(
        file,
        FILE_WRITE,
        Some((
            1,
            Box::new(move |w: &mut PacketWriter| {
                super::link::Lent {
                    at: lent_at,
                    len: lent_len,
                }
                .write(w);
            }),
        )),
        buf,
    );
    async move {
        let (r, data, lent) = sent.await;
        (lent, link::answer(r).map(|_| ()))
    }
}

pub fn file_write_then<L: crate::link::Lend>(
    file: Handle,
    buf: L,
    then: impl FnOnce((L, Result<(), Errno>)) + 'static,
) {
    let mut buf = buf;
    let (lent_at, lent_len) = crate::link::Lend::span(&mut buf);
    let lent = buf;
    link::send(
        file,
        FILE_WRITE,
        Some((1, &|w: &mut PacketWriter| {
            super::link::Lent {
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

pub fn file_changed(file: Handle) -> impl core::future::Future<Output = Result<(), Errno>> {
    let sent = wait(file, FILE_CHANGED, None, ());
    async move {
        let (r, data, lent) = sent.await;
        link::answer(r).map(|_| ())
    }
}

pub fn file_changed_then(file: Handle, then: impl FnOnce(Result<(), Errno>) + 'static) {
    let lent = ();
    link::send(
        file,
        FILE_CHANGED,
        None,
        Some(Box::new(move |r, data| then(link::answer(r).map(|_| ())))),
    );
}

pub fn file_close(file: Handle) -> impl core::future::Future<Output = Result<(), Errno>> {
    let sent = wait(file, FILE_CLOSE, None, ());
    async move {
        let (r, data, lent) = sent.await;
        link::answer(r).map(|_| ())
    }
}

pub fn file_close_then(file: Handle, then: impl FnOnce(Result<(), Errno>) + 'static) {
    let lent = ();
    link::send(
        file,
        FILE_CLOSE,
        None,
        Some(Box::new(move |r, data| then(link::answer(r).map(|_| ())))),
    );
}
