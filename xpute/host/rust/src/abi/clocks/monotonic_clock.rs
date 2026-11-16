// xpute-host/abi/clocks/monotonic_clock.rs
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
                NOW => match self.1.now() {
                    Ok(v) => {
                        ret_.start(8);
                        ret_.u64(0, v);
                        0
                    }
                    Err(e) => e as i32,
                },
                WAIT_FOR => {
                    let how_long = args_.u64(0)?;
                    let side_ = self.0;
                    self.1.wait_for(
                        how_long,
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

mod _rt {
    pub(super) use std::rc::Rc;
    pub(super) use xpute_kit::status::errno::Errno;
    pub(super) use xpute_link::{Handle, Lift, Lower, Method, Object, Packet, Return, Side, Tag};
}
