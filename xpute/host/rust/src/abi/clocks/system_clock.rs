// xpute-host/abi/clocks/system_clock.rs
//
// GENERATED from wit/clocks (xpute-link-gen wit.rs)

/// The interface's handle, named by `xpute:clocks/system-clock` (frame.json5 `NAMED`).
pub const HANDLE: _rt::Handle = 0x7dfa61;

/// `now`
pub const NOW: _rt::Method = 0;

#[derive(Clone, Debug, PartialEq)]
pub struct Instant {
    pub seconds: i64,
    pub nanoseconds: u32,
}

impl Instant {
    pub const SZ: usize = 16;
    pub const ALIGN: usize = 8;

    pub fn store(&self, w: &mut _rt::Lower, at: usize) {
        w.i64(at, self.seconds);
        w.u32(at + 8, self.nanoseconds);
    }

    /// Stores a record of these fields, each borrowed, as `store` stores one.
    pub fn store_of(w_: &mut _rt::Lower, at_: usize, seconds: i64, nanoseconds: u32) {
        w_.i64(at_, seconds);
        w_.u32(at_ + 8, nanoseconds);
    }

    pub fn load(r: _rt::Lift<'_>, at: usize) -> Result<Instant, _rt::Errno> {
        Ok(Instant {
            seconds: r.i64(at)?,
            nanoseconds: r.u32(at + 8)?,
        })
    }
}

pub trait Host {
    fn now(&self) -> Result<Instant, _rt::XputeError>;
}

struct _Object(_rt::Rc<dyn Host>);

impl _rt::Object for _Object {
    fn call(&self, m_: _rt::Method, _: _rt::Tag, _: _rt::Lift, ret_: &mut _rt::Lower) -> i32 {
        let mut run = || -> Result<i32, _rt::XputeError> {
            Ok(match m_ {
                NOW => {
                    let v = self.0.now()?;
                    {
                        ret_.start(Instant::SZ);
                        v.store(ret_, 0);
                        0
                    }
                }
                _ => return Err(_rt::Errno::ENOSYS.into()),
            })
        };
        run().unwrap_or_else(|e| _rt::store_err(ret_, &e))
    }
}

/// Mounts `implementation` under the interface's handle on `side_`.
pub fn mount(side_: &'static _rt::Side, implementation: _rt::Rc<dyn Host>) {
    side_.mount_named(HANDLE, _rt::Rc::new(_Object(implementation)));
}

mod _rt {
    pub(super) use std::rc::Rc;
    pub(super) use xpute_kit::status::errno::Errno;
    pub(super) use xpute_kit::status::error::XputeError;
    pub(super) use xpute_link::{Handle, Lift, Lower, Method, Object, Side, Tag, store_err};
}
