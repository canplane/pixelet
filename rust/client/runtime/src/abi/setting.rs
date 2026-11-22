// pxl-runtime/abi/setting.rs
//
// GENERATED from spec/client/core/wit (xpute-link-gen wit.rs)

/// The interface's handle, named by `pxl:core/setting` (frame.json5 `NAMED`).
pub const HANDLE: _rt::Handle = 0x6f63cb;

/// `set`
pub const SET: _rt::Method = 0;

/// The size of the fixed part of `set`'s args.
pub const SET_ARGS_SZ: usize = 8;

pub fn set(
    side_: &_rt::Side,
    key: u32,
    val: u32,
) -> Result<Result<(), super::types::ErrorCode>, _rt::XputeError> {
    let (r, data) = side_.call(
        HANDLE,
        SET,
        _rt::Packet::Args(Some((8, &|w_: &mut _rt::Lower| {
            w_.u32(0, key);
            w_.u32(4, val);
        }))),
    );
    _rt::answer(r, data).and_then(|_| {
        let r_ = _rt::Lift::new(data.ok_or(_rt::Errno::EBADMSG)?);
        Ok(r_.case(0, 2).and_then(|d| {
            if d == 0 {
                Ok(Ok(()))
            } else {
                r_.case(1, 6)
                    .and_then(|v| super::types::ErrorCode::of(v).ok_or(_rt::Errno::EBADMSG))
                    .map(Err)
            }
        })?)
    })
}

mod _rt {
    pub(super) use xpute_kit::status::errno::Errno;
    pub(super) use xpute_kit::status::error::XputeError;
    pub(super) use xpute_link::{Handle, Lift, Lower, Method, Packet, Side, answer};
}
