// xpute-guest/abi/clocks/monotonic_clock.rs
//
// GENERATED from wit/clocks (xpute-link-gen wit.rs)

/// The interface's handle, named by `xpute:clocks/monotonic-clock` (frame.json5 `NAMED`).
pub const HANDLE: _rt::Handle = 0x73f703;

/// `now`
pub const NOW: _rt::Method = 0;
/// `wait-for`
pub const WAIT_FOR: _rt::Method = 1;

pub type Mark = u64;

pub fn now() -> Result<u64, _rt::Errno> {
    let (r, data) = _rt::link::call(HANDLE, NOW, None);
    _rt::answer(r).and_then(|_| {
        let r_ = _rt::Lift::new(data.ok_or(_rt::Errno::EBADMSG)?);
        r_.u64(0)
    })
}

pub fn wait_for(how_long: u64) -> impl Future<Output = Result<(), _rt::Errno>> + use<> {
    let sent = _rt::wait::wait(
        HANDLE,
        WAIT_FOR,
        Some((8, &|w_: &mut _rt::Lower| {
            w_.u64(0, how_long);
        })),
        (),
    );
    async move {
        let (r, _, _) = sent.await;
        _rt::answer(r)
    }
}

mod _rt {
    pub(super) use crate::{link, wait};
    pub(super) use xpute_kit::status::errno::Errno;
    pub(super) use xpute_link::{Handle, Lift, Lower, Method, answer};
}
