// xpute-guest/abi/clocks/system_clock.rs
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

pub fn now() -> Result<Instant, _rt::Errno> {
    let (r, data) = _rt::link::call(HANDLE, NOW, None);
    _rt::answer(r).and_then(|_| {
        let r_ = _rt::Lift::new(data.ok_or(_rt::Errno::EBADMSG)?);
        Instant::load(r_, 0)
    })
}

mod _rt {
    pub(super) use crate::link;
    pub(super) use xpute_kit::status::errno::Errno;
    pub(super) use xpute_link::{Handle, Lift, Lower, Method, answer};
}
