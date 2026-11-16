// pxl-gpu-host/abi/gpu.rs
//
// GENERATED from spec/gpu/wit (xpute-link-gen wit.rs)

/// The interface's handle, named by `pxl:gpu/gpu` (frame.json5 `NAMED`).
pub const HANDLE: _rt::Handle = 0x54bd98;

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
    fn run(&self, recs: _rt::Lent, task_return: _rt::Return<()>);
    fn get_device(&self) -> Result<u32, _rt::Errno>;
}

struct _Object(&'static _rt::Side, _rt::Rc<dyn Host>);

impl _rt::Object for _Object {
    fn call(
        &self,
        m_: _rt::Method,
        tag_: _rt::Tag,
        args_: _rt::Lift,
        ret_: &mut _rt::Lower,
    ) -> i32 {
        let mut run = || -> Result<i32, _rt::Errno> {
            Ok(match m_ {
                RUN => {
                    let recs = _rt::Lent::load(args_, 0)?;
                    let side_ = self.0;
                    self.1.run(
                        recs,
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
                GET_DEVICE => match self.1.get_device() {
                    Ok(v) => {
                        ret_.start(4);
                        ret_.u32(0, v);
                        0
                    }
                    Err(e) => e as i32,
                },
                _ => _rt::Errno::ENOSYS as i32,
            })
        };
        run().unwrap_or_else(|e| e as i32)
    }
}

/// Mounts `implementation` under the interface's handle on `side_`.
pub fn mount(side_: &'static _rt::Side, implementation: _rt::Rc<dyn Host>) {
    side_.mount_named(HANDLE, _rt::Rc::new(_Object(side_, implementation)));
}

pub trait HostDevice {
    fn lost(&self, block: _rt::Lent, task_return: _rt::Return<()>);
    fn ready(&self, task_return: _rt::Return<Result<(), ErrorCode>>);
    fn settled(&self, task_return: _rt::Return<()>);
}

struct _DeviceObject(&'static _rt::Side, _rt::Rc<dyn HostDevice>);

impl _rt::Object for _DeviceObject {
    fn call(&self, m_: _rt::Method, tag_: _rt::Tag, args_: _rt::Lift, _: &mut _rt::Lower) -> i32 {
        let run = || -> Result<i32, _rt::Errno> {
            Ok(match m_ {
                DEVICE_LOST => {
                    let block = _rt::Lent::load(args_, 0)?;
                    let side_ = self.0;
                    self.1.lost(
                        block,
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
                DEVICE_READY => {
                    let side_ = self.0;
                    self.1.ready(_rt::Return::new(
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
                DEVICE_SETTLED => {
                    let side_ = self.0;
                    self.1.settled(_rt::Return::new(
                        Box::new(move |stop| side_.on_cancel(tag_, stop)),
                        Box::new(move |got| match got {
                            Ok(()) => {
                                side_.task_return(tag_, 0, _rt::Packet::Args(None));
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
pub fn device(
    side_: &'static _rt::Side,
    implementation: _rt::Rc<dyn HostDevice>,
) -> _rt::Rc<dyn _rt::Object> {
    _rt::Rc::new(_DeviceObject(side_, implementation))
}

mod _rt {
    pub(super) use std::rc::Rc;
    pub(super) use xpute_kit::status::errno::Errno;
    pub(super) use xpute_link::abi::types::Lent;
    pub(super) use xpute_link::{Handle, Lift, Lower, Method, Object, Packet, Return, Side, Tag};
}
