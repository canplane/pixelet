// xpute-native-runtime/abi/stop.rs
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
    ) -> Result<(), _rt::Errno>;
}

struct _Object(_rt::Rc<dyn Host>);

impl _rt::Object for _Object {
    fn call(&self, m_: _rt::Method, _: _rt::Tag, args_: Option<&[u8]>) -> _rt::Reply {
        let run = || -> Result<_rt::Reply, _rt::Errno> {
            Ok(match m_ {
                STOPPED => {
                    let r_ = _rt::Lift::new(args_.ok_or(_rt::Errno::EBADMSG)?);
                    let errnum = r_.i32(0)?;
                    let file = r_.str(4).map(str::to_owned)?;
                    let line = r_.u32(12)?;
                    let n1 = r_.u64(16)?;
                    let n2 = r_.u64(24)?;
                    match self.0.stopped(errnum, file, line, n1, n2) {
                        Ok(()) => _rt::Reply::from(0),
                        Err(e) => _rt::Reply::from(e as i32),
                    }
                }
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

mod _rt {
    pub(super) use crate::link;
    pub(super) use std::rc::Rc;
    pub(super) use xpute_kit::status::errno::Errno;
    pub(super) use xpute_link::{Handle, Lift, Method, Object, Reply, Tag};
}

/// The interfaces the world imports, each `(name, handle)`, which are
/// mounted before the guest boots: one with functions of its own under its
/// handle, one of resources alone by an opener at a prefix.
pub const IMPORTS: &[(&str, xpute_link::Handle)] = &[("xpute:stop/stop", 0x4eccaa)];

/// The methods of the named interfaces the host serves that answer
/// later, each `(handle, method)`, as WIT's `async func` marks them.
pub const LATER: &[(xpute_link::Handle, xpute_link::Method)] = &[];
