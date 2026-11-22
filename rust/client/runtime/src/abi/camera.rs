// pxl-runtime/abi/camera.rs
//
// GENERATED from spec/client/core/wit (xpute-link-gen wit.rs)

/// The interface's handle, named by `pxl:core/camera` (frame.json5 `NAMED`).
pub const HANDLE: _rt::Handle = 0x5a7108;

/// `track`
pub const TRACK: _rt::Method = 0;
/// `goto`
pub const GOTO: _rt::Method = 1;

/// The size of the fixed part of `track`'s args.
pub const TRACK_ARGS_SZ: usize = 1;

pub fn track(side_: &_rt::Side, mode: super::types::TrackingMode) -> Result<(), _rt::XputeError> {
    let (r, data) = side_.call(
        HANDLE,
        TRACK,
        _rt::Packet::Args(Some((1, &|w_: &mut _rt::Lower| {
            w_.u8(0, mode as u8);
        }))),
    );
    _rt::answer(r, data)
}

/// The size of the fixed part of `goto`'s args.
pub const GOTO_ARGS_SZ: usize = 16;

pub fn goto(side_: &_rt::Side, lon: f64, lat: f64) -> Result<(), _rt::XputeError> {
    let (r, data) = side_.call(
        HANDLE,
        GOTO,
        _rt::Packet::Args(Some((16, &|w_: &mut _rt::Lower| {
            w_.f64(0, lon);
            w_.f64(8, lat);
        }))),
    );
    _rt::answer(r, data)
}

mod _rt {
    pub(super) use xpute_kit::status::error::XputeError;
    pub(super) use xpute_link::{Handle, Lower, Method, Packet, Side, answer};
}
