// pxl-core/abi/setting.rs
//
// GENERATED from spec/client/core/wit (xpute-link-gen wit.rs)

/// The interface's handle, named by `pxl:core/setting` (frame.json5 `NAMED`).
pub const HANDLE: _rt::Handle = 0x6f63cb;

/// `set`
pub const SET: _rt::Method = 0;

pub trait Guest {
    fn set(&self, key: u32, val: u32) -> Result<Result<(), super::types::ErrorCode>, _rt::Errno>;
}

struct _Object(_rt::Rc<dyn Guest>);

impl _rt::Object for _Object {
    fn call(&self, m_: _rt::Method, _: _rt::Tag, args_: Option<&[u8]>) -> _rt::Reply {
        let run = || -> Result<_rt::Reply, _rt::Errno> {
            Ok(match m_ {
                SET => {
                    let r_ = _rt::Lift::new(args_.ok_or(_rt::Errno::EBADMSG)?);
                    let key = r_.u32(0)?;
                    let val = r_.u32(4)?;
                    match self.0.set(key, val) {
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
                    }
                }
                _ => _rt::Reply::from(_rt::Errno::ENOSYS as i32),
            })
        };
        run().unwrap_or_else(|e| _rt::Reply::from(e as i32))
    }
}

/// Mounts `implementation` under the interface's handle.
pub fn mount(implementation: _rt::Rc<dyn Guest>) {
    _rt::link::mount_named(HANDLE, _rt::Rc::new(_Object(implementation)));
}

mod _rt {
    pub(super) use std::rc::Rc;
    pub(super) use xpute_guest::link;
    pub(super) use xpute_kit::status::errno::Errno;
    pub(super) use xpute_link::{Handle, Lift, Lower, Method, Object, Reply, Tag};
}
