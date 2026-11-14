// pxl-runtime/abi/perf.rs
//
// GENERATED from spec/client/core/wit (xpute-link-gen wit.rs)

/// The interface's handle, named by `pxl:core/perf` (frame.json `NAMED`).
pub const HANDLE: _rt::Handle = 0x6be6ac;

/// `start`
pub const START: _rt::Method = 0;
/// `stop`
pub const STOP: _rt::Method = 1;
/// `stats`
pub const STATS: _rt::Method = 2;
/// `ring`
pub const RING: _rt::Method = 3;
/// `strtab`
pub const STRTAB: _rt::Method = 4;

pub fn start() -> Result<Result<(), super::types::ErrorCode>, _rt::Errno> {
    let (r, data) = _rt::link::call(HANDLE, START, None);
    _rt::answer(r).and_then(|_| {
        let r_ = _rt::Lift::new(data.ok_or(_rt::Errno::EBADMSG)?);
        r_.case(0, 2).and_then(|d| {
            if d == 0 {
                Ok(Ok(()))
            } else {
                r_.case(1, 6)
                    .and_then(|v| super::types::ErrorCode::of(v).ok_or(_rt::Errno::EBADMSG))
                    .map(Err)
            }
        })
    })
}

pub fn stop() -> Result<(), _rt::Errno> {
    let (r, _) = _rt::link::call(HANDLE, STOP, None);
    _rt::answer(r)
}

pub fn stats(window_ms: Option<f64>) -> Result<Vec<u8>, _rt::Errno> {
    let (r, data) = _rt::link::call(
        HANDLE,
        STATS,
        Some((16, &|w_: &mut _rt::Lower| {
            if let Some(v) = &window_ms {
                w_.u8(0, 1);
                w_.f64(8, *v);
            }
        })),
    );
    _rt::answer(r).and_then(|_| {
        let r_ = _rt::Lift::new(data.ok_or(_rt::Errno::EBADMSG)?);
        r_.arr::<u8>(0)
    })
}

pub fn ring(ring: u32, from: u32, cnt: u32) -> Result<Vec<u8>, _rt::Errno> {
    let (r, data) = _rt::link::call(
        HANDLE,
        RING,
        Some((12, &|w_: &mut _rt::Lower| {
            w_.u32(0, ring);
            w_.u32(4, from);
            w_.u32(8, cnt);
        })),
    );
    _rt::answer(r).and_then(|_| {
        let r_ = _rt::Lift::new(data.ok_or(_rt::Errno::EBADMSG)?);
        r_.arr::<u8>(0)
    })
}

pub fn strtab() -> Result<Vec<u8>, _rt::Errno> {
    let (r, data) = _rt::link::call(HANDLE, STRTAB, None);
    _rt::answer(r).and_then(|_| {
        let r_ = _rt::Lift::new(data.ok_or(_rt::Errno::EBADMSG)?);
        r_.arr::<u8>(0)
    })
}

mod _rt {
    pub use xpute_kit::status::errno::Errno;
    pub use xpute_link::{answer, Handle, Lift, Lower, Method};
    pub use xpute_native_runtime::link;
}
