// xpute-native-runtime/abi/clocks/monotonic_clock.rs
//
// GENERATED from wit/clocks (xpute-link-gen wit.rs)

/// The interface's handle, named by `xpute:clocks/monotonic-clock` (frame.json5 `NAMED`).
pub const HANDLE: _rt::Handle = 0x73f703;

/// `now`
pub const NOW: _rt::Method = 0;
/// `wait-for`
pub const WAIT_FOR: _rt::Method = 1;

pub type Mark = u64;

pub trait Host {
    fn now(&self) -> Result<u64, _rt::Errno>;
    fn wait_for(&self, how_long: u64, task_return: _rt::Return<()>);
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
                            8,
                            Box::new(move |w: &mut _rt::Lower| {
                                w.u64(0, v);
                            }),
                        )),
                    },
                    Err(e) => _rt::Reply::from(e as i32),
                },
                WAIT_FOR => {
                    let r_ = _rt::Lift::new(args_.ok_or(_rt::Errno::EBADMSG)?);
                    let how_long = r_.u64(0)?;
                    self.0.wait_for(
                        how_long,
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
    pub use crate::link;
    pub use std::rc::Rc;
    pub use xpute_kit::status::errno::Errno;
    pub use xpute_link::{Handle, Lift, Lower, Method, Object, Reply, Return, Tag};
}
