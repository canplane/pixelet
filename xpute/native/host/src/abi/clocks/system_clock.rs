// xpute-native-host/abi/clocks/system_clock.rs
//
// GENERATED from wit/clocks (xpute-link-gen wit.rs)

#![allow(
    unused_imports,
    unused_variables,
    dead_code,
    clippy::all,
    reason = "generated from WIT"
)]

/// The interface's handle, named by `xpute:clocks/system-clock` (frame.json `NAMED`).
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
        w.i64(at + 0, *&self.seconds);
        w.u32(at + 8, *&self.nanoseconds);
    }

    pub fn load(r: _rt::Lift<'_>, at: usize) -> Result<Instant, _rt::Errno> {
        Ok(Instant {
            seconds: r.i64(at + 0)?,
            nanoseconds: r.u32(at + 8)?,
        })
    }
}

pub trait Host {
    fn now(&self) -> Result<Instant, _rt::Errno>;
}

struct _Object(_rt::Rc<dyn Host>);

impl _rt::Object for _Object {
    fn call(&self, m_: _rt::Method, tag_: _rt::Tag, args_: Option<&[u8]>) -> _rt::Reply {
        let run = || -> Result<_rt::Reply, _rt::Errno> {
            Ok(match m_ {
                NOW => match self.0.now() {
                    Ok(v) => _rt::Reply {
                        result: 0,
                        data: Some((
                            16,
                            Box::new(move |w: &mut _rt::Lower| {
                                (&v).store(w, 0);
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

/// Serves `imp` under the interface's handle.
pub fn mount(imp: _rt::Rc<dyn Host>) {
    _rt::link::mount_named(HANDLE, _rt::Rc::new(_Object(imp)));
}

mod _rt {
    pub use crate::link;
    pub use std::rc::Rc;
    pub use xpute_kit::status::errno::Errno;
    pub use xpute_link::{Handle, Lift, Lower, Method, Object, Reply, Tag};
}
