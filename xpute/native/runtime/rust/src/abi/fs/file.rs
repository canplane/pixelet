// xpute-native-runtime/abi/fs/file.rs
//
// GENERATED from wit/fs (xpute-link-gen wit.rs)

/// `[method]file.size`
pub const FILE_SIZE: _rt::Method = 0;
/// `[method]file.read`
pub const FILE_READ: _rt::Method = 1;
/// `[method]file.write`
pub const FILE_WRITE: _rt::Method = 2;
/// `[method]file.set-aside`
pub const FILE_SET_ASIDE: _rt::Method = 3;
/// `[method]file.close`
pub const FILE_CLOSE: _rt::Method = 4;

pub trait HostFile {
    fn size(&self, task_return: _rt::Return<Result<u32, super::types::ErrorCode>>);
    fn read(&self, buf: _rt::Lent, task_return: _rt::Return<Result<u32, super::types::ErrorCode>>);
    fn write(&self, buf: _rt::Lent, task_return: _rt::Return<()>);
    fn set_aside(&self, task_return: _rt::Return<Result<(), super::types::ErrorCode>>);
    fn close(&self, task_return: _rt::Return<Result<(), super::types::ErrorCode>>);
}

struct _FileObject(_rt::Rc<dyn HostFile>);

impl _rt::Object for _FileObject {
    fn call(&self, m_: _rt::Method, tag_: _rt::Tag, args_: Option<&[u8]>) -> _rt::Reply {
        let run = || -> Result<_rt::Reply, _rt::Errno> {
            Ok(match m_ {
                FILE_SIZE => {
                    self.0.size(_rt::Return::new(
                        tag_,
                        _rt::link::on_cancel,
                        Box::new(move |got| {
                            let reply = match got {
                                Ok(v) => _rt::Reply {
                                    result: 0,
                                    data: Some((
                                        8,
                                        Box::new(move |w: &mut _rt::Lower| match &v {
                                            Ok(v) => {
                                                w.u32(4, *v);
                                            }
                                            Err(v) => {
                                                w.u8(0, 1);
                                                w.u8(4, *v as u8);
                                            }
                                        }),
                                    )),
                                },
                                Err(e) => _rt::Reply::from(e as i32),
                            };
                            _rt::link::task_return(
                                tag_,
                                reply.result,
                                reply
                                    .data
                                    .as_ref()
                                    .map(|(n, w)| (*n, &**w as &dyn Fn(&mut _rt::Lower))),
                            );
                        }),
                    ));
                    _rt::Reply::from(_rt::Errno::EINPROGRESS as i32)
                }
                FILE_READ => {
                    let r_ = _rt::Lift::new(args_.ok_or(_rt::Errno::EBADMSG)?);
                    let buf = _rt::Lent::load(r_, 0)?;
                    self.0.read(
                        buf,
                        _rt::Return::new(
                            tag_,
                            _rt::link::on_cancel,
                            Box::new(move |got| {
                                let reply = match got {
                                    Ok(v) => _rt::Reply {
                                        result: 0,
                                        data: Some((
                                            8,
                                            Box::new(move |w: &mut _rt::Lower| match &v {
                                                Ok(v) => {
                                                    w.u32(4, *v);
                                                }
                                                Err(v) => {
                                                    w.u8(0, 1);
                                                    w.u8(4, *v as u8);
                                                }
                                            }),
                                        )),
                                    },
                                    Err(e) => _rt::Reply::from(e as i32),
                                };
                                _rt::link::task_return(
                                    tag_,
                                    reply.result,
                                    reply
                                        .data
                                        .as_ref()
                                        .map(|(n, w)| (*n, &**w as &dyn Fn(&mut _rt::Lower))),
                                );
                            }),
                        ),
                    );
                    _rt::Reply::from(_rt::Errno::EINPROGRESS as i32)
                }
                FILE_WRITE => {
                    let r_ = _rt::Lift::new(args_.ok_or(_rt::Errno::EBADMSG)?);
                    let buf = _rt::Lent::load(r_, 0)?;
                    self.0.write(
                        buf,
                        _rt::Return::new(
                            tag_,
                            _rt::link::on_cancel,
                            Box::new(move |got| {
                                let reply = match got {
                                    Ok(()) => _rt::Reply::from(0),
                                    Err(e) => _rt::Reply::from(e as i32),
                                };
                                _rt::link::task_return(
                                    tag_,
                                    reply.result,
                                    reply
                                        .data
                                        .as_ref()
                                        .map(|(n, w)| (*n, &**w as &dyn Fn(&mut _rt::Lower))),
                                );
                            }),
                        ),
                    );
                    _rt::Reply::from(_rt::Errno::EINPROGRESS as i32)
                }
                FILE_SET_ASIDE => {
                    self.0.set_aside(_rt::Return::new(
                        tag_,
                        _rt::link::on_cancel,
                        Box::new(move |got| {
                            let reply = match got {
                                Ok(v) => _rt::Reply {
                                    result: 0,
                                    data: Some((
                                        2,
                                        Box::new(move |w: &mut _rt::Lower| match &v {
                                            Ok(_) => {}
                                            Err(v) => {
                                                w.u8(0, 1);
                                                w.u8(1, *v as u8);
                                            }
                                        }),
                                    )),
                                },
                                Err(e) => _rt::Reply::from(e as i32),
                            };
                            _rt::link::task_return(
                                tag_,
                                reply.result,
                                reply
                                    .data
                                    .as_ref()
                                    .map(|(n, w)| (*n, &**w as &dyn Fn(&mut _rt::Lower))),
                            );
                        }),
                    ));
                    _rt::Reply::from(_rt::Errno::EINPROGRESS as i32)
                }
                FILE_CLOSE => {
                    self.0.close(_rt::Return::new(
                        tag_,
                        _rt::link::on_cancel,
                        Box::new(move |got| {
                            let reply = match got {
                                Ok(v) => _rt::Reply {
                                    result: 0,
                                    data: Some((
                                        2,
                                        Box::new(move |w: &mut _rt::Lower| match &v {
                                            Ok(_) => {}
                                            Err(v) => {
                                                w.u8(0, 1);
                                                w.u8(1, *v as u8);
                                            }
                                        }),
                                    )),
                                },
                                Err(e) => _rt::Reply::from(e as i32),
                            };
                            _rt::link::task_return(
                                tag_,
                                reply.result,
                                reply
                                    .data
                                    .as_ref()
                                    .map(|(n, w)| (*n, &**w as &dyn Fn(&mut _rt::Lower))),
                            );
                        }),
                    ));
                    _rt::Reply::from(_rt::Errno::EINPROGRESS as i32)
                }
                _ => _rt::Reply::from(_rt::Errno::ENOSYS as i32),
            })
        };
        run().unwrap_or_else(|e| _rt::Reply::from(e as i32))
    }
}

/// The object over `implementation`, for an opener or a function to answer.
pub fn file(implementation: _rt::Rc<dyn HostFile>) -> _rt::Rc<dyn _rt::Object> {
    _rt::Rc::new(_FileObject(implementation))
}

mod _rt {
    pub use crate::link;
    pub use std::rc::Rc;
    pub use xpute_kit::status::errno::Errno;
    pub use xpute_link::abi::types::Lent;
    pub use xpute_link::{Lift, Lower, Method, Object, Reply, Return, Tag};
}
