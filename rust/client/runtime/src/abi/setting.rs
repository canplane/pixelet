// pxl-runtime/abi/setting.rs
//
// GENERATED from spec/client/core/wit (xpute-link-gen wit.rs)

/// The interface's handle, named by `pxl:core/setting` (frame.json `NAMED`).
pub const HANDLE: _rt::Handle = 0x6f63cb;

/// `dev`
pub const DEV: _rt::Method = 0;
/// `set`
pub const SET: _rt::Method = 1;

pub fn dev(dev: bool) -> Result<(), _rt::Errno> {
    let (r, _) = _rt::link::call(
        HANDLE,
        DEV,
        Some((1, &|w_: &mut _rt::Lower| {
            w_.bool(0, dev);
        })),
    );
    _rt::answer(r)
}

pub fn set(key: u32, val: u32) -> Result<Result<(), super::types::ErrorCode>, _rt::Errno> {
    let (r, data) = _rt::link::call(
        HANDLE,
        SET,
        Some((8, &|w_: &mut _rt::Lower| {
            w_.u32(0, key);
            w_.u32(4, val);
        })),
    );
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

mod _rt {
    pub use xpute_kit::status::errno::Errno;
    pub use xpute_link::{answer, Handle, Lift, Lower, Method};
    pub use xpute_native_runtime::link;
}
