// pxl-core/abi/watch.rs
//
// GENERATED from spec/client/core/wit (xpute-link-gen wit.rs)

/// The interface's handle, named by `pxl:core/watch` (frame.json5 `NAMED`).
pub const HANDLE: _rt::Handle = 0x71919e;

/// `start`
pub const START: _rt::Method = 0;
/// `stop`
pub const STOP: _rt::Method = 1;

pub fn start(sensor: super::types::Sensor) -> Result<(), _rt::Errno> {
    let (r, _) = _rt::link::call(
        HANDLE,
        START,
        Some((1, &|w_: &mut _rt::Lower| {
            w_.u8(0, sensor as u8);
        })),
    );
    _rt::answer(r)
}

pub fn stop(sensor: super::types::Sensor) -> Result<(), _rt::Errno> {
    let (r, _) = _rt::link::call(
        HANDLE,
        STOP,
        Some((1, &|w_: &mut _rt::Lower| {
            w_.u8(0, sensor as u8);
        })),
    );
    _rt::answer(r)
}

mod _rt {
    pub use xpute_guest::link;
    pub use xpute_kit::status::errno::Errno;
    pub use xpute_link::{answer, Handle, Lower, Method};
}
