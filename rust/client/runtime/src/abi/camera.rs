// pxl-runtime/abi/camera.rs
//
// GENERATED from spec/client/core/wit (xpute-link-gen wit.rs)

/// The interface's handle, named by `pxl:core/camera` (frame.json5 `NAMED`).
pub const HANDLE: _rt::Handle = 0x5a7108;

/// `track`
pub const TRACK: _rt::Method = 0;
/// `goto`
pub const GOTO: _rt::Method = 1;

pub fn track(side_: &_rt::Side, mode: super::types::TrackingMode) -> Result<(), _rt::Errno> {
    let (r, _) = side_.call(
        HANDLE,
        TRACK,
        _rt::Packet::Args(Some((1, &|w_: &mut _rt::Lower| {
            w_.u8(0, mode as u8);
        }))),
    );
    _rt::answer(r)
}

pub fn goto(side_: &_rt::Side, lon: f64, lat: f64) -> Result<(), _rt::Errno> {
    let (r, _) = side_.call(
        HANDLE,
        GOTO,
        _rt::Packet::Args(Some((16, &|w_: &mut _rt::Lower| {
            w_.f64(0, lon);
            w_.f64(8, lat);
        }))),
    );
    _rt::answer(r)
}

mod _rt {
    pub(super) use xpute_kit::status::errno::Errno;
    pub(super) use xpute_link::{Handle, Lower, Method, Packet, Side, answer};
}
