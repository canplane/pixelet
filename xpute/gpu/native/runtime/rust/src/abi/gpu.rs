// xpute-gpu-native-runtime/abi/gpu.rs
//
// GENERATED from gpu/wit (xpute-link-gen wit.rs)

/// The interface's handle, named by `xpute:gpu/gpu` (frame.json5 `NAMED`).
pub const HANDLE: _rt::Handle = 0x79d455;

/// `run`
pub const RUN: _rt::Method = 0;
/// `get-device`
pub const GET_DEVICE: _rt::Method = 1;
/// `[method]device.lost`
pub const DEVICE_LOST: _rt::Method = 0;
/// `[method]device.ready`
pub const DEVICE_READY: _rt::Method = 1;
/// `[method]device.settled`
pub const DEVICE_SETTLED: _rt::Method = 2;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[repr(u32)]
pub enum ErrorCode {
    /// The host has no device to give.
    NoDevice = 0,
}

impl ErrorCode {
    /// Every member, in its source's own order.
    pub const ALL: [ErrorCode; 1] = [ErrorCode::NoDevice];

    /// The member `val` is, or none for a number no member has.
    pub const fn of(val: u32) -> Option<ErrorCode> {
        match val {
            0 => Some(ErrorCode::NoDevice),
            _ => None,
        }
    }

    /// The member's name, as its source spells it.
    pub const fn name(self) -> &'static str {
        match self {
            ErrorCode::NoDevice => "NoDevice",
        }
    }
}

pub trait Host {
    fn run(&self, records: _rt::Lent, task_return: _rt::Return<()>);
    fn get_device(&self) -> Result<u32, _rt::Errno>;
}

struct _Object(_rt::Rc<dyn Host>);

impl _rt::Object for _Object {
    fn call(&self, m_: _rt::Method, tag_: _rt::Tag, args_: Option<&[u8]>) -> _rt::Reply {
        let run = || -> Result<_rt::Reply, _rt::Errno> {
            Ok(match m_ {
                RUN => {
                    let r_ = _rt::Lift::new(args_.ok_or(_rt::Errno::EBADMSG)?);
                    let records = _rt::Lent::load(r_, 0)?;
                    self.0.run(
                        records,
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
                GET_DEVICE => match self.0.get_device() {
                    Ok(v) => _rt::Reply {
                        result: 0,
                        data: Some((
                            4,
                            Box::new(move |w: &mut _rt::Lower| {
                                w.u32(0, v);
                            }),
                        )),
                    },
                    Err(e) => _rt::Reply::from(e as i32),
                },
                _ => _rt::Reply::from(_rt::Errno::ENOSYS as i32),
            })
        };
        run().unwrap_or_else(|e| _rt::Reply::from(e as i32))
    }
}

/// Serves `implementation` under the interface's handle.
pub fn mount(implementation: _rt::Rc<dyn Host>) {
    _rt::link::mount_named(HANDLE, _rt::Rc::new(_Object(implementation)));
}

pub trait HostDevice {
    fn lost(&self, block: _rt::Lent, task_return: _rt::Return<()>);
    fn ready(&self, task_return: _rt::Return<Result<(), ErrorCode>>);
    fn settled(&self, task_return: _rt::Return<()>);
}

struct _DeviceObject(_rt::Rc<dyn HostDevice>);

impl _rt::Object for _DeviceObject {
    fn call(&self, m_: _rt::Method, tag_: _rt::Tag, args_: Option<&[u8]>) -> _rt::Reply {
        let run = || -> Result<_rt::Reply, _rt::Errno> {
            Ok(match m_ {
                DEVICE_LOST => {
                    let r_ = _rt::Lift::new(args_.ok_or(_rt::Errno::EBADMSG)?);
                    let block = _rt::Lent::load(r_, 0)?;
                    self.0.lost(
                        block,
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
                DEVICE_READY => {
                    self.0.ready(_rt::Return::new(
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
                DEVICE_SETTLED => {
                    self.0.settled(_rt::Return::new(
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
pub fn device(implementation: _rt::Rc<dyn HostDevice>) -> _rt::Rc<dyn _rt::Object> {
    _rt::Rc::new(_DeviceObject(implementation))
}

mod _rt {
    pub(super) use std::rc::Rc;
    pub(super) use xpute_kit::status::errno::Errno;
    pub(super) use xpute_link::abi::types::Lent;
    pub(super) use xpute_link::{Handle, Lift, Lower, Method, Object, Reply, Return, Tag};
    pub(super) use xpute_native_runtime::link;
}
