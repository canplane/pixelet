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
    fn call(&self, m_: _rt::Method, _: _rt::Tag, args_: _rt::Lift, ret_: &mut _rt::Lower) -> i32 {
        let mut run = || -> Result<i32, _rt::Errno> {
            Ok(match m_ {
                SET => {
                    let key = args_.u32(0)?;
                    let val = args_.u32(4)?;
                    match self.0.set(key, val) {
                        Ok(v) => {
                            ret_.start(2);
                            match &v {
                                Ok(_) => {}
                                Err(v) => {
                                    ret_.u8(0, 1);
                                    ret_.u8(1, *v as u8);
                                }
                            }
                            0
                        }
                        Err(e) => e as i32,
                    }
                }
                _ => _rt::Errno::ENOSYS as i32,
            })
        };
        run().unwrap_or_else(|e| e as i32)
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
    pub(super) use xpute_link::{Handle, Lift, Lower, Method, Object, Tag};
}
