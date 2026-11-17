// pxl-gpu-guest/abi/gpu.rs
//
// GENERATED from spec/gpu/wit (xpute-link-gen wit.rs)

/// The interface's handle, named by `pxl:gpu/gpu` (frame.json5 `NAMED`).
pub const HANDLE: _rt::Handle = 0x54bd98;

/// `run`
pub const RUN: _rt::Method = 0;
/// `get-device`
pub const GET_DEVICE: _rt::Method = 1;
/// `[method]device.lost`
pub const DEVICE_LOST: _rt::Method = 0;
/// `[method]device.ready`
pub const DEVICE_READY: _rt::Method = 1;
/// `[method]device.settled`
pub const DEVICE_SETTLED: _rt::Method = 2;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[repr(u32)]
pub enum ErrorCode {
    /// The host has no device to give.
    NoDevice = 0,
}

impl ErrorCode {
    /// Every member, in its source's own order.
    pub const ALL: [ErrorCode; 1] = [ErrorCode::NoDevice];

    /// The member `val` is, or none for a number no member has.
    pub const fn of(val: u32) -> Option<ErrorCode> {
        match val {
            0 => Some(ErrorCode::NoDevice),
            _ => None,
        }
    }

    /// The member's name, as its source spells it.
    pub const fn name(self) -> &'static str {
        match self {
            ErrorCode::NoDevice => "NoDevice",
        }
    }
}

pub fn run<L: _rt::Lend>(recs: L) -> impl Future<Output = (L, Result<(), _rt::Errno>)> + use<L> {
    let mut recs = recs;
    let recs_lent_ = _rt::Lend::lent(&mut recs);
    let sent = _rt::wait::wait(
        HANDLE,
        RUN,
        Some((16, &|w_: &mut _rt::Lower| {
            recs_lent_.store(w_, 0);
        })),
        recs,
    );
    async move {
        let (r, _, lent) = sent.await;
        (lent, _rt::answer(r))
    }
}

pub fn get_device() -> Result<u32, _rt::Errno> {
    let (r, data) = _rt::link::call(HANDLE, GET_DEVICE, None);
    _rt::answer(r).and_then(|_| {
        let r_ = _rt::Lift::new(data.ok_or(_rt::Errno::EBADMSG)?);
        r_.u32(0)
    })
}

pub fn device_lost<L: _rt::Lend>(
    device_: _rt::Handle,
    block: L,
) -> impl Future<Output = (L, Result<(), _rt::Errno>)> + use<L> {
    let mut block = block;
    let block_lent_ = _rt::Lend::lent(&mut block);
    let sent = _rt::wait::wait(
        device_,
        DEVICE_LOST,
        Some((16, &|w_: &mut _rt::Lower| {
            block_lent_.store(w_, 0);
        })),
        block,
    );
    async move {
        let (r, _, lent) = sent.await;
        (lent, _rt::answer(r))
    }
}

pub fn device_ready(
    device_: _rt::Handle,
) -> impl Future<Output = Result<Result<(), ErrorCode>, _rt::Errno>> + use<> {
    let sent = _rt::wait::wait(device_, DEVICE_READY, None, ());
    async move {
        let (r, data, _) = sent.await;
        _rt::answer(r).and_then(|_| {
            let r_ = _rt::Lift::new(data.as_deref().ok_or(_rt::Errno::EBADMSG)?);
            r_.case(0, 2).and_then(|d| {
                if d == 0 {
                    Ok(Ok(()))
                } else {
                    r_.case(1, 1)
                        .and_then(|v| ErrorCode::of(v).ok_or(_rt::Errno::EBADMSG))
                        .map(Err)
                }
            })
        })
    }
}

pub fn device_settled(
    device_: _rt::Handle,
) -> impl Future<Output = Result<(), _rt::Errno>> + use<> {
    let sent = _rt::wait::wait(device_, DEVICE_SETTLED, None, ());
    async move {
        let (r, _, _) = sent.await;
        _rt::answer(r)
    }
}

mod _rt {
    pub(super) use xpute_guest::{link, wait};
    pub(super) use xpute_kit::status::errno::Errno;
    pub(super) use xpute_link::{Handle, Lend, Lift, Lower, Method, answer};
}
