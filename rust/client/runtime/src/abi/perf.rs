// pxl-runtime/abi/perf.rs
//
// GENERATED from spec/client/core/wit (xpute-link-gen wit.rs)

/// The interface's handle, named by `pxl:core/perf` (frame.json5 `NAMED`).
pub const HANDLE: _rt::Handle = 0x6be6ac;

/// `start`
pub const START: _rt::Method = 0;
/// `stop`
pub const STOP: _rt::Method = 1;
/// `stats`
pub const STATS: _rt::Method = 2;
/// `table`
pub const TABLE: _rt::Method = 3;
/// `worst`
pub const WORST: _rt::Method = 4;
/// `strtab`
pub const STRTAB: _rt::Method = 5;

#[derive(Clone, Debug, PartialEq)]
pub struct Rows {
    pub first_seq: u64,
    pub end_seq: u64,
    pub bytes: Vec<u8>,
}

impl Rows {
    pub const SZ: usize = 24;
    pub const ALIGN: usize = 8;

    pub fn store(&self, w: &mut _rt::Lower, at: usize) {
        w.u64(at, self.first_seq);
        w.u64(at + 8, self.end_seq);
        w.arr(at + 16, &self.bytes);
    }

    pub fn load(r: _rt::Lift<'_>, at: usize) -> Result<Rows, _rt::Errno> {
        Ok(Rows {
            first_seq: r.u64(at)?,
            end_seq: r.u64(at + 8)?,
            bytes: r.arr::<u8>(at + 16)?,
        })
    }
}

pub fn start(side_: &_rt::Side) -> Result<Result<(), super::types::ErrorCode>, _rt::Errno> {
    let (r, data) = side_.call(HANDLE, START, _rt::Packet::Args(None));
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

pub fn stop(side_: &_rt::Side) -> Result<(), _rt::Errno> {
    let (r, _) = side_.call(HANDLE, STOP, _rt::Packet::Args(None));
    _rt::answer(r)
}

pub fn stats(side_: &_rt::Side, window_ms: Option<f64>) -> Result<Vec<u8>, _rt::Errno> {
    let (r, data) = side_.call(
        HANDLE,
        STATS,
        _rt::Packet::Args(Some((16, &|w_: &mut _rt::Lower| {
            if let Some(v) = &window_ms {
                w_.u8(0, 1);
                w_.f64(8, *v);
            }
        }))),
    );
    _rt::answer(r).and_then(|_| {
        let r_ = _rt::Lift::new(data.ok_or(_rt::Errno::EBADMSG)?);
        r_.arr::<u8>(0)
    })
}

pub fn table(side_: &_rt::Side, table: u32, from: u64, cnt: u32) -> Result<Rows, _rt::Errno> {
    let (r, data) = side_.call(
        HANDLE,
        TABLE,
        _rt::Packet::Args(Some((24, &|w_: &mut _rt::Lower| {
            w_.u32(0, table);
            w_.u64(8, from);
            w_.u32(16, cnt);
        }))),
    );
    _rt::answer(r).and_then(|_| {
        let r_ = _rt::Lift::new(data.ok_or(_rt::Errno::EBADMSG)?);
        Rows::load(r_, 0)
    })
}

pub fn worst(side_: &_rt::Side) -> Result<Vec<u8>, _rt::Errno> {
    let (r, data) = side_.call(HANDLE, WORST, _rt::Packet::Args(None));
    _rt::answer(r).and_then(|_| {
        let r_ = _rt::Lift::new(data.ok_or(_rt::Errno::EBADMSG)?);
        r_.arr::<u8>(0)
    })
}

pub fn strtab(side_: &_rt::Side) -> Result<Vec<u8>, _rt::Errno> {
    let (r, data) = side_.call(HANDLE, STRTAB, _rt::Packet::Args(None));
    _rt::answer(r).and_then(|_| {
        let r_ = _rt::Lift::new(data.ok_or(_rt::Errno::EBADMSG)?);
        r_.arr::<u8>(0)
    })
}

mod _rt {
    pub(super) use xpute_kit::status::errno::Errno;
    pub(super) use xpute_link::{Handle, Lift, Lower, Method, Packet, Side, answer};
}
