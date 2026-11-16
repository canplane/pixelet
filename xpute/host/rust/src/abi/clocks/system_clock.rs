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

    pub fn load(r: _rt::Lift<'_>, at: usize) -> Result<Instant, _rt::Errno> {
        Ok(Instant {
            seconds: r.i64(at)?,
            nanoseconds: r.u32(at + 8)?,
        })
    }
}

pub trait Host {
    fn now(&self) -> Result<Instant, _rt::Errno>;
}

struct _Object(_rt::Rc<dyn Host>);

impl _rt::Object for _Object {
    fn call(&self, m_: _rt::Method, _: _rt::Tag, _: _rt::Lift, ret_: &mut _rt::Lower) -> i32 {
        let mut run = || -> Result<i32, _rt::Errno> {
            Ok(match m_ {
                NOW => match self.0.now() {
                    Ok(v) => {
                        ret_.start(Instant::SZ);
                        v.store(ret_, 0);
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
    side_.mount_named(HANDLE, _rt::Rc::new(_Object(implementation)));
}

mod _rt {
    pub(super) use std::rc::Rc;
    pub(super) use xpute_kit::status::errno::Errno;
    pub(super) use xpute_link::{Handle, Lift, Lower, Method, Object, Side, Tag};
}
