// xpute-native-host/abi/fs/file.rs
//
// GENERATED from wit/fs (xpute-link-gen wit.rs)

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
use xpute_link::abi::types::Lent;
use xpute_link::{Handle, Method, Object, Reply, Return, Tag};

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
    fn size(&self, task_return: Return<u32>);
    fn read(&self, buf: Lent, task_return: Return<u32>);
    fn write(&self, buf: Lent, task_return: Return<()>);
    fn changed(&self, task_return: Return<()>);
    fn close(&self, task_return: Return<()>);
}

struct FileObject(Rc<dyn File>);

impl Object for FileObject {
    fn call(&self, m_: Method, tag_: Tag, args_: Option<BranchCursor<'_>>) -> Reply {
        let run = || -> Result<Reply, MarshalError> {
            Ok(match m_ {
                FILE_SIZE => {
                    self.0.size(Return::new(
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
                FILE_READ => {
                    let b_ = args_.ok_or(MarshalError::new(Errno::EBADMSG))?;
                    let buf = b_.at(0).and_then(|c| Lent::read(c))?;
                    self.0.read(
                        buf,
                        Return::new(
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
                        ),
                    );
                    Reply::from(Errno::EINPROGRESS as i32)
                }
                FILE_WRITE => {
                    let b_ = args_.ok_or(MarshalError::new(Errno::EBADMSG))?;
                    let buf = b_.at(0).and_then(|c| Lent::read(c))?;
                    self.0.write(
                        buf,
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
                FILE_CHANGED => {
                    self.0.changed(Return::new(
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
                FILE_CLOSE => {
                    self.0.close(Return::new(
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
pub fn file(imp: Rc<dyn File>) -> Rc<dyn Object> {
    Rc::new(FileObject(imp))
}
