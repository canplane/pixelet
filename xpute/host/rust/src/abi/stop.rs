// xpute-host/abi/stop.rs
//
// GENERATED from wit/stop (xpute-link-gen wit.rs)

/// The interface's handle, named by `xpute:stop/stop` (frame.json5 `NAMED`).
pub const HANDLE: _rt::Handle = 0x4eccaa;

/// `stopped`
pub const STOPPED: _rt::Method = 0;

pub trait Host {
    fn stopped(
        &self,
        errnum: i32,
        file: String,
        line: u32,
        n1: u64,
        n2: u64,
        msg: Option<String>,
    ) -> Result<(), _rt::Errno>;
}

struct _Object(_rt::Rc<dyn Host>);

impl _rt::Object for _Object {
    fn call(&self, m_: _rt::Method, _: _rt::Tag, args_: _rt::Lift, _: &mut _rt::Lower) -> i32 {
        let run = || -> Result<i32, _rt::Errno> {
            Ok(match m_ {
                STOPPED => {
                    let errnum = args_.i32(0)?;
                    let file = args_.str(4).map(str::to_owned)?;
                    let line = args_.u32(12)?;
                    let n1 = args_.u64(16)?;
                    let n2 = args_.u64(24)?;
                    let msg = args_.case(32, 2).and_then(|d| {
                        if d == 0 {
                            Ok(None)
                        } else {
                            args_.str(36).map(str::to_owned).map(Some)
                        }
                    })?;
                    match self.0.stopped(errnum, file, line, n1, n2, msg) {
                        Ok(()) => 0,
                        Err(e) => e as i32,
                    }
                }
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

/// The interfaces the world imports, each `(name, handle)`, which are
/// mounted before the guest boots: one with functions of its own under its
/// handle, one of resources alone by an opener at a prefix.
pub const IMPORTS: &[(&str, xpute_link::Handle)] = &[("xpute:stop/stop", 0x4eccaa)];

/// The methods of the interfaces the world imports that answer at once,
/// each `(handle, method)`, a function without WIT's `async`, which a
/// runtime refuses to carry to a UI on another thread.
pub const CALLS: &[(xpute_link::Handle, xpute_link::Method)] = &[(0x4eccaa, 0)];
