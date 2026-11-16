// xpute-host/abi/fs/file.rs
//
// GENERATED from wit/fs (xpute-link-gen wit.rs)

/// The interface's handle, named by `xpute:fs/file` (frame.json5 `NAMED`).
pub const HANDLE: _rt::Handle = 0x5b6743;

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

struct _FileObject(&'static _rt::Side, _rt::Rc<dyn HostFile>);

impl _rt::Object for _FileObject {
    fn call(&self, m_: _rt::Method, tag_: _rt::Tag, args_: _rt::Lift, _: &mut _rt::Lower) -> i32 {
        let run = || -> Result<i32, _rt::Errno> {
            Ok(match m_ {
                FILE_SIZE => {
                    let side_ = self.0;
                    self.1.size(_rt::Return::new(
                        Box::new(move |stop| side_.on_cancel(tag_, stop)),
                        Box::new(move |got| match got {
                            Ok(v) => {
                                side_.task_return(
                                    tag_,
                                    0,
                                    _rt::Packet::Args(Some((8, &|w: &mut _rt::Lower| match &v {
                                        Ok(v) => {
                                            w.u32(4, *v);
                                        }
                                        Err(v) => {
                                            w.u8(0, 1);
                                            w.u8(4, *v as u8);
                                        }
                                    }))),
                                );
                            }
                            Err(e) => {
                                side_.task_return(tag_, e as i32, _rt::Packet::Args(None));
                            }
                        }),
                    ));
                    _rt::Errno::EINPROGRESS as i32
                }
                FILE_READ => {
                    let buf = _rt::Lent::load(args_, 0)?;
                    let side_ = self.0;
                    self.1.read(
                        buf,
                        _rt::Return::new(
                            Box::new(move |stop| side_.on_cancel(tag_, stop)),
                            Box::new(move |got| match got {
                                Ok(v) => {
                                    side_.task_return(
                                        tag_,
                                        0,
                                        _rt::Packet::Args(Some((
                                            8,
                                            &|w: &mut _rt::Lower| match &v {
                                                Ok(v) => {
                                                    w.u32(4, *v);
                                                }
                                                Err(v) => {
                                                    w.u8(0, 1);
                                                    w.u8(4, *v as u8);
                                                }
                                            },
                                        ))),
                                    );
                                }
                                Err(e) => {
                                    side_.task_return(tag_, e as i32, _rt::Packet::Args(None));
                                }
                            }),
                        ),
                    );
                    _rt::Errno::EINPROGRESS as i32
                }
                FILE_WRITE => {
                    let buf = _rt::Lent::load(args_, 0)?;
                    let side_ = self.0;
                    self.1.write(
                        buf,
                        _rt::Return::new(
                            Box::new(move |stop| side_.on_cancel(tag_, stop)),
                            Box::new(move |got| match got {
                                Ok(()) => {
                                    side_.task_return(tag_, 0, _rt::Packet::Args(None));
                                }
                                Err(e) => {
                                    side_.task_return(tag_, e as i32, _rt::Packet::Args(None));
                                }
                            }),
                        ),
                    );
                    _rt::Errno::EINPROGRESS as i32
                }
                FILE_SET_ASIDE => {
                    let side_ = self.0;
                    self.1.set_aside(_rt::Return::new(
                        Box::new(move |stop| side_.on_cancel(tag_, stop)),
                        Box::new(move |got| match got {
                            Ok(v) => {
                                side_.task_return(
                                    tag_,
                                    0,
                                    _rt::Packet::Args(Some((2, &|w: &mut _rt::Lower| match &v {
                                        Ok(_) => {}
                                        Err(v) => {
                                            w.u8(0, 1);
                                            w.u8(1, *v as u8);
                                        }
                                    }))),
                                );
                            }
                            Err(e) => {
                                side_.task_return(tag_, e as i32, _rt::Packet::Args(None));
                            }
                        }),
                    ));
                    _rt::Errno::EINPROGRESS as i32
                }
                FILE_CLOSE => {
                    let side_ = self.0;
                    self.1.close(_rt::Return::new(
                        Box::new(move |stop| side_.on_cancel(tag_, stop)),
                        Box::new(move |got| match got {
                            Ok(v) => {
                                side_.task_return(
                                    tag_,
                                    0,
                                    _rt::Packet::Args(Some((2, &|w: &mut _rt::Lower| match &v {
                                        Ok(_) => {}
                                        Err(v) => {
                                            w.u8(0, 1);
                                            w.u8(1, *v as u8);
                                        }
                                    }))),
                                );
                            }
                            Err(e) => {
                                side_.task_return(tag_, e as i32, _rt::Packet::Args(None));
                            }
                        }),
                    ));
                    _rt::Errno::EINPROGRESS as i32
                }
                _ => _rt::Errno::ENOSYS as i32,
            })
        };
        run().unwrap_or_else(|e| e as i32)
    }
}

/// The object over `implementation` on `side_`, for an opener or a function to answer.
pub fn file(
    side_: &'static _rt::Side,
    implementation: _rt::Rc<dyn HostFile>,
) -> _rt::Rc<dyn _rt::Object> {
    _rt::Rc::new(_FileObject(side_, implementation))
}

mod _rt {
    pub(super) use std::rc::Rc;
    pub(super) use xpute_kit::status::errno::Errno;
    pub(super) use xpute_link::abi::types::Lent;
    pub(super) use xpute_link::{Handle, Lift, Lower, Method, Object, Packet, Return, Side, Tag};
}
